import XCTest
@testable import SnaplineCore

final class SensitiveTextTests: XCTestCase {
    private func hidden(_ text: String, _ categories: Set<SensitiveCategory> = Set(SensitiveCategory.allCases)) -> [String] {
        SensitiveText.matches(in: text, categories: categories).map { String(text[$0.range]) }
    }

    func testEmail() {
        XCTAssertEqual(hidden("Contact: jane.doe+test@example.co.id today"), ["jane.doe+test@example.co.id"])
    }

    func testPhoneNumbers() {
        XCTAssertEqual(hidden("Call +62 812-3456-7890 now"), ["+62 812-3456-7890"])
        XCTAssertEqual(hidden("Office (415) 555-0132"), ["(415) 555-0132"])
    }

    func testCardNumberNeedsValidLuhn() {
        XCTAssertEqual(hidden("Card 4242 4242 4242 4242 exp 12/30"), ["4242 4242 4242 4242"])
        XCTAssertEqual(hidden("Card 4242-4242-4242-4241"), []) // fails Luhn
    }

    func testIBANNeedsValidChecksum() {
        XCTAssertEqual(hidden("IBAN GB82 WEST 1234 5698 7654 32 ok"), ["GB82 WEST 1234 5698 7654 32"])
        XCTAssertEqual(hidden("IBAN GB83 WEST 1234 5698 7654 32"), [])
    }

    func testKnownTokenFormats() {
        let samples = [
            "AKIAIOSFODNN7EXAMPLE",
            "AIzaSyA-1234567890abcdefghijklmnopqrstu",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "github_pat_11ABCDEFG0123456789_abcdefghijklmnop",
            "xoxb-123456789012-abcdefghijkl",
            "sk_live_51Habcdefghijklmnop",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
            "-----BEGIN RSA PRIVATE KEY-----",
        ]
        for sample in samples {
            XCTAssertEqual(hidden("value: \(sample) end"), [sample], sample)
        }
    }

    func testSecretRunsToEndOfWordWhenTextRecognitionMisreadsACharacter() {
        // Real Vision output: an "I" inside the JWT was read as "|".
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOilxMjMOInO.dozjgNryP4J3jVmNHIOW5N_XgLOn3|9PIFUPOTHsR8U"
        XCTAssertEqual(hidden("Authorization: Bearer \(token) next"), [token])
    }

    func testBearerHidesOnlyTheToken() {
        XCTAssertEqual(hidden("Authorization: Bearer abcDEF123456ghiJKL789"), ["abcDEF123456ghiJKL789"])
    }

    func testKeyValueHidesOnlyTheValue() {
        XCTAssertEqual(hidden("password=hunter22"), ["hunter22"])
        XCTAssertEqual(hidden(#"api_key: "q8Zr2"#), ["q8Zr2"])
        XCTAssertEqual(hidden("DB_PASSWORD = s3cr3t!"), ["s3cr3t!"])
        // The label must stay readable; only the key is hidden.
        XCTAssertEqual(hidden("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE"), ["AKIAIOSFODNN7EXAMPLE"])
    }

    func testHighEntropyString() {
        XCTAssertEqual(hidden("token Zx81kQ0pLm2Vb7Nc4Rt9Ws3Ey6Ua5Io1 here"), ["Zx81kQ0pLm2Vb7Nc4Rt9Ws3Ey6Ua5Io1"])
    }

    func testOrdinaryTextIsNotHidden() {
        let text = "Booking 2026-10-28 for 2 guests, room 1204, total IDR 1.250.000, order #48213, version 2.22"
        XCTAssertEqual(hidden(text), [])
        XCTAssertEqual(hidden("internationalization-localization-configuration"), [])
    }

    func testCategoryFilter() {
        let text = "mail me@x.io key AKIAIOSFODNN7EXAMPLE"
        XCTAssertEqual(hidden(text, [.contact]), ["me@x.io"])
        XCTAssertEqual(hidden(text, [.secret]), ["AKIAIOSFODNN7EXAMPLE"])
        XCTAssertEqual(hidden(text, []), [])
    }

    func testOverlappingMatchesAreMerged() {
        // The JWT is also a high-entropy string; it must be reported once.
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U"
        XCTAssertEqual(SensitiveText.matches(in: jwt, categories: [.secret]).count, 1)
    }
}
