import Foundation
import SnaplineCore

struct UploadResult: Sendable {
    /// Link copied to the clipboard and opened by "Open".
    let pageURL: URL
    /// Direct image URL for image search engines.
    let directURL: URL
}

struct UploadError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

protocol Uploader: Sendable {
    func upload(_ png: Data, fileName: String, progress: @escaping @Sendable (Double) -> Void) async throws -> UploadResult
}

@MainActor
enum Uploaders {
    /// The target chosen in Preferences, with its settings read now on the main actor.
    static func current() throws -> Uploader {
        switch Settings.string(Key.uploadTarget) {
        case "drive":
            guard GoogleAuth.shared.isSignedIn else {
                throw UploadError(message: "Sign in to Google in Snapline Preferences → Upload.")
            }
            return DriveUploader()
        default:
            return try S3Uploader.makeFromSettings()
        }
    }
}

/// Reports request body progress from URLSession.
private final class ProgressDelegate: NSObject, URLSessionTaskDelegate, Sendable {
    let onProgress: @Sendable (Double) -> Void
    init(_ onProgress: @escaping @Sendable (Double) -> Void) { self.onProgress = onProgress }

    func urlSession(_ session: URLSession, task: URLSessionTask, didSendBodyData _: Int64,
                    totalBytesSent: Int64, totalBytesExpectedToSend: Int64) {
        guard totalBytesExpectedToSend > 0 else { return }
        onProgress(Double(totalBytesSent) / Double(totalBytesExpectedToSend))
    }
}

func send(_ request: URLRequest, body: Data, progress: (@Sendable (Double) -> Void)? = nil) async throws -> (Data, HTTPURLResponse) {
    let (data, response) = try await URLSession.shared.upload(for: request, from: body, delegate: progress.map(ProgressDelegate.init))
    guard let http = response as? HTTPURLResponse else { throw UploadError(message: "No HTTP response.") }
    return (data, http)
}

/// A request answered with a non-2xx status. Keeps the status and body so callers can react to specific errors.
struct HTTPError: LocalizedError {
    let action: String
    let status: Int
    let body: Data
    var errorDescription: String? {
        "\(action) failed (HTTP \(status)). " + (String(data: body.prefix(300), encoding: .utf8) ?? "")
    }
}

func checkStatus(_ http: HTTPURLResponse, body: Data, action: String) throws {
    guard (200..<300).contains(http.statusCode) else { throw HTTPError(action: action, status: http.statusCode, body: body) }
}

// MARK: - S3-compatible

struct S3Uploader: Uploader {
    let endpoint: String
    let bucket: String
    let prefix: String
    let publicBaseURL: String
    let signer: SigV4Signer

    @MainActor
    static func makeFromSettings() throws -> S3Uploader {
        let endpoint = Settings.string(Key.s3Endpoint), bucket = Settings.string(Key.s3Bucket)
        let publicBase = Settings.string(Key.s3PublicBaseURL)
        guard S3Location.urls(endpoint: endpoint, bucket: bucket, key: "check.png", publicBaseURL: publicBase) != nil,
              let access = Keychain.get(.s3AccessKey), let secret = Keychain.get(.s3SecretKey) else {
            throw UploadError(message: "Fill in the S3 settings in Snapline Preferences → Upload (HTTPS endpoint, bucket, public URL, keys).")
        }
        let region = Settings.string(Key.s3Region)
        return S3Uploader(endpoint: endpoint, bucket: bucket, prefix: Settings.string(Key.s3Prefix), publicBaseURL: publicBase,
                          signer: SigV4Signer(accessKey: access, secretKey: secret, region: region.isEmpty ? "auto" : region))
    }

    func upload(_ png: Data, fileName: String, progress: @escaping @Sendable (Double) -> Void) async throws -> UploadResult {
        let key = Naming.objectKey(prefix: prefix, date: Date())
        guard let urls = S3Location.urls(endpoint: endpoint, bucket: bucket, key: key, publicBaseURL: publicBaseURL) else {
            throw UploadError(message: "Invalid S3 endpoint or public URL.")
        }
        var request = URLRequest(url: urls.upload)
        request.httpMethod = "PUT"
        let hash = SigV4Signer.sha256Hex(png)
        request.setValue("image/png", forHTTPHeaderField: "Content-Type")
        request.setValue(hash, forHTTPHeaderField: "x-amz-content-sha256")
        request = signer.sign(request, payloadHash: hash, date: Date())

        let (data, http) = try await send(request, body: png, progress: progress)
        try checkStatus(http, body: data, action: "S3 upload")
        return UploadResult(pageURL: urls.public, directURL: urls.public)
    }
}

// MARK: - Google Drive

struct DriveUploader: Uploader {
    private static let api = URL(string: "https://www.googleapis.com/drive/v3")!
    private static let uploadURL = URL(string: "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,webViewLink")!

    func upload(_ png: Data, fileName: String, progress: @escaping @Sendable (Double) -> Void) async throws -> UploadResult {
        let token = try await GoogleAuth.shared.validAccessToken()
        let folder = try await folderID(token: token)

        let boundary = "snapline-\(UUID().uuidString)"
        var request = URLRequest(url: Self.uploadURL)
        request.httpMethod = "POST"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("multipart/related; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        let body = try DriveMultipart.body(boundary: boundary, metadata: DriveFileMetadata(name: fileName, parents: [folder]),
                                           fileData: png, mimeType: "image/png")
        let (data, http) = try await send(request, body: body, progress: progress)
        try checkStatus(http, body: data, action: "Drive upload")
        struct File: Decodable { let id: String; let webViewLink: String }
        let file = try JSONDecoder().decode(File.self, from: data)

        // Anyone with the link can view, like Lightshot.
        let permissions = Self.file(file.id).appending(path: "permissions")
        let (pData, pHTTP) = try await json("POST", permissions, ["type": "anyone", "role": "reader"], token: token)
        try checkStatus(pHTTP, body: pData, action: "Drive sharing")

        guard let page = URL(string: file.webViewLink) else { throw UploadError(message: "Drive returned no link.") }
        return UploadResult(pageURL: page, directURL: DriveMultipart.directImageURL(fileID: file.id))
    }

    /// `files/<id>`, with the id as one path segment (it comes from UserDefaults or the server).
    private static func file(_ id: String) -> URL {
        api.appending(path: "files").appending(path: id, directoryHint: .notDirectory)
    }

    /// The "Snapline" folder in My Drive; created again if it was deleted or trashed.
    private func folderID(token: String) async throws -> String {
        if let id = UserDefaults.standard.string(forKey: Key.driveFolderID) {
            var request = URLRequest(url: Self.file(id).appending(queryItems: [URLQueryItem(name: "fields", value: "id,trashed")]))
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            let (data, response) = try await URLSession.shared.data(for: request)
            struct Folder: Decodable { let trashed: Bool }
            if (response as? HTTPURLResponse)?.statusCode == 200, (try? JSONDecoder().decode(Folder.self, from: data))?.trashed == false {
                return id
            }
        }
        let create = Self.api.appending(path: "files").appending(queryItems: [URLQueryItem(name: "fields", value: "id")])
        let (data, http) = try await json("POST", create, ["name": "Snapline", "mimeType": "application/vnd.google-apps.folder"], token: token)
        try checkStatus(http, body: data, action: "Creating the Snapline folder")
        struct Created: Decodable { let id: String }
        let id = try JSONDecoder().decode(Created.self, from: data).id
        UserDefaults.standard.set(id, forKey: Key.driveFolderID)
        return id
    }

    private func json(_ method: String, _ url: URL, _ object: [String: String], token: String) async throws -> (Data, HTTPURLResponse) {
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        return try await send(request, body: try JSONEncoder().encode(object))
    }
}
