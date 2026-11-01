import CryptoKit
import Foundation

/// AWS Signature Version 4 for a single request, enough for S3-compatible `PUT`.
/// Signs every header already set on the request plus `host` and `x-amz-date`.
public struct SigV4Signer: Sendable {
    public let accessKey: String
    private let secretKey: String
    public let region: String
    public let service: String

    public init(accessKey: String, secretKey: String, region: String, service: String = "s3") {
        self.accessKey = accessKey
        self.secretKey = secretKey
        self.region = region
        self.service = service
    }

    /// Adds `x-amz-date` and `Authorization` to `request`, signing every header already set plus `host`.
    /// `payloadHash` is the hex SHA-256 of the body (see `sha256Hex`). Returns the request unsigned if its URL has no host.
    public func sign(_ request: URLRequest, payloadHash: String, date: Date) -> URLRequest {
        var request = request
        let amzDate = Self.format(date, "yyyyMMdd'T'HHmmss'Z'")
        let day = String(amzDate.prefix(8))
        request.setValue(amzDate, forHTTPHeaderField: "x-amz-date")

        guard let url = request.url, let host = url.host else { return request }
        var headers: [String: String] = ["host": url.port.map { "\(host):\($0)" } ?? host]
        for (name, value) in request.allHTTPHeaderFields ?? [:] {
            headers[name.lowercased()] = value.trimmingCharacters(in: .whitespaces)
        }
        let names = headers.keys.sorted()
        let signedHeaders = names.joined(separator: ";")

        let canonicalRequest = [
            request.httpMethod ?? "GET",
            Self.canonicalURI(url.path(percentEncoded: false)),
            Self.canonicalQuery(url),
            names.map { "\($0):\(headers[$0]!)\n" }.joined(),
            signedHeaders,
            payloadHash,
        ].joined(separator: "\n")

        let scope = "\(day)/\(region)/\(service)/aws4_request"
        let stringToSign = ["AWS4-HMAC-SHA256", amzDate, scope, Self.sha256Hex(Data(canonicalRequest.utf8))]
            .joined(separator: "\n")

        var key = SymmetricKey(data: Data("AWS4\(secretKey)".utf8))
        for part in [day, region, service, "aws4_request"] {
            key = SymmetricKey(data: Data(HMAC<SHA256>.authenticationCode(for: Data(part.utf8), using: key)))
        }
        let signature = HMAC<SHA256>.authenticationCode(for: Data(stringToSign.utf8), using: key).hex

        request.setValue(
            "AWS4-HMAC-SHA256 Credential=\(accessKey)/\(scope), SignedHeaders=\(signedHeaders), Signature=\(signature)",
            forHTTPHeaderField: "Authorization"
        )
        return request
    }

    public static func sha256Hex(_ data: Data) -> String { SHA256.hash(data: data).hex }

    /// S3 style: each path segment is percent-encoded once with the RFC 3986 unreserved set.
    public static func canonicalURI(_ path: String) -> String {
        guard !path.isEmpty else { return "/" }
        return path.split(separator: "/", omittingEmptySubsequences: false)
            .map { uriEncode(String($0)) }
            .joined(separator: "/")
    }

    static func canonicalQuery(_ url: URL) -> String {
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        return items.map { (uriEncode($0.name), uriEncode($0.value ?? "")) }
            .sorted { $0 < $1 }
            .map { "\($0.0)=\($0.1)" }
            .joined(separator: "&")
    }

    static func uriEncode(_ s: String) -> String { PercentEncoding.unreserved(s) }

    static func format(_ date: Date, _ pattern: String) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = TimeZone(identifier: "UTC")
        f.dateFormat = pattern
        return f.string(from: date)
    }
}

extension Sequence where Element == UInt8 {
    var hex: String { map { String(format: "%02x", $0) }.joined() }
}
