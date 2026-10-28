import CryptoKit
import Foundation

/// PKCE (RFC 7636) for the OAuth "installed app" flow.
public enum PKCE {
    public static func makeVerifier() -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes).base64URLEncodedString()
    }

    public static func challenge(for verifier: String) -> String {
        Data(SHA256.hash(data: Data(verifier.utf8))).base64URLEncodedString()
    }
}

/// Pure parts of Google sign-in: URLs, callback parsing, token decoding.
/// The network and the loopback listener live in the app target.
public enum GoogleOAuth {
    public static let scope = "openid email https://www.googleapis.com/auth/drive.file"
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

    /// The page the browser shows after the redirect. `message` is escaped: it can contain text from the request.
    public static func callbackPage(message: String) -> String {
        let escaped = message
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "'", with: "&#39;")
        return "<!doctype html><meta charset=utf-8><title>Snapline</title>"
            + "<body style='font:16px -apple-system;padding:40px'>\(escaped)"
    }

    /// Form body for the token endpoint.
    public static func formBody(_ fields: [String: String]) -> Data {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-._~"))
        let pairs = fields.sorted { $0.key < $1.key }.map {
            "\($0.key)=\($0.value.addingPercentEncoding(withAllowedCharacters: allowed) ?? $0.value)"
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
    public func base64URLEncodedString() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    public init?(base64URLEncoded s: String) {
        var b64 = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        b64 += String(repeating: "=", count: (4 - b64.count % 4) % 4)
        self.init(base64Encoded: b64)
    }
}
