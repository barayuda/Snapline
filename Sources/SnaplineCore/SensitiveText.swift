import Foundation

public enum SensitiveCategory: String, CaseIterable, Sendable {
    case contact, payment, secret, face

    public var title: String {
        switch self {
        case .contact: "Contact info (emails, phone numbers)"
        case .payment: "Payment data (card numbers, IBANs)"
        case .secret: "Secrets (API keys, tokens, passwords)"
        case .face: "Faces"
        }
    }
}

public struct SensitiveMatch: Sendable, Equatable {
    public let range: Range<String.Index>
    public let category: SensitiveCategory
}

/// Finds sensitive values in one line of recognised text.
public enum SensitiveText {
    private struct Rule: @unchecked Sendable {
        let regex: NSRegularExpression
        let category: SensitiveCategory
        let group: Int
        /// Checks the raw match and returns the range to hide, or nil to drop it.
        let resolve: @Sendable (NSString, NSRange) -> NSRange?

        init(_ pattern: String, _ category: SensitiveCategory, group: Int = 0,
             resolve: @escaping @Sendable (NSString, NSRange) -> NSRange? = { _, r in r }) {
            regex = try! NSRegularExpression(pattern: pattern)
            self.category = category
            self.group = group
            self.resolve = resolve
        }

        init(_ pattern: String, _ category: SensitiveCategory, valid: @escaping @Sendable (String) -> Bool) {
            self.init(pattern, category) { ns, r in valid(ns.substring(with: r)) ? r : nil }
        }
    }

    private static let rules: [Rule] = [
        Rule(#"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"#, .contact),
        Rule(#"(?<!\d)(?:\d[ -]?){12,18}\d(?!\d)"#, .payment, valid: isCardNumber),
        Rule(#"\b[A-Z]{2}\d{2}(?: ?[A-Z0-9]){11,30}\b"#, .payment, resolve: ibanMatch),
        Rule(#"eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]*"#, .secret),
        Rule(#"-----BEGIN [A-Z ]*PRIVATE KEY-----"#, .secret),
        Rule(#"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"#, .secret),
        Rule(#"AIza[0-9A-Za-z_-]{35}"#, .secret),
        Rule(#"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})"#, .secret),
        Rule(#"\bxox[abprs]-[A-Za-z0-9-]{10,}"#, .secret),
        Rule(#"\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]{16,}"#, .secret),
        Rule(#"(?i)\bbearer\s+([A-Za-z0-9._~+/-]{16,}=*)"#, .secret, group: 1),
        Rule(#"(?i)\b[A-Za-z_]*(?:password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key|client[_-]?secret)\b\s*[:=]\s*"?([^\s"',;]+)"#,
             .secret, group: 1),
        // "=" only as base64 padding at the end, so `NAME=value` is not read as one token.
        Rule(#"(?<![A-Za-z0-9+/_-])[A-Za-z0-9+/_-]{32,}={0,2}(?![A-Za-z0-9+/_=-])"#, .secret, valid: looksRandom),
    ]

    private static let phoneDetector = try! NSDataDetector(types: NSTextCheckingResult.CheckingType.phoneNumber.rawValue)

    public static func matches(in text: String, categories: Set<SensitiveCategory>) -> [SensitiveMatch] {
        let ns = text as NSString
        let all = NSRange(location: 0, length: ns.length)
        var found: [(NSRange, SensitiveCategory)] = []

        for rule in rules where categories.contains(rule.category) {
            for m in rule.regex.matches(in: text, range: all) {
                let r = m.range(at: rule.group)
                guard r.location != NSNotFound, var hide = rule.resolve(ns, r) else { continue }
                if rule.category == .secret { hide = extendToWordEnd(ns, hide) }
                found.append((hide, rule.category))
            }
        }

        if categories.contains(.contact) {
            for m in phoneDetector.matches(in: text, range: all) {
                let digits = ns.substring(with: m.range).filter(\.isNumber).count
                if (7...15).contains(digits) { found.append((m.range, .contact)) }
            }
        }

        return merge(found).compactMap { r, c in Range(r, in: text).map { SensitiveMatch(range: $0, category: c) } }
    }

    // MARK: - Validators

    static func isCardNumber(_ s: String) -> Bool {
        let digits = s.compactMap(\.wholeNumberValue)
        guard (13...19).contains(digits.count) else { return false }
        let sum = digits.reversed().enumerated().reduce(0) { acc, item in
            let (i, d) = item
            guard i % 2 == 1 else { return acc + d }
            let doubled = d * 2
            return acc + (doubled > 9 ? doubled - 9 : doubled)
        }
        return sum % 10 == 0
    }

    static func isIBAN(_ s: String) -> Bool {
        let compact = s.replacingOccurrences(of: " ", with: "")
        guard (15...34).contains(compact.count) else { return false }
        let rearranged = compact.dropFirst(4) + compact.prefix(4)
        var remainder = 0
        for ch in rearranged {
            guard let v = ch.isNumber ? ch.wholeNumberValue : ch.isLetter ? Int(ch.asciiValue! - 55) : nil else { return false }
            for digit in String(v) { remainder = (remainder * 10 + digit.wholeNumberValue!) % 97 }
        }
        return remainder == 1
    }

    /// The IBAN regex is greedy and can swallow a following capitalised word; drop trailing groups until valid.
    static func ibanMatch(_ ns: NSString, _ range: NSRange) -> NSRange? {
        var candidate = ns.substring(with: range)
        while true {
            if isIBAN(candidate) { return NSRange(location: range.location, length: (candidate as NSString).length) }
            guard let space = candidate.lastIndex(of: " ") else { return nil }
            candidate = String(candidate[..<space])
        }
    }

    /// Long tokens with letters and digits and high Shannon entropy look like generated keys.
    static func looksRandom(_ s: String) -> Bool {
        guard s.contains(where: \.isLetter), s.contains(where: \.isNumber) else { return false }
        let counts = Dictionary(s.map { ($0, 1) }, uniquingKeysWith: +)
        let n = Double(s.count)
        let entropy = counts.values.reduce(0.0) { acc, c in
            let p = Double(c) / n
            return acc - p * log2(p)
        }
        return entropy >= 3.5
    }

    /// Text recognition sometimes misreads one character inside a long token (an "I" as "|"),
    /// which would end the match early. A secret therefore runs to the end of its word.
    /// It is not extended to the left, so a label like `API_KEY=` stays readable.
    static func extendToWordEnd(_ ns: NSString, _ r: NSRange) -> NSRange {
        let rest = NSRange(location: NSMaxRange(r), length: ns.length - NSMaxRange(r))
        let space = ns.rangeOfCharacter(from: .whitespacesAndNewlines, options: [], range: rest)
        let end = space.location == NSNotFound ? ns.length : space.location
        return NSRange(location: r.location, length: end - r.location)
    }

    private static func merge(_ items: [(NSRange, SensitiveCategory)]) -> [(NSRange, SensitiveCategory)] {
        var out: [(NSRange, SensitiveCategory)] = []
        for (r, c) in items.sorted(by: { $0.0.location < $1.0.location }) {
            if let last = out.last, NSIntersectionRange(last.0, r).length > 0 {
                out[out.count - 1].0 = NSUnionRange(last.0, r)
            } else {
                out.append((r, c))
            }
        }
        return out
    }
}
