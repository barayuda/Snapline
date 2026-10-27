import XCTest
@testable import SnaplineCore

final class DriveRequestTests: XCTestCase {
    func testMultipartRelatedBodyLayout() throws {
        let png = Data([0x89, 0x50, 0x4E, 0x47])
        let body = DriveMultipart.body(
            boundary: "BOUNDARY",
            metadata: ["name": "Screenshot at Oct 28 10-00-00.png", "parents": ["folder1"]],
            fileData: png,
            mimeType: "image/png"
        )
        let head = "--BOUNDARY\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n"
        XCTAssertTrue(body.starts(with: Data(head.utf8)))
        XCTAssertTrue(body.range(of: Data("\r\n--BOUNDARY\r\nContent-Type: image/png\r\n\r\n".utf8)) != nil)
        XCTAssertTrue(body.range(of: png) != nil)
        XCTAssertTrue(body.suffix(16) == Data("\r\n--BOUNDARY--\r\n".utf8))

        // The JSON part must be valid JSON with the parent folder.
        let jsonStart = head.utf8.count
        let jsonEnd = try XCTUnwrap(body.range(of: Data("\r\n--BOUNDARY\r\n".utf8))).lowerBound
        let json = try JSONSerialization.jsonObject(with: body.subdata(in: jsonStart..<jsonEnd)) as? [String: Any]
        XCTAssertEqual(json?["parents"] as? [String], ["folder1"])
    }

    func testDirectImageURL() {
        XCTAssertEqual(DriveMultipart.directImageURL(fileID: "abc").absoluteString, "https://drive.google.com/uc?export=view&id=abc")
    }
}
