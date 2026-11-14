import Testing
@testable import SnaplineCore

struct SensitiveTextTests {
    private func hidden(_ text: String, _ categories: Set<SensitiveCategory> = Set(SensitiveCategory.allCases)) -> [String] {
        SensitiveText.matches(in: text, categories: categories).map { String(text[$0.range]) }
    }

    struct SharedCases: Decodable { let cases: [SensitiveTextCase] }
    static let shared = Fixtures.load("sensitive-text.json", as: SharedCases.self).cases

    /// Cases shared with the Rust port: both apps must hide exactly the same things.
    @Test(arguments: shared)
    func sharedCase(_ c: SensitiveTextCase) {
        let categories = c.categories.map { Set($0.compactMap(SensitiveCategory.init(rawValue:))) }
            ?? Set(SensitiveCategory.allCases)
        #expect(hidden(c.text.joined(), categories) == c.hidden.map { $0.joined() })
    }

    @Test func categoryFilter() {
        let text = "mail me@x.io key AKIAIOSFODNN7EXAMPLE"
        #expect(hidden(text, [.contact]) == ["me@x.io"])
        #expect(hidden(text, [.secret]) == ["AKIAIOSFODNN7EXAMPLE"])
        #expect(hidden(text, []) == [])
    }

    @Test func overlappingMatchesAreMerged() {
        // The JWT is also a high-entropy string; it must be reported once.
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        #expect(SensitiveText.matches(in: jwt, categories: [.secret]).count == 1)
    }
}
