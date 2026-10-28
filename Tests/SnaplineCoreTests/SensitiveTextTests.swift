import Testing
@testable import SnaplineCore

struct SensitiveTextTests {
    private func hidden(_ text: String, _ categories: Set<SensitiveCategory> = Set(SensitiveCategory.allCases)) -> [String] {
        SensitiveText.matches(in: text, categories: categories).map { String(text[$0.range]) }
    }

    @Test func email() {
        #expect(hidden("Contact: jane.doe+test@example.co.id today") == ["jane.doe+test@example.co.id"])
    }

    @Test(arguments: ["+62 812-3456-7890", "(415) 555-0132"])
    func phoneNumbers(_ phone: String) {
        #expect(hidden("Call \(phone) now") == [phone])
    }

    // MARK: Payment

    @Test(arguments: [
        "4242 4242 4242 4242",       // 16 digits in groups of 4
        "4222222222222",             // 13 digits, the shortest card length
        "3782 822463 10005",         // American Express grouping 4-6-5
        "4000000000000000006",       // 19 digits, the longest card length
        "4242-4242-4242-4242",
    ])
    func cardNumbersAreHidden(_ card: String) {
        #expect(hidden("Card \(card) exp 12/30", [.payment]) == [card])
    }

    @Test(arguments: [
        "4242-4242-4242-4241",       // fails the Luhn check
        "400000000002",              // 12 digits: too short for a card, even with a valid checksum
        "40000000000000000002",      // 20 digits: too long
    ])
    func notCardNumbers(_ digits: String) {
        #expect(hidden("Number \(digits) here", [.payment]) == [])
    }

    @Test(arguments: [
        ("IBAN GB82 WEST 1234 5698 7654 32 ok", "GB82 WEST 1234 5698 7654 32"),
        ("IBAN GB82 WEST 1234 5698 7654 32 OK", "GB82 WEST 1234 5698 7654 32"),   // a capitalised word after it is trimmed off
        ("IBAN NO93 8601 1117 947 paid", "NO93 8601 1117 947"),                   // shortest (15 characters)
        ("IBAN MT84 MALT 0110 0001 2345 MTLC AST0 01S", "MT84 MALT 0110 0001 2345 MTLC AST0 01S"),   // 31 characters
        ("IBAN DE89370400440532013000 sent", "DE89370400440532013000"),          // without spaces
    ])
    func ibansAreHidden(text: String, iban: String) {
        #expect(hidden(text, [.payment]) == [iban])
    }

    @Test func ibanNeedsValidChecksum() {
        #expect(hidden("IBAN GB83 WEST 1234 5698 7654 32", [.payment]) == [])
    }

    // MARK: Secrets

    @Test(arguments: [
        "AKIAIOSFODNN7EXAMPLE",
        "AIzaSyA-1234567890abcdefghijklmnopqrstu",
        "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
        "github_pat_11ABCDEFG0123456789_abcdefghijklmnop",
        "xoxb-123456789012-abcdefghijkl",
        "sk_live_51Habcdefghijklmnop",
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
        "-----BEGIN RSA PRIVATE KEY-----",
    ])
    func knownTokenFormats(_ token: String) {
        #expect(hidden("value: \(token) end") == [token])
    }

    @Test func secretRunsToEndOfWordWhenTextRecognitionMisreadsACharacter() {
        // Real Vision output: an "I" inside the JWT was read as "|".
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOilxMjMOInO.dozjgNryP4J3jVmNHIOW5N_XgLOn3|9PIFUPOTHsR8U"
        #expect(hidden("Authorization: Bearer \(token) next") == [token])
    }

    @Test func bearerHidesOnlyTheToken() {
        #expect(hidden("Authorization: Bearer abcDEF123456ghiJKL789") == ["abcDEF123456ghiJKL789"])
    }

    @Test(arguments: [
        ("password=hunter22", "hunter22"),
        (#"api_key: "q8Zr2"#, "q8Zr2"),
        ("DB_PASSWORD = s3cr3t!", "s3cr3t!"),
        ("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE", "AKIAIOSFODNN7EXAMPLE"),   // the label stays readable
    ])
    func keyValueHidesOnlyTheValue(text: String, value: String) {
        #expect(hidden(text) == [value])
    }

    @Test func highEntropyString() {
        #expect(hidden("token Zx81kQ0pLm2Vb7Nc4Rt9Ws3Ey6Ua5Io1 here") == ["Zx81kQ0pLm2Vb7Nc4Rt9Ws3Ey6Ua5Io1"])
    }

    // MARK: Not sensitive

    @Test(arguments: [
        "Booking 2026-10-28 for 2 guests, room 1204, total IDR 1.250.000, order #48213, version 2.22",
        "internationalization-localization-configuration",
    ])
    func ordinaryTextIsNotHidden(_ text: String) {
        #expect(hidden(text) == [])
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
