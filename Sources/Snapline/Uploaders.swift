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
            return try S3Uploader.fromSettings()
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

func checkStatus(_ data: Data, _ http: HTTPURLResponse, _ what: String) throws {
    guard (200..<300).contains(http.statusCode) else {
        let detail = String(data: data.prefix(300), encoding: .utf8) ?? ""
        throw UploadError(message: "\(what) failed (HTTP \(http.statusCode)). \(detail)")
    }
}

// MARK: - S3-compatible

struct S3Uploader: Uploader {
    let endpoint: URL
    let bucket: String
    let prefix: String
    let publicBaseURL: String
    let signer: SigV4Signer

    @MainActor
    static func fromSettings() throws -> S3Uploader {
        let endpoint = Settings.string(Key.s3Endpoint), bucket = Settings.string(Key.s3Bucket)
        let publicBase = Settings.string(Key.s3PublicBaseURL)
        guard let url = URL(string: endpoint), url.scheme == "https", !bucket.isEmpty, !publicBase.isEmpty,
              let access = Keychain.get(.s3AccessKey), let secret = Keychain.get(.s3SecretKey) else {
            throw UploadError(message: "Fill in the S3 settings in Snapline Preferences → Upload (HTTPS endpoint, bucket, public URL, keys).")
        }
        let region = Settings.string(Key.s3Region)
        return S3Uploader(endpoint: url, bucket: bucket, prefix: Settings.string(Key.s3Prefix), publicBaseURL: publicBase,
                          signer: SigV4Signer(accessKey: access, secretKey: secret, region: region.isEmpty ? "auto" : region))
    }

    func upload(_ png: Data, fileName: String, progress: @escaping @Sendable (Double) -> Void) async throws -> UploadResult {
        let key = Naming.objectKey(prefix: prefix, date: Date())
        let encodedKey = SigV4Signer.canonicalURI(key)
        // Path-style URL works for AWS, Cloudflare R2 and MinIO.
        guard let url = URL(string: endpoint.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
                            + "/" + SigV4Signer.canonicalURI(bucket) + "/" + encodedKey),
              let page = URL(string: publicBaseURL.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + "/" + encodedKey) else {
            throw UploadError(message: "Invalid S3 endpoint or public URL.")
        }
        var request = URLRequest(url: url)
        request.httpMethod = "PUT"
        let hash = SigV4Signer.sha256Hex(png)
        request.setValue("image/png", forHTTPHeaderField: "Content-Type")
        request.setValue(hash, forHTTPHeaderField: "x-amz-content-sha256")
        request = signer.sign(request, payloadHash: hash, date: Date())

        let (data, http) = try await send(request, body: png, progress: progress)
        try checkStatus(data, http, "S3 upload")
        return UploadResult(pageURL: page, directURL: page)
    }
}

// MARK: - Google Drive

struct DriveUploader: Uploader {
    private static let api = "https://www.googleapis.com/drive/v3"

    func upload(_ png: Data, fileName: String, progress: @escaping @Sendable (Double) -> Void) async throws -> UploadResult {
        let token = try await GoogleAuth.shared.validAccessToken()
        let folder = try await folderID(token: token)

        let boundary = "snapline-\(UUID().uuidString)"
        var request = URLRequest(url: URL(string: "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,webViewLink")!)
        request.httpMethod = "POST"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("multipart/related; boundary=\(boundary)", forHTTPHeaderField: "Content-Type")
        let body = DriveMultipart.body(boundary: boundary, metadata: ["name": fileName, "parents": [folder]], fileData: png, mimeType: "image/png")
        let (data, http) = try await send(request, body: body, progress: progress)
        try checkStatus(data, http, "Drive upload")
        struct File: Decodable { let id: String; let webViewLink: String }
        let file = try JSONDecoder().decode(File.self, from: data)

        // Anyone with the link can view, like Lightshot.
        let (pData, pHTTP) = try await json("POST", "\(Self.api)/files/\(file.id)/permissions", ["type": "anyone", "role": "reader"], token)
        try checkStatus(pData, pHTTP, "Drive sharing")

        guard let page = URL(string: file.webViewLink) else { throw UploadError(message: "Drive returned no link.") }
        return UploadResult(pageURL: page, directURL: DriveMultipart.directImageURL(fileID: file.id))
    }

    /// The "Snapline" folder in My Drive; created again if it was deleted or trashed.
    private func folderID(token: String) async throws -> String {
        if let id = UserDefaults.standard.string(forKey: Key.driveFolderID) {
            var request = URLRequest(url: URL(string: "\(Self.api)/files/\(id)?fields=id,trashed")!)
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            let (data, response) = try await URLSession.shared.data(for: request)
            struct Folder: Decodable { let trashed: Bool }
            if (response as? HTTPURLResponse)?.statusCode == 200, (try? JSONDecoder().decode(Folder.self, from: data))?.trashed == false {
                return id
            }
        }
        let (data, http) = try await json("POST", "\(Self.api)/files?fields=id",
                                          ["name": "Snapline", "mimeType": "application/vnd.google-apps.folder"], token)
        try checkStatus(data, http, "Creating the Snapline folder")
        struct Created: Decodable { let id: String }
        let id = try JSONDecoder().decode(Created.self, from: data).id
        UserDefaults.standard.set(id, forKey: Key.driveFolderID)
        return id
    }

    private func json(_ method: String, _ url: String, _ object: [String: String], _ token: String) async throws -> (Data, HTTPURLResponse) {
        var request = URLRequest(url: URL(string: url)!)
        request.httpMethod = method
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        return try await send(request, body: try JSONSerialization.data(withJSONObject: object))
    }
}
