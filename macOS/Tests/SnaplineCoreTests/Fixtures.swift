import Foundation
import Testing

/// Test cases shared with the Rust tests, in `fixtures/` at the repo root.
enum Fixtures {
    static func load<T: Decodable>(_ name: String, as type: T.Type = T.self) -> T {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()                  // SnaplineCoreTests
            .deletingLastPathComponent()                  // Tests
            .deletingLastPathComponent()                  // macOS
            .deletingLastPathComponent()                  // repo root
            .appendingPathComponent("fixtures/\(name)")
        return try! JSONDecoder().decode(T.self, from: Data(contentsOf: url))
    }
}

/// One case from `fixtures/sensitive-text.json`. Text and hidden values are lists of pieces joined with no
/// separator, so fake secrets stay split in the file.
struct SensitiveTextCase: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let text: [String]
    let categories: [String]?
    let hidden: [[String]]
    var testDescription: String { name }
}
