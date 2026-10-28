import Foundation

/// Invisible markers (Unicode private-use characters) around text the user should check.
/// They travel through ReadingOrder and are removed before anything reaches the clipboard.
public enum UncertainMarks {
    static let open: Character = "\u{E000}"
    static let close: Character = "\u{E001}"

    /// `s` between the markers.
    public static func wrap(_ s: String) -> String { "\(open)\(s)\(close)" }
    /// Whether `s` has any marked spot.
    public static func contains(_ s: String) -> Bool { s.contains(open) }
    /// `s` without markers (what goes to the clipboard).
    public static func strip(_ s: String) -> String { s.filter { $0 != open && $0 != close } }

    /// Plain text plus the ranges (in the plain text, UTF-16) that were marked.
    public static func extract(_ s: String) -> (text: String, ranges: [NSRange]) {
        var plain = ""
        var ranges: [NSRange] = []
        var start: Int?
        for ch in s {
            if ch == open {
                start = (plain as NSString).length
            } else if ch == close, let st = start {
                ranges.append(NSRange(location: st, length: (plain as NSString).length - st))
                start = nil
            } else {
                plain.append(ch)
            }
        }
        return (plain, ranges)
    }
}
