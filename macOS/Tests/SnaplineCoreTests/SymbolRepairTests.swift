import CoreGraphics
import Testing
@testable import SnaplineCore

/// Glyph images are drawn with the system font, like the templates. A macOS font update could change the
/// measured table below; if a row breaks after an OS update, re-measure it rather than loosening the check.
///
/// Serialized: drawing symbols makes CoreText ask the font service (fontd) for fallback fonts over a synchronous
/// XPC call. With every test-pool thread doing that at once the run deadlocked (seen on macOS 27); one at a time it
/// takes about 1.5 s. The app draws from a single task, so it is not affected.
@Suite(.serialized)
struct SymbolRepairTests {
    static let dark = (fg: CGFloat(0.92), bg: CGFloat(0.11))
    static let light = (fg: CGFloat(0.05), bg: CGFloat(1.0))

    /// Draws `drawn` like a screenshot shows it, then checks what text recognition `read`.
    private func check(_ read: String, drawn: String, size: CGFloat = 28, colors: (fg: CGFloat, bg: CGFloat) = dark) -> SymbolRepair.Check {
        SymbolRepair.check(read, in: SymbolRepair.render(drawn, size: size, foreground: colors.fg, background: colors.bg))
    }

    private func repaired(_ read: String, drawn: String, size: CGFloat = 28, colors: (fg: CGFloat, bg: CGFloat) = dark) -> String {
        check(read, drawn: drawn, size: size, colors: colors).text
    }

    // MARK: Measured misreads

    struct Misread: CustomTestStringConvertible, Sendable {
        let read: String
        let symbol: String
        var testDescription: String { "\(read) → \(symbol)" }
    }

    /// What Vision read instead of the symbol on screen, measured on macOS 27 at 15 and 26 pt.
    static let measured = [
        ("4", "⇧"), ("D", "⎋"), ("®", "⌦"), ("&", "⇪"), ("|", "⇪"), ("&", "⏎"), ("«", "⇤"), ("I", "⇤"),
        ("#", "⇞"), ("‡", "⇟"), ("&", "↖"), ("y", "↘"), ("&", "⏏"), ("P", "↩"), ("‡", "≠"), ("+", "±"), ("v", "✓"),
        ("→", "⇥"),
    ].map { Misread(read: $0.0, symbol: $0.1) }

    /// Retina pixel sizes of 15 pt and 26 pt text, as on a MacBook screen.
    static let retina: [CGFloat] = [30, 52]

    @Test(arguments: measured, retina)
    func measuredMisreadIsRestoredOnRetina(_ m: Misread, size: CGFloat) {
        #expect(repaired(m.read, drawn: m.symbol, size: size) == m.symbol)
    }

    @Test(arguments: measured)
    func tinyNonRetinaSymbolIsRestoredOrFlaggedNeverWrong(_ m: Misread) {
        let result = check(m.read, drawn: m.symbol, size: 15)
        #expect(result.text == m.symbol || (result.text == m.read && result.isUncertain), "gave \(result)")
    }

    @Test(arguments: Set(measured.map(\.read)).sorted(), [CGFloat(15)] + retina)
    func realLookAlikeCharacterStaysAndIsNotFlagged(_ read: String, size: CGFloat) {
        #expect(check(read, drawn: read, size: size) == SymbolRepair.Check(text: read, isUncertain: false))
    }

    // MARK: Words

    @Test(arguments: [
        ("+9C", "⇧⌘C"), ("+9C,", "⇧⌘C,"), ("9Z", "⌘Z"), ("^TH", "⌃⌥H"),
    ])
    func shortcutSymbolsAreRestored(read: String, drawn: String) {
        #expect(repaired(read, drawn: drawn) == drawn)
    }

    @Test(arguments: [
        "2026", "9C", "T1+V", "x^2", "Version", "4K", "Day", "Pay", "IDE", "y=2", "#42", "A&B", "x|y", "v2",
    ])
    func realWordsAreLeftAlone(_ word: String) {
        #expect(repaired(word, drawn: word) == word)
    }

    @Test func worksOnLightBackgroundAndOtherSizes() {
        #expect(repaired("+9C", drawn: "⇧⌘C", size: 15, colors: Self.light) == "⇧⌘C")
        #expect(repaired("9Z", drawn: "⌘Z", size: 44, colors: Self.light) == "⌘Z")
    }

    @Test func wordWithoutSuspectsIsNotTouched() {
        // "Help" has no suspect characters, so even an image of four ⌘ must not change it.
        #expect(repaired("Help", drawn: "⌘⌘⌘⌘", colors: (1, 0)) == "Help")
    }

    @Test func sliverOfNeighbouringWordAtTheEdgeIsIgnored() throws {
        // Real crops often catch a pixel or two of the next word ("⇧⌘C, then" cut just after the "t" starts).
        let full = SymbolRepair.render("⇧⌘C, then", size: 28, foreground: Self.dark.fg, background: Self.dark.bg)
        let runs = Ink(full).columnRuns()
        try #require(runs.count > 4)
        let crop = try #require(full.cropping(to: CGRect(x: 0, y: 0, width: runs[4].lowerBound + 2, height: full.height)))
        #expect(SymbolRepair.check("+9C,", in: crop).text == "⇧⌘C,")
    }

    @Test func droppedSpaceBetweenSymbolsIsRestored() {
        // Vision read "⌥ ⌃" as one word "T^"; the gap in the image is a space, so put it back.
        #expect(repaired("T^", drawn: "⌥ ⌃", size: 30) == "⌥ ⌃")
        // Shortcut combos are drawn tight and must stay together.
        #expect(repaired("+9C", drawn: "⇧⌘C", size: 30) == "⇧⌘C")
    }

    // MARK: Review flag

    @Test func shortcutWhosePiecesDoNotLineUpIsFlaggedForReview() {
        // A line drawn through the word (or any extra mark) breaks the one-piece-per-character mapping.
        #expect(check("+9C", drawn: "⇧⌘C | ", size: 30) == SymbolRepair.Check(text: "+9C", isUncertain: true))
    }

    @Test(arguments: [
        ("Total", "Tota"),                                   // kerned letters can join
        ("2026", "20 26"),
        ("#42", "#42"),                                      // "#" and "4" touch: 3 characters, 2 pieces
        ("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE", "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE"),   // underscores join letters
    ])
    func ordinaryWordsWithMismatchedPiecesAreNotFlagged(read: String, drawn: String) {
        #expect(!check(read, drawn: drawn, size: 30).isUncertain)
    }

    @Test func codeInMonospacedFontIsNotFlagged() {
        // Inline code (a commit hash) in the monospaced system font, as in a chat app.
        let image = SymbolRepair.render("a21e64e", size: 26, foreground: Self.dark.fg, background: 0.2, font: .userFixedPitch)
        #expect(SymbolRepair.check("a21e64e", in: image) == SymbolRepair.Check(text: "a21e64e", isUncertain: false))
    }

    @Test func mismatchedPiecesLeaveWordAlone() {
        // The image has 3 glyphs but the read text has 2 characters: no safe mapping, so no change.
        #expect(repaired("9C", drawn: "⇧⌘C") == "9C")
    }
}
