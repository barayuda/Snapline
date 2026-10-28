import Foundation

/// Which languages to read text in. Automatic detection can mistake a line full of symbols for
/// Chinese or Japanese, so use the Mac's own preferred languages instead, with English as a fallback.
public enum RecognitionLanguages {
    /// `preferred`: e.g. `Locale.preferredLanguages`; `supported`: what Vision reports it can read.
    public static func pick(preferred: [String], supported: [String]) -> [String] {
        var result: [String] = []
        for tag in preferred {
            let language = tag.split(separator: "-").first.map(String.init) ?? tag
            let match = supported.first { $0 == tag }
                ?? supported.first { $0.split(separator: "-").first.map(String.init) == language }
            if let match, !result.contains(match) { result.append(match) }
        }
        if !result.contains("en-US"), supported.contains("en-US") { result.append("en-US") }
        return result
    }
}
