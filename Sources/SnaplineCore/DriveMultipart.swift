import Foundation

/// The metadata part of a Drive upload.
public struct DriveFileMetadata: Encodable, Sendable {
    public let name: String
    public let parents: [String]

    public init(name: String, parents: [String]) {
        self.name = name
        self.parents = parents
    }
}

/// Request body helpers for the Google Drive v3 API.
public enum DriveMultipart {
    /// `multipart/related` body for `uploadType=multipart`: JSON metadata, then the file.
    public static func body(boundary: String, metadata: DriveFileMetadata, fileData: Data, mimeType: String) throws -> Data {
        var body = Data()
        body.append(Data("--\(boundary)\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n".utf8))
        body.append(try JSONEncoder().encode(metadata))
        body.append(Data("\r\n--\(boundary)\r\nContent-Type: \(mimeType)\r\n\r\n".utf8))
        body.append(fileData)
        body.append(Data("\r\n--\(boundary)--\r\n".utf8))
        return body
    }

    /// Undocumented by Google but widely used; image search engines need a direct image URL.
    public static func directImageURL(fileID: String) -> URL {
        var c = URLComponents(string: "https://drive.google.com/uc")!
        c.queryItems = [URLQueryItem(name: "export", value: "view"), URLQueryItem(name: "id", value: fileID)]
        return c.url!
    }
}
