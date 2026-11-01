import Foundation
import Testing
@testable import SnaplineCore

/// Cases from AWS's official Signature Version 4 test suite (awslabs/aws-c-auth, tests/aws-signing-test-suite/v4).
struct SigV4Tests {
    struct Vector: CustomTestStringConvertible, Sendable {
        let name: String
        let method: String
        let url: String
        let headers: [String: String]
        let authorization: String
        var testDescription: String { name }
    }

    static let vectors = [
        Vector(name: "get-vanilla", method: "GET", url: "https://example.amazonaws.com/", headers: [:],
               authorization: "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, "
                   + "SignedHeaders=host;x-amz-date, "
                   + "Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"),
        // Query parameters must be sorted by name in the canonical request.
        Vector(name: "get-vanilla-query-order-key-case", method: "GET",
               url: "https://example.amazonaws.com/?Param2=value2&Param1=value1", headers: [:],
               authorization: "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, "
                   + "SignedHeaders=host;x-amz-date, "
                   + "Signature=b97d918cfa904a5beff61c982a1b6f458b799221646efd99d3219ec94cdf2500"),
        // Extra headers are lowercased, sorted and signed (as the S3 upload does with content-type).
        Vector(name: "post-header-key-sort", method: "POST", url: "https://example.amazonaws.com/",
               headers: ["My-Header1": "value1"],
               authorization: "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, "
                   + "SignedHeaders=host;my-header1;x-amz-date, "
                   + "Signature=c5410059b04c1ee005303aed430f6e6645f61f4dc9e1461ec8f8916fdf18852c"),
    ]

    @Test(arguments: vectors)
    func matchesAWSTestSuite(_ v: Vector) throws {
        let signer = SigV4Signer(accessKey: "AKIDEXAMPLE", secretKey: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
                                 region: "us-east-1", service: "service")
        var request = URLRequest(url: try #require(URL(string: v.url)))
        request.httpMethod = v.method
        for (name, value) in v.headers { request.setValue(value, forHTTPHeaderField: name) }
        let date = try #require(ISO8601DateFormatter().date(from: "2015-08-30T12:36:00Z"))

        let signed = signer.sign(request, payloadHash: SigV4Signer.sha256Hex(Data()), date: date)

        #expect(signed.value(forHTTPHeaderField: "x-amz-date") == "20150830T123600Z")
        #expect(signed.value(forHTTPHeaderField: "Authorization") == v.authorization)
    }

    @Test func canonicalURIEncodesEachPathSegmentOnce() {
        #expect(SigV4Signer.canonicalURI("/bucket/shots/2026/10/a b+c.png") == "/bucket/shots/2026/10/a%20b%2Bc.png")
        #expect(SigV4Signer.canonicalURI("") == "/")
    }

    @Test func sha256OfEmptyPayload() {
        #expect(SigV4Signer.sha256Hex(Data()) == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    }

    @Test(arguments: [
        ("abc-._~XYZ09", "abc-._~XYZ09"),   // unreserved: unchanged
        ("a b", "a%20b"),
        ("a+b/c", "a%2Bb%2Fc"),
        ("é", "%C3%A9"),                    // non-ASCII letters are encoded too
    ])
    func percentEncodingKeepsOnlyUnreserved(input: String, expected: String) {
        #expect(PercentEncoding.unreserved(input) == expected)
    }
}

/// Where an S3-compatible upload goes, and the public link it gets.
struct S3LocationTests {
    @Test func joinsEndpointBucketAndKeyAndEncodesTheKey() throws {
        let urls = try #require(S3Location.urls(endpoint: "https://acc.r2.cloudflarestorage.com/", bucket: "shots",
                                                key: "snapline/2026/10/ab cd.png", publicBaseURL: "https://shots.example.com/"))
        #expect(urls.upload.absoluteString == "https://acc.r2.cloudflarestorage.com/shots/snapline/2026/10/ab%20cd.png")
        #expect(urls.public.absoluteString == "https://shots.example.com/snapline/2026/10/ab%20cd.png")
    }

    @Test func keepsAnEndpointPathAndPort() throws {
        let urls = try #require(S3Location.urls(endpoint: "https://minio.example.com:9000/s3", bucket: "b",
                                                key: "k.png", publicBaseURL: "https://cdn.example.com/b"))
        #expect(urls.upload.absoluteString == "https://minio.example.com:9000/s3/b/k.png")
        #expect(urls.public.absoluteString == "https://cdn.example.com/b/k.png")
    }

    @Test(arguments: ["http://localhost:4566", "http://127.0.0.1:9000", "http://[::1]:9000"])
    func allowsPlainHTTPOnlyOnThisMac(endpoint: String) throws {
        // Loopback never leaves the Mac, so a local MinIO or floci may use http.
        let urls = try #require(S3Location.urls(endpoint: endpoint, bucket: "b", key: "k.png", publicBaseURL: endpoint + "/b"))
        #expect(urls.upload.absoluteString == endpoint + "/b/k.png")
    }

    @Test(arguments: [
        ("http://insecure.example.com", "b", "https://cdn.example.com"),   // HTTPS only: keys are sent with it
        ("http://192.168.1.5:9000", "b", "https://cdn.example.com"),       // other machines need HTTPS, even on the LAN
        ("http://localhost.example.com", "b", "https://cdn.example.com"),   // not loopback
        ("https://ok.example.com", "", "https://cdn.example.com"),         // no bucket
        ("https://ok.example.com", "b", ""),                               // no public URL
        ("not a url", "b", "https://cdn.example.com"),
    ])
    func rejectsIncompleteSettings(endpoint: String, bucket: String, publicBase: String) {
        #expect(S3Location.urls(endpoint: endpoint, bucket: bucket, key: "k.png", publicBaseURL: publicBase) == nil)
    }
}

/// The signed PUT the app sends for an upload (checked offline; see S3IntegrationTests for a live run).
struct S3UploadRequestTests {
    @Test func signedPutCarriesBodyHashTypeAndSignature() throws {
        let png = Data([0x89, 0x50, 0x4E, 0x47])
        let signer = SigV4Signer(accessKey: "AKIDEXAMPLE", secretKey: "secret", region: "auto")
        let date = try #require(ISO8601DateFormatter().date(from: "2026-10-28T06:00:00Z"))
        let upload = try #require(S3Location.uploadRequest(
            png, key: "snapline/2026/10/a b.png", contentType: "image/png",
            endpoint: "https://acc.r2.cloudflarestorage.com", bucket: "shots",
            publicBaseURL: "https://shots.example.com", signer: signer, date: date))

        #expect(upload.request.httpMethod == "PUT")
        #expect(upload.request.url?.absoluteString == "https://acc.r2.cloudflarestorage.com/shots/snapline/2026/10/a%20b.png")
        #expect(upload.publicURL.absoluteString == "https://shots.example.com/snapline/2026/10/a%20b.png")
        #expect(upload.request.value(forHTTPHeaderField: "Content-Type") == "image/png")
        #expect(upload.request.value(forHTTPHeaderField: "x-amz-content-sha256") == SigV4Signer.sha256Hex(png))
        #expect(upload.request.value(forHTTPHeaderField: "x-amz-date") == "20261028T060000Z")
        let auth = try #require(upload.request.value(forHTTPHeaderField: "Authorization"))
        #expect(auth.hasPrefix("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20261028/auto/s3/aws4_request, "))
        #expect(auth.contains("SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature="))
    }

    @Test func noRequestForIncompleteSettings() {
        let signer = SigV4Signer(accessKey: "a", secretKey: "b", region: "auto")
        #expect(S3Location.uploadRequest(Data(), key: "k.png", contentType: "image/png", endpoint: "http://insecure.example.com",
                                         bucket: "b", publicBaseURL: "https://cdn.example.com", signer: signer, date: Date()) == nil)
    }
}
