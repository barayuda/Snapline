import AppKit
import Network
import SnaplineCore

/// Google sign-in with the OAuth "installed app" flow: system browser + PKCE + a
/// one-time listener on 127.0.0.1. The refresh token is kept in the Keychain.
@MainActor
final class GoogleAuth {
    static let shared = GoogleAuth()

    private var accessToken: String?
    private var expiry = Date.distantPast

    var isSignedIn: Bool { Keychain.get(.googleRefreshToken) != nil }
    var email: String? { UserDefaults.standard.string(forKey: Key.googleEmail) }

    private var credentials: (id: String, secret: String)? {
        let id = Settings.string(Key.googleClientID)
        guard !id.isEmpty, let secret = Keychain.get(.googleClientSecret), !secret.isEmpty else { return nil }
        return (id, secret)
    }

    func signIn() async throws {
        guard let (clientID, clientSecret) = credentials else {
            throw UploadError(message: "Enter the Google OAuth client ID and secret first.")
        }
        let state = UUID().uuidString
        let server = try LoopbackServer(state: state)
        let port = try await server.start()
        defer { server.stop() }

        let redirect = "http://127.0.0.1:\(port)"
        let verifier = PKCE.makeVerifier()
        NSWorkspace.shared.open(GoogleOAuth.authorizationURL(clientID: clientID, redirectURI: redirect,
                                                             challenge: PKCE.challenge(for: verifier), state: state))

        let line = try await server.waitForCallback()
        // The browser waits for the token exchange, so its page only says "signed in" once that is true.
        do {
            let code = try GoogleOAuth.code(fromRequestLine: line, expectedState: state)
            let token = try await tokenRequest([
                "code": code, "client_id": clientID, "client_secret": clientSecret,
                "redirect_uri": redirect, "grant_type": "authorization_code", "code_verifier": verifier,
            ])
            guard let refresh = token.refreshToken else { throw UploadError(message: "Google did not return a refresh token.") }
            Keychain.set(.googleRefreshToken, refresh)
            UserDefaults.standard.set(token.idToken.flatMap(GoogleOAuth.email(fromIDToken:)), forKey: Key.googleEmail)
            UserDefaults.standard.removeObject(forKey: Key.driveFolderID)
            accessToken = token.accessToken
            expiry = Date().addingTimeInterval(TimeInterval(token.expiresIn - 60))
            server.reply(signedIn: true)
        } catch {
            server.reply(signedIn: false, reason: error.localizedDescription)
            throw error
        }
    }

    func signOut() async {
        if let refresh = Keychain.get(.googleRefreshToken) {
            var request = URLRequest(url: GoogleOAuth.revokeURL)
            request.httpMethod = "POST"
            request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
            _ = try? await send(request, body: GoogleOAuth.formBody(["token": refresh]))
        }
        Keychain.set(.googleRefreshToken, nil)
        UserDefaults.standard.removeObject(forKey: Key.googleEmail)
        UserDefaults.standard.removeObject(forKey: Key.driveFolderID)
        accessToken = nil
    }

    func validAccessToken() async throws -> String {
        if let accessToken, Date() < expiry { return accessToken }
        guard let (clientID, clientSecret) = credentials, let refresh = Keychain.get(.googleRefreshToken) else {
            throw UploadError(message: "Sign in to Google in Snapline Preferences → Upload.")
        }
        do {
            let token = try await tokenRequest(["client_id": clientID, "client_secret": clientSecret,
                                                "refresh_token": refresh, "grant_type": "refresh_token"])
            accessToken = token.accessToken
            expiry = Date().addingTimeInterval(TimeInterval(token.expiresIn - 60))
            return token.accessToken
        } catch let error as HTTPError where error.status == 400 && GoogleOAuth.isInvalidGrant(error.body) {
            // Revoked or expired (for example an app left in "Testing" mode for 7 days).
            Keychain.set(.googleRefreshToken, nil)
            UserDefaults.standard.removeObject(forKey: Key.googleEmail)
            throw UploadError(message: "Google sign-in expired. Sign in again in Snapline Preferences → Upload.")
        }
    }

    private func tokenRequest(_ fields: [String: String]) async throws -> GoogleOAuth.TokenResponse {
        var request = URLRequest(url: GoogleOAuth.tokenURL)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        let (data, http) = try await send(request, body: GoogleOAuth.formBody(fields))
        try checkStatus(http, body: data, action: "Google token request")
        return try JSONDecoder().decode(GoogleOAuth.TokenResponse.self, from: data)
    }
}

/// Accepts the single browser redirect from Google on 127.0.0.1:<random port>.
/// `@unchecked Sendable`: all mutable state (`connection`, `callback`, `early`, `ready`) is only touched on `queue`
/// (`ready` is set before the listener starts, so before any handler can run).
private final class LoopbackServer: @unchecked Sendable {
    private let listener: NWListener
    private let state: String
    private let queue = DispatchQueue(label: "snapline.oauth")
    private var connection: NWConnection?
    private var callback: CheckedContinuation<String, Error>?
    /// A redirect that arrived before `waitForCallback` was called.
    private var early: Result<String, Error>?
    private var ready: CheckedContinuation<UInt16, Error>?

    init(state: String) throws {
        self.state = state
        let params = NWParameters.tcp
        params.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: params)
    }

    func start() async throws -> UInt16 {
        try await withCheckedThrowingContinuation { (c: CheckedContinuation<UInt16, Error>) in
            ready = c
            listener.stateUpdateHandler = { [weak self] state in
                guard let self, let ready else { return }
                switch state {
                case .ready:
                    self.ready = nil
                    ready.resume(returning: listener.port?.rawValue ?? 0)
                case let .failed(error), let .waiting(error):
                    self.ready = nil
                    ready.resume(throwing: error)
                case .cancelled:
                    self.ready = nil
                    ready.resume(throwing: CancellationError())
                default:
                    break
                }
            }
            listener.newConnectionHandler = { [weak self] conn in self?.accept(conn) }
            listener.start(queue: queue)
        }
    }

    /// Waits up to 5 minutes for the redirect that carries our state (other requests get a 404).
    func waitForCallback() async throws -> String {
        try await withCheckedThrowingContinuation { c in
            queue.async { [weak self] in
                guard let self else { return c.resume(throwing: CancellationError()) }
                if let early = self.early {
                    self.early = nil
                    return c.resume(with: early)
                }
                self.callback = c
                self.queue.asyncAfter(deadline: .now() + 300) { [weak self] in
                    self?.finish(.failure(UploadError(message: "Google sign-in timed out. Try again.")))
                }
            }
        }
    }

    private func accept(_ conn: NWConnection) {
        conn.start(queue: queue)
        conn.receive(minimumIncompleteLength: 1, maximumLength: 16_384) { [weak self] data, _, _, _ in
            guard let self else { return }
            let line = data.flatMap { String(data: $0, encoding: .utf8) }?.components(separatedBy: "\r\n").first ?? ""
            // Only the redirect carrying our state counts; anything else (favicon, stray local requests) gets a 404.
            if connection == nil, line.contains("state=\(state)") {
                connection = conn
                finish(.success(line))
            } else {
                conn.send(content: Data("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".utf8),
                          completion: .contentProcessed { _ in conn.cancel() })
            }
        }
    }

    private func finish(_ result: Result<String, Error>) {
        guard let c = callback else {
            if early == nil { early = result }   // waitForCallback has not started yet
            return
        }
        callback = nil
        c.resume(with: result)
    }

    func reply(signedIn: Bool, reason: String = "") {
        queue.async {
            guard let conn = self.connection else { return }
            let html = GoogleOAuth.callbackPage(signedIn: signedIn, reason: reason)
            // The page's URL holds the sign-in code: allow only its inline style and icon, and keep it out of the cache.
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: \(html.utf8.count)\r\n"
                + "Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n"
                + "Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n\(html)"
            conn.send(content: Data(response.utf8), completion: .contentProcessed { _ in conn.cancel() })
        }
    }

    func stop() {
        queue.asyncAfter(deadline: .now() + 1) { self.listener.cancel() }
    }
}
