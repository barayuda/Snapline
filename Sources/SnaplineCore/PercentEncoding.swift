import Foundation

/// RFC 3986 percent-encoding, shared by request signing, form bodies and search links.
public enum PercentEncoding {
    /// Only the unreserved ASCII characters stay as they are (`A–Z a–z 0–9 - . _ ~`); everything else,
    /// including non-ASCII letters, is encoded as UTF-8 bytes. (`CharacterSet.alphanumerics` would let "é" through.)
    public static func unreserved(_ s: String) -> String {
        s.addingPercentEncoding(withAllowedCharacters: unreservedSet) ?? s
    }

    private static let unreservedSet = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
}

/// Where an S3-compatible upload goes, and the public link it will have.
public enum S3Location {
    /// Plain http is accepted only for these: the keys never leave the Mac (e.g. a local MinIO or floci).
    static let loopbackHosts: Set<String> = ["localhost", "127.0.0.1", "::1", "[::1]"]

    /// Path-style upload URL (`<endpoint>/<bucket>/<key>`, which works for AWS, Cloudflare R2 and MinIO) and the
    /// public link (`<publicBaseURL>/<key>`). Each key segment is percent-encoded once. Returns nil unless the
    /// endpoint is HTTPS (the keys are sent with it; plain http only on this Mac) and bucket and public URL are set.
    public static func urls(endpoint: String, bucket: String, key: String, publicBaseURL: String) -> (upload: URL, public: URL)? {
        let slash = CharacterSet(charactersIn: "/")
        let endpoint = endpoint.trimmingCharacters(in: slash), base = publicBaseURL.trimmingCharacters(in: slash)
        guard !bucket.isEmpty, !base.isEmpty, let check = URL(string: endpoint), let host = check.host,
              check.scheme == "https" || (check.scheme == "http" && loopbackHosts.contains(host)) else { return nil }
        let encodedKey = SigV4Signer.canonicalURI(key)
        guard let upload = URL(string: endpoint + "/" + SigV4Signer.canonicalURI(bucket) + "/" + encodedKey),
              let link = URL(string: base + "/" + encodedKey) else { return nil }
        return (upload, link)
    }

    /// The signed `PUT` that uploads `data` to `key`, plus the public link it will have afterwards.
    /// The body is not attached: pass `data` to the upload call. Returns nil for incomplete settings (see `urls`).
    public static func uploadRequest(_ data: Data, key: String, contentType: String, endpoint: String, bucket: String,
                                     publicBaseURL: String, signer: SigV4Signer, date: Date) -> (request: URLRequest, publicURL: URL)? {
        guard let urls = urls(endpoint: endpoint, bucket: bucket, key: key, publicBaseURL: publicBaseURL) else { return nil }
        var request = URLRequest(url: urls.upload)
        request.httpMethod = "PUT"
        let hash = SigV4Signer.sha256Hex(data)
        request.setValue(contentType, forHTTPHeaderField: "Content-Type")
        request.setValue(hash, forHTTPHeaderField: "x-amz-content-sha256")
        return (signer.sign(request, payloadHash: hash, date: date), urls.public)
    }
}
