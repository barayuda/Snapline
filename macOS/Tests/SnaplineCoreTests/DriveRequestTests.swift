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

    // A user report: signing in again forgot the saved folder, and each sign-in made one more "Snapline" in My Drive.
    @Test func searchFindsTheOldestSnaplineFolder() throws {
        let url = DriveFolder.searchURL(api: URL(string: "https://www.googleapis.com/drive/v3")!)
        let items = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems)
        let query = Dictionary(uniqueKeysWithValues: items.map { ($0.name, $0.value ?? "") })
        #expect(url.path() == "/drive/v3/files")
        #expect(query["q"] == "name = 'Snapline' and mimeType = 'application/vnd.google-apps.folder' and trashed = false")
        #expect(query["orderBy"] == "createdTime")
        #expect(query["fields"] == "files(id)")

        #expect(try DriveFolder.firstID(inList: Data(#"{"files":[{"id":"OLD"},{"id":"NEW"}]}"#.utf8)) == "OLD")
        #expect(try DriveFolder.firstID(inList: Data(#"{"files":[]}"#.utf8)) == nil)
        #expect(try DriveFolder.firstID(inList: Data("{}".utf8)) == nil)
    }

    // Only a 404 or a trashed folder is gone: a passing error (expired token, rate limit, outage) must not make a
    // second folder.
    @Test func onlyAMissingOrTrashedFolderIsGone() {
        #expect(DriveFolder.saved(status: 200, body: Data(#"{"id":"F1","trashed":false}"#.utf8)) == .use)
        #expect(DriveFolder.saved(status: 200, body: Data(#"{"id":"F1","trashed":true}"#.utf8)) == .gone)
        #expect(DriveFolder.saved(status: 404, body: Data("{}".utf8)) == .gone)
        for status in [401, 403, 429, 500, 503] {
            #expect(DriveFolder.saved(status: status, body: Data("busy".utf8)) == .failed)
        }
        #expect(DriveFolder.saved(status: 200, body: Data("not json".utf8)) == .failed)
    }
}

