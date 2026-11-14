import Foundation
import Testing
@testable import SnaplineCore

struct PKCETests {
    // RFC 7636 Appendix B.
    @Test func challengeMatchesRFCExample() {
        #expect(PKCE.challenge(for: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk") == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM")
    }

    @Test func verifierIsURLSafeAndLongEnough() {
        let verifier = PKCE.makeVerifier()
        #expect((43...128).contains(verifier.count))
        #expect(verifier.rangeOfCharacter(from: CharacterSet(charactersIn: "+/=")) == nil)
        #expect(PKCE.makeVerifier() != verifier)
    }
}

struct GoogleOAuthTests {
    @Test func authorizationURLHasAllRequiredParameters() throws {
        let url = GoogleOAuth.authorizationURL(clientID: "client-123", redirectURI: "http://127.0.0.1:5555",
                                               challenge: "abc", state: "xyz")
        let items = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems)
        let query = Dictionary(uniqueKeysWithValues: items.map { ($0.name, $0.value ?? "") })
        #expect(url.host == "accounts.google.com")
        #expect(query == [
            "client_id": "client-123", "redirect_uri": "http://127.0.0.1:5555", "response_type": "code",
            "scope": "openid email https://www.googleapis.com/auth/drive.file", "code_challenge": "abc",
            "code_challenge_method": "S256", "state": "xyz", "access_type": "offline", "prompt": "consent",
        ])
    }

    @Test(arguments: [
        ("GET /?state=xyz&code=4/0Abc&scope=email HTTP/1.1", "4/0Abc"),
        ("GET /?code=4%2F0Abc&state=xyz HTTP/1.1", "4/0Abc"),   // percent-encoded code is decoded
    ])
    func callbackReturnsTheCode(line: String, code: String) throws {
        #expect(try GoogleOAuth.code(fromRequestLine: line, expectedState: "xyz") == code)
    }

    @Test(arguments: [
        // State is checked first: a request without our state cannot end sign-in, not even with an error.
        ("GET /?error=%3Cscript%3E HTTP/1.1", "Sign-in state did not match."),
        ("GET /?state=evil&code=1 HTTP/1.1", "Sign-in state did not match."),
        ("GET /?code=1 HTTP/1.1", "Sign-in state did not match."),
        ("GET /?state=evil&state=xyz&code=1 HTTP/1.1", "Sign-in state did not match."),   // first value counts
        ("GET /?error=access_denied&state=xyz HTTP/1.1", "Google sign-in failed: access_denied"),
        ("GET /?state=xyz&code= HTTP/1.1", "No sign-in code received."),
        ("GET /?state=xyz HTTP/1.1", "No sign-in code received."),
        ("GET", "Unexpected sign-in response."),
    ])
    func callbackRejects(line: String, message: String) {
        let error = #expect(throws: GoogleOAuth.CallbackError.self) {
            try GoogleOAuth.code(fromRequestLine: line, expectedState: "xyz")
        }
        #expect(error?.message == message)
    }

    @Test func callbackPageEscapesHTML() {
        let page = GoogleOAuth.callbackPage(signedIn: false, reason: "failed: <script>alert(1)</script> & \"x\"")
        #expect(!page.contains("<script>"))
        #expect(page.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;x&quot;"))
    }

    @Test func callbackPageSaysWhatHappenedAndWhatToDoNext() {
        let ok = GoogleOAuth.callbackPage(signedIn: true)
        #expect(ok.contains("<title>Signed in to Snapline</title>"))
        #expect(ok.contains("You can close this tab"))

        let failed = GoogleOAuth.callbackPage(signedIn: false, reason: "Google sign-in failed: access_denied")
        #expect(failed.contains("<title>Snapline sign-in did not finish</title>"))
        #expect(failed.contains("Google sign-in failed: access_denied"))
        #expect(failed.contains("Sign in with Google…"))   // how to try again
    }

    @Test(arguments: [true, false])
    func callbackPageLoadsNothingFromTheNetwork(signedIn: Bool) {
        // Served from 127.0.0.1 with the sign-in code in its URL: no fonts, images or links that could leak it or fail offline.
        let page = GoogleOAuth.callbackPage(signedIn: signedIn, reason: "x")
        #expect(!page.contains("http"))   // also rules out url(https://…) and links
        #expect(!page.contains("src="))
        #expect(!page.contains("//"))
    }

    @Test func formBodyEncodesEverythingButUnreservedASCII() {
        let body = String(decoding: GoogleOAuth.formBody(["b": "x y+z/é", "a": "1"]), as: UTF8.self)
        #expect(body == "a=1&b=x%20y%2Bz%2F%C3%A9")   // sorted keys; "é" encoded as UTF-8
    }

    @Test func tokenResponseDecodes() throws {
        let json = #"{"access_token":"ya29.a","expires_in":3599,"refresh_token":"1//r","scope":"email","token_type":"Bearer","id_token":"x"}"#
        let token = try JSONDecoder().decode(GoogleOAuth.TokenResponse.self, from: Data(json.utf8))
        #expect(token.accessToken == "ya29.a")
        #expect(token.refreshToken == "1//r")
        #expect(token.expiresIn == 3599)
    }

    @Test(arguments: [
        (#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#, true),
        (#"{"error":"invalid_client"}"#, false),
        ("<html>Bad Gateway</html>", false),
    ])
    func recognisesAnExpiredSignIn(body: String, expired: Bool) {
        #expect(GoogleOAuth.isInvalidGrant(Data(body.utf8)) == expired)
    }

    @Test func emailFromIDToken() {
        // header.payload.signature where payload = {"email":"me@gmail.com","email_verified":true}
        let payload = Data(#"{"email":"me@gmail.com","email_verified":true}"#.utf8).base64URLEncodedString()
        #expect(GoogleOAuth.email(fromIDToken: "eyJhbGciOiJSUzI1NiJ9.\(payload).sig") == "me@gmail.com")
        #expect(GoogleOAuth.email(fromIDToken: "not-a-jwt") == nil)
    }
}
