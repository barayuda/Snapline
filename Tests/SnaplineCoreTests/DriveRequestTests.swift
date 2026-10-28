import Foundation
import Testing
@testable import SnaplineCore

struct DriveRequestTests {
    @Test func multipartRelatedBodyLayout() throws {
        let png = Data([0x89, 0x50, 0x4E, 0x47])
        let body = try DriveMultipart.body(
            boundary: "BOUNDARY",
            metadata: DriveFileMetadata(name: "Screenshot at Oct 28 10-00-00.png", parents: ["folder1"]),
            fileData: png,
            mimeType: "image/png"
        )
        let head = Data("--BOUNDARY\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n".utf8)
        #expect(body.starts(with: head))
        #expect(body.range(of: Data("\r\n--BOUNDARY\r\nContent-Type: image/png\r\n\r\n".utf8)) != nil)
        #expect(body.range(of: png) != nil)
        #expect(body.suffix(16) == Data("\r\n--BOUNDARY--\r\n".utf8))

        // The JSON part decodes back to the same name and parent folder.
        let jsonEnd = try #require(body.range(of: Data("\r\n--BOUNDARY\r\n".utf8))).lowerBound
        let json = try #require(JSONSerialization.jsonObject(with: body.subdata(in: head.count..<jsonEnd)) as? [String: Any])
        #expect(json["name"] as? String == "Screenshot at Oct 28 10-00-00.png")
        #expect(json["parents"] as? [String] == ["folder1"])
    }

    @Test func directImageURL() {
        #expect(DriveMultipart.directImageURL(fileID: "abc").absoluteString == "https://drive.google.com/uc?export=view&id=abc")
    }
}
