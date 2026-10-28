import CryptoKit
import Foundation

/// PKCE (RFC 7636) for the OAuth "installed app" flow.
public enum PKCE {
    /// A new random code verifier (43 URL-safe characters from 32 secure random bytes).
    public static func makeVerifier() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        // A failed random source would give a predictable verifier; never sign in with that.
        precondition(SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess, "No secure random bytes")
        return Data(bytes).base64URLEncodedString()
    }

    /// The S256 challenge for `verifier`: base64url(SHA-256(verifier)).
    public static func challenge(for verifier: String) -> String {
        Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncodedString()
    }
}

/// Pure parts of Google sign-in: URLs, callback parsing, token decoding.
/// The network and the loopback listener live in the app target.
public enum GoogleOAuth {
    static let scope = "openid email https://www.googleapis.com/auth/drive.file"
    public static let tokenURL = URL(string: "https://oauth2.googleapis.com/token")!
    public static let revokeURL = URL(string: "https://oauth2.googleapis.com/revoke")!

    public struct CallbackError: Error, LocalizedError {
        public let message: String
        public var errorDescription: String? { message }
    }

    public struct TokenResponse: Decodable, Sendable {
        public let accessToken: String
        public let expiresIn: Int
        public let refreshToken: String?
        public let idToken: String?

        enum CodingKeys: String, CodingKey {
            case accessToken = "access_token", expiresIn = "expires_in"
            case refreshToken = "refresh_token", idToken = "id_token"
        }
    }

    /// Google's consent page URL for the installed-app flow, asking for a refresh token (offline access).
    public static func authorizationURL(clientID: String, redirectURI: String, challenge: String, state: String) -> URL {
        var c = URLComponents(string: "https://accounts.google.com/o/oauth2/v2/auth")!
        c.queryItems = [
            URLQueryItem(name: "client_id", value: clientID),
            URLQueryItem(name: "redirect_uri", value: redirectURI),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "scope", value: scope),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
            URLQueryItem(name: "state", value: state),
            URLQueryItem(name: "access_type", value: "offline"),
            // Always return a refresh token, also when the user signed in before.
            URLQueryItem(name: "prompt", value: "consent"),
        ]
        return c.url!
    }

    /// Reads the first line of the HTTP request Google redirects the browser to.
    public static func code(fromRequestLine line: String, expectedState: String) throws -> String {
        let parts = line.split(separator: " ")
        guard parts.count >= 2, let c = URLComponents(string: "http://127.0.0.1" + parts[1]) else {
            throw CallbackError(message: "Unexpected sign-in response.")
        }
        let query = Dictionary((c.queryItems ?? []).map { ($0.name, $0.value ?? "") }, uniquingKeysWith: { a, _ in a })
        // State first: a request that does not carry our state must not decide anything, not even an error.
        guard query["state"] == expectedState else { throw CallbackError(message: "Sign-in state did not match.") }
        if let error = query["error"] { throw CallbackError(message: "Google sign-in failed: \(error)") }
        guard let code = query["code"], !code.isEmpty else { throw CallbackError(message: "No sign-in code received.") }
        return code
    }

    /// The branded page the browser shows after the redirect, served by Snapline on 127.0.0.1.
    /// Self-contained (inline styles and icon) so it works offline and loads nothing that could see the code in its URL.
    /// `reason` is escaped: it can contain text from the request.
    public static func callbackPage(signedIn: Bool, reason: String = "") -> String {
        let escaped = reason
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "'", with: "&#39;")
        let title = signedIn ? "Signed in to Snapline" : "Snapline sign-in did not finish"
        let body = signedIn
            ? "<h1>You’re signed in</h1><p>Snapline can now upload to your Google Drive. You can close this tab and go back to Snapline.</p>"
            : "<h1>Sign-in did not finish</h1><p class=reason>\(escaped)</p>"
                + "<p>Close this tab, then click <b>Sign in with Google…</b> in Snapline’s Preferences to try again.</p>"
        return """
            <!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
            <title>\(title)</title><style>\(callbackStyle)</style>
            <main>\(callbackIcon)\(body)</main>
            """
    }

    // Brand colours from BRAND.md: ink, paper, violet deep → cyan.
    private static let callbackStyle = """
        :root{color-scheme:light dark;--paper:#f6f5fb;--ink:#0c0819;--muted:#565170;--card:#fff}
        @media(prefers-color-scheme:dark){:root{--paper:#0c0819;--ink:#f3f1fa;--muted:#a9a3c2;--card:#17112b}}
        body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--paper);color:var(--ink);\
        font:17px/1.55 -apple-system,BlinkMacSystemFont,system-ui,sans-serif}
        main{width:min(100% - 3rem,30rem);padding:2.5rem;border-radius:20px;background:var(--card);\
        box-shadow:0 1px 2px rgba(12,8,25,.06),0 24px 48px -24px rgba(44,20,120,.35)}
        svg{width:64px;height:64px;display:block;margin-bottom:1.5rem}
        h1{font-size:1.75rem;line-height:1.15;letter-spacing:-.02em;margin:0 0 .6rem}
        p{margin:0 0 .75rem;color:var(--muted)}p:last-child{margin:0}
        .reason{color:var(--ink);padding:.6rem .8rem;border-left:3px solid #e5484d;background:rgba(229,72,77,.08);border-radius:0 8px 8px 0}
        """

    /// The app icon, drawn inline: a violet → cyan squircle with white selection corners and a spark.
    private static let callbackIcon = """
        <svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">\
        <stop offset="0" stop-color="#4c1ab8"/><stop offset=".5" stop-color="#7c3aed"/><stop offset="1" stop-color="#22d3ee"/>\
        </linearGradient></defs><rect width="64" height="64" rx="15" fill="url(#g)"/>\
        <g fill="none" stroke="#fff" stroke-width="4.5" stroke-linecap="round" stroke-linejoin="round">\
        <path d="M14 24v-10h10M40 14h10v10M50 40v10h-10M24 50h-10v-10"/></g>\
        <path fill="#fff" d="M32 22c.9 6.4 3.6 9.1 10 10c-6.4.9-9.1 3.6-10 10c-.9-6.4-3.6-9.1-10-10c6.4-.9 9.1-3.6 10-10z"/>\
        <path fill="#fff" d="M42 18c.3 2 1 2.7 3 3c-2 .3-2.7 1-3 3c-.3-2-1-2.7-3-3c2-.3 2.7-1 3-3z"/></svg>
        """

    /// True when the token endpoint's error body says the refresh token is expired or revoked
    /// (`{"error":"invalid_grant"}`), so the user has to sign in again.
    public static func isInvalidGrant(_ body: Data) -> Bool {
        struct ErrorBody: Decodable { let error: String }
        return (try? JSONDecoder().decode(ErrorBody.self, from: body))?.error == "invalid_grant"
    }

    /// Form body for the token endpoint: sorted `key=value` pairs, values percent-encoded.
    public static func formBody(_ fields: [String: String]) -> Data {
        let pairs = fields.sorted { $0.key < $1.key }.map {
            "\($0.key)=\(PercentEncoding.unreserved($0.value))"
        }
        return Data(pairs.joined(separator: "&").utf8)
    }

    /// Email claim from an ID token. The token came straight from Google over TLS,
    /// so the signature is not checked; it is only used to show who is signed in.
    public static func email(fromIDToken token: String) -> String? {
        let parts = token.split(separator: ".")
        guard parts.count == 3, let data = Data(base64URLEncoded: String(parts[1])),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return json["email"] as? String
    }
}

extension Data {
    func base64URLEncodedString() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    init?(base64URLEncoded s: String) {
        var b64 = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        b64 += String(repeating: "=", count: (4 - b64.count % 4) % 4)
        self.init(base64Encoded: b64)
    }
}
