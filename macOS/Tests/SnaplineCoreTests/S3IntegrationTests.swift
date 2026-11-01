import Foundation
import Testing
@testable import SnaplineCore

/// Live upload against an S3-compatible server, using the same signed request the app sends.
/// Skipped unless SNAPLINE_S3_TEST_ENDPOINT is set (HTTPS, or http on localhost), e.g. a local floci or MinIO.
/// Use localhost rather than a *.orb.local name: test processes have no Local Network permission.
///
///     SNAPLINE_S3_TEST_ENDPOINT=http://localhost:4566 swift test --filter S3IntegrationTests
///
/// Optional: SNAPLINE_S3_TEST_ACCESS_KEY / _SECRET_KEY (default "test"), _REGION (default "us-east-1").
/// It creates its own bucket and deletes it afterwards. The link is read back without credentials, like a browser would.
@Suite(.enabled(if: ProcessInfo.processInfo.environment["SNAPLINE_S3_TEST_ENDPOINT"] != nil))
struct S3IntegrationTests {
    let env = ProcessInfo.processInfo.environment

    @Test func uploadsAndTheLinkServesTheSameImage() async throws {
        let endpoint = try #require(env["SNAPLINE_S3_TEST_ENDPOINT"])
        let signer = SigV4Signer(accessKey: env["SNAPLINE_S3_TEST_ACCESS_KEY"] ?? "test",
                                 secretKey: env["SNAPLINE_S3_TEST_SECRET_KEY"] ?? "test",
                                 region: env["SNAPLINE_S3_TEST_REGION"] ?? "us-east-1")
        let bucket = "snapline-it-\(UUID().uuidString.prefix(8).lowercased())"
        let bucketURL = try #require(URL(string: endpoint.trimmingCharacters(in: CharacterSet(charactersIn: "/")) + "/" + bucket))

        // A 200 KB body (PNG signature + random bytes) and a key with a space, like real screenshots.
        var png = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        png.append(Data((0..<200_000).map { _ in UInt8.random(in: 0...255) }))
        let key = Naming.objectKey(prefix: "snapline it", date: Date())

        try await send("PUT", bucketURL, signer)   // create the test bucket
        let upload = try #require(S3Location.uploadRequest(
            png, key: key, contentType: "image/png", endpoint: endpoint, bucket: bucket,
            publicBaseURL: bucketURL.absoluteString, signer: signer, date: Date()))
        let (_, put) = try await URLSession.shared.upload(for: upload.request, from: png)
        let (served, get) = try await URLSession.shared.data(from: upload.publicURL)

        // Clean up before checking, so a failed check does not leave the bucket behind.
        let objectURL = try #require(upload.request.url)
        try? await send("DELETE", objectURL, signer)
        try? await send("DELETE", bucketURL, signer)

        #expect((put as? HTTPURLResponse)?.statusCode == 200)
        #expect((get as? HTTPURLResponse)?.statusCode == 200)
        #expect((get as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Type") == "image/png")
        #expect(served == png)
    }

    /// A signed request without a body (bucket and object create/delete).
    private func send(_ method: String, _ url: URL, _ signer: SigV4Signer) async throws {
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue(SigV4Signer.sha256Hex(Data()), forHTTPHeaderField: "x-amz-content-sha256")
        let signed = signer.sign(request, payloadHash: SigV4Signer.sha256Hex(Data()), date: Date())
        let (_, response) = try await URLSession.shared.data(for: signed)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        try #require((200..<300).contains(status), "\(method) \(url.lastPathComponent) answered HTTP \(status)")
    }
}
