import XCTest
@testable import SnaplineCore

final class GoogleOAuthTests: XCTestCase {
    // RFC 7636 Appendix B.
    func testPKCEChallengeMatchesRFCExample() {
        XCTAssertEqual(
            PKCE.challenge(for: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        )
    }

    func testPKCEVerifierIsURLSafeAndLongEnough() {
        let verifier = PKCE.makeVerifier()
        XCTAssertGreaterThanOrEqual(verifier.count, 43)
        XCTAssertLessThanOrEqual(verifier.count, 128)
        XCTAssertNil(verifier.rangeOfCharacter(from: CharacterSet(charactersIn: "+/=")))
    }

    func testAuthorizationURLHasAllRequiredParameters() throws {
        let url = GoogleOAuth.authorizationURL(
            clientID: "client-123",
            redirectURI: "http://127.0.0.1:5555",
            challenge: "abc",
            state: "xyz"
        )
        let items = try XCTUnwrap(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems)
        let query = Dictionary(uniqueKeysWithValues: items.map { ($0.name, $0.value ?? "") })
        XCTAssertEqual(url.host, "accounts.google.com")
        XCTAssertEqual(query["client_id"], "client-123")
        XCTAssertEqual(query["redirect_uri"], "http://127.0.0.1:5555")
        XCTAssertEqual(query["response_type"], "code")
        XCTAssertEqual(query["code_challenge"], "abc")
        XCTAssertEqual(query["code_challenge_method"], "S256")
        XCTAssertEqual(query["state"], "xyz")
        XCTAssertEqual(query["access_type"], "offline")
        XCTAssertEqual(query["scope"], "openid email https://www.googleapis.com/auth/drive.file")
    }

    func testCallbackParsingReturnsCodeWhenStateMatches() throws {
        let line = "GET /?state=xyz&code=4/0Abc&scope=email HTTP/1.1"
        XCTAssertEqual(try GoogleOAuth.code(fromRequestLine: line, expectedState: "xyz"), "4/0Abc")
    }

    func testCallbackParsingRejectsWrongStateAndErrors() {
        XCTAssertThrowsError(try GoogleOAuth.code(fromRequestLine: "GET /?state=evil&code=1 HTTP/1.1", expectedState: "xyz"))
        XCTAssertThrowsError(try GoogleOAuth.code(fromRequestLine: "GET /?error=access_denied&state=xyz HTTP/1.1", expectedState: "xyz"))
    }

    func testTokenResponseDecodes() throws {
        let json = #"{"access_token":"ya29.a","expires_in":3599,"refresh_token":"1//r","scope":"email","token_type":"Bearer","id_token":"x"}"#
        let token = try JSONDecoder().decode(GoogleOAuth.TokenResponse.self, from: Data(json.utf8))
        XCTAssertEqual(token.accessToken, "ya29.a")
        XCTAssertEqual(token.refreshToken, "1//r")
        XCTAssertEqual(token.expiresIn, 3599)
    }

    func testEmailFromIDToken() {
        // header.payload.signature where payload = {"email":"me@gmail.com","email_verified":true}
        let payload = Data(#"{"email":"me@gmail.com","email_verified":true}"#.utf8).base64URLEncodedString()
        XCTAssertEqual(GoogleOAuth.email(fromIDToken: "eyJhbGciOiJSUzI1NiJ9.\(payload).sig"), "me@gmail.com")
        XCTAssertNil(GoogleOAuth.email(fromIDToken: "not-a-jwt"))
    }
}
