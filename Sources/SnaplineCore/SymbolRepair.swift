import CoreGraphics
import CoreText
import Foundation

/// Apple's text recognition has no keyboard symbols in its alphabet, so "⇧⌘C" comes back as "+9C".
/// For words with such suspect characters, this cuts the word image into one piece per glyph and
/// compares each suspect piece with drawn symbols and with the character that was read. A symbol
/// replaces the character only when it clearly matches better. If the pieces do not line up one-to-one
/// with the characters, the word is left as it was.
public enum SymbolRepair {
    /// What text recognition returns instead of the symbols below (seen in testing on macOS 27;
    /// ⌘ comes back as "9", "%", "&" or "#" depending on size and crop).
    static let suspects: Set<Character> = ["+", "9", "T", "1", "2", "^", "V", "v", "5", "O", "o", "<", ">", "t", "8", "X", "x", "*", "→", "←", "%", "&", "#", "@", "B"]
    static let symbols: [Character] = ["⌘", "⇧", "⌥", "⌃", "↩", "⎋", "⇥", "⌫", "↑", "↓", "←", "→", "✓"]

    /// A symbol must score at least this, and beat the read character by `margin`.
    static let minimumScore: Float = 0.55
    static let margin: Float = 0.10

    public static func hasSuspects(_ word: Substring) -> Bool { word.contains(where: suspects.contains) }

    /// `crop` shows exactly this word (any colours, light or dark background).
    public static func repair(_ word: String, in crop: CGImage) -> String {
        let chars = Array(word)
        guard chars.contains(where: suspects.contains) else { return word }
        let ink = Ink(crop)
        var pieces = ink.columnRuns()
        // Thin slivers touching the crop edge belong to the neighbouring word.
        let widest = pieces.map(\.count).max() ?? 0
        pieces.removeAll { ($0.lowerBound == 0 || $0.upperBound == ink.width) && $0.count * 4 < widest }
        guard pieces.count == chars.count else { return word }

        var out = chars
        for (i, ch) in chars.enumerated() where suspects.contains(ch) {
            guard let glyph = ink.shape(columns: pieces[i]) else { continue }
            let readScore = bestScore(glyph, ch)
            let best = symbols.filter { $0 != ch }
                .map { ($0, bestScore(glyph, $0)) }
                .max { $0.1 < $1.1 }
            if let (symbol, score) = best, score >= minimumScore, score > readScore + margin {
                out[i] = symbol
            }
        }
        return String(out)
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

    /// Best match against the character drawn in the regular and the bold system font.
    static func bestScore(_ shape: Shape, _ ch: Character) -> Float {
        templates(ch).map { similarity(shape, $0) }.max() ?? 0
    }

    private nonisolated(unsafe) static var cache: [Character: [Shape]] = [:]
    private static let lock = NSLock()

    static func templates(_ ch: Character) -> [Shape] {
        lock.lock()
        defer { lock.unlock() }
        if let cached = cache[ch] { return cached }
        let shapes = [CTFontUIFontType.system, .emphasizedSystem].compactMap { type -> Shape? in
            let ink = Ink(render(String(ch), size: 64, foreground: 1, background: 0, font: type))
            return ink.shape(columns: 0..<ink.width)
        }
        cache[ch] = shapes
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
        let c = CGContext(data: &gray, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width,
                          space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue)!
        c.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))

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
