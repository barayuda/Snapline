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

/// Finding the "Snapline" folder again. Scope `drive.file` shows Snapline only the files it made.
public enum DriveFolder {
    public static let name = "Snapline"
    public static let mimeType = "application/vnd.google-apps.folder"

    /// What checking the saved folder ID found.
    public enum Saved: Equatable, Sendable {
        case use, gone, failed
    }

    /// Only a 404 or a trashed folder is gone. Any other error (expired token, rate limit, outage) fails the upload,
    /// so a passing failure never makes a second folder.
    public static func saved(status: Int, body: Data) -> Saved {
        struct Folder: Decodable { let trashed: Bool }
        switch status {
        case 200:
            guard let folder = try? JSONDecoder().decode(Folder.self, from: body) else { return .failed }
            return folder.trashed ? .gone : .use
        case 404: return .gone
        default: return .failed
        }
    }

    /// `files.list` for Snapline's folders named "Snapline" that are not in the trash, oldest first.
    public static func searchURL(api: URL) -> URL {
        api.appending(path: "files").appending(queryItems: [
            URLQueryItem(name: "q", value: "name = '\(name)' and mimeType = '\(mimeType)' and trashed = false"),
            URLQueryItem(name: "orderBy", value: "createdTime"),
            URLQueryItem(name: "pageSize", value: "1"),
            URLQueryItem(name: "spaces", value: "drive"),
            URLQueryItem(name: "fields", value: "files(id)"),
        ])
    }

    /// The first folder in a `files.list` reply, if any.
    public static func firstID(inList body: Data) throws -> String? {
        struct Found: Decodable { let id: String }
        struct List: Decodable { let files: [Found]? }
        return try JSONDecoder().decode(List.self, from: body).files?.first?.id
    }
}
