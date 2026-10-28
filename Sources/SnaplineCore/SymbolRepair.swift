import CoreGraphics
import CoreText
import Foundation
import Synchronization

/// Apple's text recognition has no keyboard symbols in its alphabet, so "⇧⌘C" comes back as "+9C".
/// For words with such suspect characters, this cuts the word image into one piece per glyph and
/// compares each suspect piece with drawn symbols and with the character that was read. A symbol
/// replaces the character only when it clearly matches better. If the pieces do not line up one-to-one
/// with the characters, the word is left as it was.
public enum SymbolRepair {
    /// What text recognition returns instead of the symbols below (measured on macOS 27 at 15–44 pt):
    /// ⌘ → 9 % & #, ⇧ → + 4, ⌥ → T 1 2, ⌃ → ^, ⎋ → 5 O D, ⌦ → ® x, ⇪ → & |, ⏎ → &, ⇤ → « I, ⇞ ⇟ → # ‡,
    /// ↖ → &, ↘ → y, ⏏ → &, ↩ → + P, ≠ → ‡ *, ± → +, ✓ → v, ↑ ↓ → 1 V.
    static let suspects: Set<Character> = [
        "+", "9", "T", "1", "2", "^", "V", "v", "5", "O", "o", "<", ">", "t", "8", "X", "x", "*", "→", "←",
        "%", "&", "#", "@", "B", "4", "D", "®", "|", "«", "I", "‡", "y", "P",
    ]
    /// Mac key symbols plus a few text symbols. × and ✗ are left out on purpose: they look too much like a
    /// real "x" to tell apart safely, and a wrong replacement is worse than none.
    static let symbols: [Character] = ["⌘", "⇧", "⌥", "⌃", "↩", "⏎", "⎋", "⇥", "⇤", "⌫", "⌦", "⇪", "⇞", "⇟", "↖", "↘",
                                       "⏏", "↑", "↓", "←", "→", "✓", "≠", "±"]

    /// A symbol must score at least this, and beat the read character by `margin`.
    static let minimumScore: Float = 0.45
    static let margin: Float = 0.10
    /// A glyph nothing was read for (skipped by text recognition) counts as a symbol only with this score,
    /// and this lead over the next-best symbol; otherwise it is reported as unknown.
    static let droppedMinimumScore: Float = 0.6
    static let droppedMargin: Float = 0.08
    /// A symbol that beats the read character but not clearly, scoring at least this, asks for review.
    static let reviewScore: Float = 0.35
    /// Symbols drawn as two separate shapes (a bar and an arrow), so they split into two pieces.
    static let twoPartSymbols: Set<Character> = ["⇤", "⇥"]

    public struct Check: Equatable, Sendable {
        public let text: String
        /// The word may contain a symbol that could not be checked; ask the user to review it.
        public let isUncertain: Bool
    }

    /// The symbols Snapline knows, for the review panel's palette.
    public static var palette: [Character] { symbols }

    public static func hasSuspects(_ word: Substring) -> Bool { word.contains(where: suspects.contains) }

    /// `crop` shows exactly this word (any colours, light or dark background).
    static func repair(_ word: String, in crop: CGImage) -> String { check(word, in: crop).text }

    /// Repairs keyboard symbols misread in `word`, using `crop` (an image of exactly this word, any colours).
    /// `isUncertain` is set when the image clearly shows a symbol that could not be repaired safely.
    public static func check(_ word: String, in crop: CGImage) -> Check {
        let chars = Array(word)
        guard chars.contains(where: suspects.contains) else { return Check(text: word, isUncertain: false) }
        let ink = Ink(crop)
        var pieces = ink.columnRuns()
        // Thin slivers touching the crop edge belong to the neighbouring word.
        let widest = pieces.map(\.count).max() ?? 0
        pieces.removeAll { ($0.lowerBound == 0 || $0.upperBound == ink.width) && $0.count * 4 < widest }

        if pieces.count == chars.count { return classify(chars, pieces, ink).check }
        if pieces.count == chars.count + 1 {
            // ⇤ and ⇥ split into two pieces: try merging each neighbouring pair, and keep the merge only
            // when the merged piece really is one of those symbols.
            for i in 0..<(pieces.count - 1) {
                var merged = pieces
                merged[i] = pieces[i].lowerBound..<pieces[i + 1].upperBound
                merged.remove(at: i + 1)
                let result = classify(chars, merged, ink)
                if let symbol = result.symbols[i], twoPartSymbols.contains(symbol) { return result.check }
            }
        }
        // No safe mapping: leave the word, but ask for review when the image clearly shows a symbol.
        // The piece must also look more like that symbol than like any character of the word itself.
        let ownCharacters = Set(chars)
        let showsSymbol = pieces.contains { piece in
            guard let symbol = clearSymbol(ink, piece), let shape = ink.shape(columns: piece) else { return false }
            let symbolScore = bestScore(shape, symbol)
            return ownCharacters.allSatisfy { bestScore(shape, $0) < symbolScore - margin }
        }
        return Check(text: word, isUncertain: showsSymbol)
    }

    private static func classify(_ chars: [Character], _ pieces: [Range<Int>], _ ink: Ink)
        -> (check: Check, symbols: [Int: Character]) {
        var out = chars.map(String.init)
        var symbols: [Int: Character] = [:]
        var isUncertain = false
        for (i, ch) in chars.enumerated() where suspects.contains(ch) {
            guard let glyph = ink.shape(columns: pieces[i]) else { continue }
            let readScore = bestScore(glyph, ch)
            guard let (symbol, score) = Self.symbols.filter({ $0 != ch })
                .map({ ($0, bestScore(glyph, $0)) })
                .max(by: { $0.1 < $1.1 }) else { continue }
            if score >= minimumScore, score > readScore + margin {
                out[i] = String(symbol)
                symbols[i] = symbol
            } else if score > readScore, score >= reviewScore {
                isUncertain = true
            }
        }
        // Text recognition sometimes drops the space between two symbols ("⌥ ⌃" read as "T^").
        // Put it back when the gap is as wide as a space and a symbol is on either side.
        let spaceGap = CGFloat(ink.inkHeight()) * 0.3
        var text = ""
        for i in out.indices {
            if i > 0, symbols[i] != nil || symbols[i - 1] != nil,
               CGFloat(pieces[i].lowerBound - pieces[i - 1].upperBound) > spaceGap {
                text += " "
            }
            text += out[i]
        }
        return (Check(text: text, isUncertain: isUncertain), symbols)
    }

    /// A glyph between two recognised words that no word covers: text recognition skipped it.
    public struct Dropped: Equatable, Sendable {
        /// Index (in `keptWords`) of the word it follows; -1 when it comes before the first word.
        public let afterWord: Int
        /// The symbol(s) it clearly matches, or nil when unknown (the user should check it).
        public let symbol: String?
    }

    /// `line`: image of one recognised line. `keptWords`: pixel column spans of the words Vision returned.
    public static func droppedSymbols(in line: CGImage, keptWords: [Range<Int>]) -> [Dropped] {
        let ink = Ink(line)
        let lineHeight = ink.inkHeight()
        let spaceGap = Int(Double(lineHeight) * 0.3)
        // Glyph-sized ink outside every kept word (2 px tolerance for tight word boxes).
        let loose = ink.columnRuns().filter { run in
            !keptWords.contains { $0.lowerBound - 2 < run.upperBound && run.lowerBound < $0.upperBound + 2 }
                && ink.inkHeight(columns: run) * 100 >= lineHeight * 35
        }
        // Neighbouring runs closer than a space belong to one dropped word ("⇧⌘", or the two parts of "⇤").
        var groups: [[Range<Int>]] = []
        for run in loose {
            if let last = groups.last?.last, run.lowerBound - last.upperBound < spaceGap {
                groups[groups.count - 1].append(run)
            } else {
                groups.append([run])
            }
        }
        return groups.map { group in
            let afterWord = keptWords.filter { $0.upperBound <= group[0].lowerBound }.count - 1
            // Whole group as one symbol (⇤ ⇥), else each run on its own; any unknown run makes it unknown.
            if group.count > 1, let whole = clearSymbol(ink, group[0].lowerBound..<group[group.count - 1].upperBound) {
                return Dropped(afterWord: afterWord, symbol: String(whole))
            }
            let symbols = group.map { clearSymbol(ink, $0) }
            return Dropped(afterWord: afterWord,
                           symbol: symbols.contains(nil) ? nil : String(symbols.compactMap { $0 }))
        }
    }

    /// A symbol only when it matches well and clearly better than every other symbol.
    private static func clearSymbol(_ ink: Ink, _ columns: Range<Int>) -> Character? {
        guard let shape = ink.shape(columns: columns) else { return nil }
        let ranked = symbols.map { ($0, bestScore(shape, $0)) }.sorted { $0.1 > $1.1 }
        guard ranked[0].1 >= droppedMinimumScore, ranked[0].1 - ranked[1].1 >= droppedMargin else { return nil }
        return ranked[0].0
    }

    // MARK: - Shapes

    /// A glyph normalized to a square grid (aspect kept, centred), plus its width/height ratio.
    struct Shape {
        static let size = 24
        var cells: [Float]
        var aspect: CGFloat
    }

    /// Soft overlap of two shapes (sum of minimums over sum of maximums), damped by aspect difference.
    static func similarity(_ a: Shape, _ b: Shape) -> Float {
        var lo: Float = 0, hi: Float = 0
        for i in a.cells.indices {
            lo += min(a.cells[i], b.cells[i])
            hi += max(a.cells[i], b.cells[i])
        }
        guard hi > 0 else { return 0 }
        let aspect = Float(min(a.aspect, b.aspect) / max(a.aspect, b.aspect))
        return lo / hi * (0.6 + 0.4 * aspect)
    }

    /// Best match against the character drawn in the regular, bold and monospaced system fonts.
    static func bestScore(_ shape: Shape, _ ch: Character) -> Float {
        templates(ch).map { similarity(shape, $0) }.max() ?? 0
    }

    /// Rendered templates per character. Rendering happens outside the lock; a duplicate render in a race is harmless.
    private static let cache = Mutex<[Character: [Shape]]>([:])

    static func templates(_ ch: Character) -> [Shape] {
        if let cached = cache.withLock({ $0[ch] }) { return cached }
        // Monospaced letters and digits for code text ("1" has a flag and a foot there). Not for punctuation:
        // a monospaced "^" is drawn as large as ⌃ and would hide it.
        let fonts: [CTFontUIFontType] = ch.isLetter || ch.isNumber ? [.system, .emphasizedSystem, .userFixedPitch]
                                                                   : [.system, .emphasizedSystem]
        let shapes = fonts.compactMap { type -> Shape? in
            let ink = Ink(render(String(ch), size: 64, foreground: 1, background: 0, font: type))
            return ink.shape(columns: 0..<ink.width)
        }
        cache.withLock { $0[ch] = shapes }
        return shapes
    }

    // MARK: - Drawing (templates and tests)

    /// Grey text on a grey background, `size` points tall, with a small margin. Used for templates and tests.
    static func render(_ text: String, size: CGFloat, foreground: CGFloat, background: CGFloat,
                       font type: CTFontUIFontType = .system) -> CGImage {
        let font = CTFontCreateUIFontForLanguage(type, size, nil)!
        let attrs = [kCTFontAttributeName: font,
                     kCTForegroundColorAttributeName: CGColor(gray: foreground, alpha: 1)] as CFDictionary
        let line = CTLineCreateWithAttributedString(CFAttributedStringCreate(nil, text as CFString, attrs))
        let bounds = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
        let pad = ceil(size * 0.25)
        let w = Int(ceil(bounds.width + pad * 2)), h = Int(ceil(size * 1.4))
        let c = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                          space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
        c.setFillColor(gray: background, alpha: 1)
        c.fill(CGRect(x: 0, y: 0, width: w, height: h))
        c.textPosition = CGPoint(x: pad - bounds.minX, y: size * 0.35)
        CTLineDraw(line, c)
        return c.makeImage()!
    }
}

/// Ink coverage of an image: 0 = background, 1 = full ink, for either light-on-dark or dark-on-light text.
struct Ink {
    let width: Int, height: Int
    private var values: [Float]

    init(_ image: CGImage) {
        width = image.width
        height = image.height
        var gray = [UInt8](repeating: 0, count: width * height)
        // The buffer pointer is only valid inside this closure, so create the context and draw within it.
        let (w, h) = (width, height)
        gray.withUnsafeMutableBytes { buffer in
            let c = CGContext(data: buffer.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w,
                              space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)
            c?.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        }

        // Background = the median of the border pixels.
        var border: [UInt8] = []
        for x in 0..<width { border.append(gray[x]); border.append(gray[(height - 1) * width + x]) }
        for y in 0..<height { border.append(gray[y * width]); border.append(gray[y * width + width - 1]) }
        let background = Float(border.sorted()[border.count / 2])
        let diffs = gray.map { abs(Float($0) - background) }
        let strongest = max(diffs.max() ?? 0, 40)
        values = diffs.map { min($0 / strongest, 1) }
    }

    func at(_ x: Int, _ y: Int) -> Float { values[y * width + x] }

    /// Height of the rows that contain ink (the text height of the word), optionally within some columns.
    func inkHeight(columns: Range<Int>? = nil, threshold: Float = 0.35) -> Int {
        let cols = columns ?? 0..<width
        let rows = (0..<height).filter { y in cols.contains { at($0, y) > threshold } }
        guard let first = rows.first, let last = rows.last else { return 0 }
        return last - first + 1
    }

    /// Runs of columns that contain ink: normally one per glyph.
    func columnRuns(threshold: Float = 0.35) -> [Range<Int>] {
        var runs: [Range<Int>] = []
        var start: Int?
        for x in 0...width {
            let inked = x < width && (0..<height).contains { at(x, $0) > threshold }
            if inked, start == nil { start = x }
            if !inked, let s = start { runs.append(s..<x); start = nil }
        }
        return runs
    }

    /// The ink inside `columns`, cropped to its bounding box and scaled into a square grid.
    func shape(columns: Range<Int>, threshold: Float = 0.35) -> SymbolRepair.Shape? {
        var minY = height, maxY = -1, minX = columns.upperBound, maxX = columns.lowerBound - 1
        for y in 0..<height {
            for x in columns where at(x, y) > threshold {
                minY = min(minY, y); maxY = max(maxY, y); minX = min(minX, x); maxX = max(maxX, x)
            }
        }
        guard maxY >= minY, maxX >= minX else { return nil }
        let w = maxX - minX + 1, h = maxY - minY + 1
        let n = SymbolRepair.Shape.size
        let scale = CGFloat(n) / CGFloat(max(w, h))
        let offX = (CGFloat(n) - CGFloat(w) * scale) / 2, offY = (CGFloat(n) - CGFloat(h) * scale) / 2
        var cells = [Float](repeating: 0, count: n * n)
        var counts = [Float](repeating: 0, count: n * n)
        for y in minY...maxY {
            for x in minX...maxX {
                let gx = min(n - 1, Int(offX + CGFloat(x - minX) * scale))
                let gy = min(n - 1, Int(offY + CGFloat(y - minY) * scale))
                cells[gy * n + gx] += at(x, y)
                counts[gy * n + gx] += 1
            }
        }
        for i in cells.indices where counts[i] > 0 { cells[i] /= counts[i] }
        // Small glyphs leave grid cells unsampled: fill them from the nearest sampled neighbour to the left/top.
        for i in cells.indices where counts[i] == 0 {
            if i % n > 0, counts[i - 1] > 0 { cells[i] = cells[i - 1] } else if i >= n { cells[i] = cells[i - n] }
        }
        return SymbolRepair.Shape(cells: cells, aspect: CGFloat(w) / CGFloat(h))
    }
}
