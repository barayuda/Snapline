import XCTest
@testable import SnaplineCore

final class SigV4Tests: XCTestCase {
    // AWS Signature Version 4 test suite, case "get-vanilla".
    func testGetVanillaMatchesAWSTestVector() {
        let signer = SigV4Signer(
            accessKey: "AKIDEXAMPLE",
            secretKey: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            region: "us-east-1",
            service: "service"
        )
        var request = URLRequest(url: URL(string: "https://example.amazonaws.com/")!)
        request.httpMethod = "GET"
        let date = ISO8601DateFormatter().date(from: "2015-08-30T12:36:00Z")!

        let signed = signer.sign(request, payloadHash: SigV4Signer.sha256Hex(Data()), date: date)

        XCTAssertEqual(signed.value(forHTTPHeaderField: "x-amz-date"), "20150830T123600Z")
        XCTAssertEqual(
            signed.value(forHTTPHeaderField: "Authorization"),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, "
                + "SignedHeaders=host;x-amz-date, "
                + "Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        )
    }

    func testCanonicalURIEncodesEachPathSegmentOnce() {
        XCTAssertEqual(SigV4Signer.canonicalURI("/bucket/shots/2026/10/a b+c.png"), "/bucket/shots/2026/10/a%20b%2Bc.png")
        XCTAssertEqual(SigV4Signer.canonicalURI(""), "/")
    }

    func testSHA256OfEmptyPayload() {
        XCTAssertEqual(SigV4Signer.sha256Hex(Data()), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    }
}
