import Testing
@testable import SnaplineCore

struct RecognitionLanguagesTests {
    private let supported = ["en-US", "fr-FR", "de-DE", "zh-Hans", "ja-JP", "id-ID", "ms-MY", "pt-BR"]

    @Test(arguments: [
        (["en-ID", "id-ID"], ["en-US", "id-ID"]),        // this Mac: English (Indonesia), Indonesian
        (["ja-JP"], ["ja-JP", "en-US"]),                 // English is always added as a fallback
        (["pt-PT"], ["pt-BR", "en-US"]),                 // same language, other region
        (["xx-YY", "en-GB", "en-US"], ["en-US"]),        // unsupported skipped, duplicates removed
    ])
    func picksSupportedLanguages(preferred: [String], expected: [String]) {
        #expect(RecognitionLanguages.pick(preferred: preferred, supported: supported) == expected)
    }
}
