import XCTest
@testable import SnaplineCore

final class RecognitionLanguagesTests: XCTestCase {
    private let supported = ["en-US", "fr-FR", "de-DE", "zh-Hans", "ja-JP", "id-ID", "ms-MY", "pt-BR"]

    func testUsesTheMacsPreferredLanguagesMappedToSupportedOnes() {
        // This Mac: English (Indonesia), Indonesian.
        XCTAssertEqual(RecognitionLanguages.pick(preferred: ["en-ID", "id-ID"], supported: supported), ["en-US", "id-ID"])
    }

    func testEnglishIsAlwaysIncludedAsFallback() {
        XCTAssertEqual(RecognitionLanguages.pick(preferred: ["ja-JP"], supported: supported), ["ja-JP", "en-US"])
        XCTAssertEqual(RecognitionLanguages.pick(preferred: ["pt-PT"], supported: supported), ["pt-BR", "en-US"])
    }

    func testUnsupportedLanguagesAreSkippedAndDuplicatesRemoved() {
        XCTAssertEqual(RecognitionLanguages.pick(preferred: ["xx-YY", "en-GB", "en-US"], supported: supported), ["en-US"])
    }
}
