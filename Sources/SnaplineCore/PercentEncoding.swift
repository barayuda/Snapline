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
    /// Path-style upload URL (`<endpoint>/<bucket>/<key>`, which works for AWS, Cloudflare R2 and MinIO) and the
    /// public link (`<publicBaseURL>/<key>`). Each key segment is percent-encoded once. Returns nil unless the
    /// endpoint is HTTPS (the keys are sent with it) and bucket and public URL are set.
    public static func urls(endpoint: String, bucket: String, key: String, publicBaseURL: String) -> (upload: URL, public: URL)? {
        let slash = CharacterSet(charactersIn: "/")
        let endpoint = endpoint.trimmingCharacters(in: slash), base = publicBaseURL.trimmingCharacters(in: slash)
        guard !bucket.isEmpty, !base.isEmpty,
              let check = URL(string: endpoint), check.scheme == "https", check.host != nil else { return nil }
        let encodedKey = SigV4Signer.canonicalURI(key)
        guard let upload = URL(string: endpoint + "/" + SigV4Signer.canonicalURI(bucket) + "/" + encodedKey),
              let link = URL(string: base + "/" + encodedKey) else { return nil }
        return (upload, link)
    }
}
