import CoreGraphics
import XCTest
@testable import SnaplineCore

final class SymbolRepairTests: XCTestCase {
    private let dark = (fg: CGFloat(0.92), bg: CGFloat(0.11))
    private let light = (fg: CGFloat(0.05), bg: CGFloat(1.0))

    /// Draws `text` like a screenshot would show it, then repairs what text recognition read.
    private func repaired(_ read: String, drawn: String, size: CGFloat = 28, colors: (fg: CGFloat, bg: CGFloat)? = nil) -> String {
        let c = colors ?? dark
        let image = SymbolRepair.render(drawn, size: size, foreground: c.fg, background: c.bg)
        return SymbolRepair.repair(read, in: image)
    }

    func testShortcutSymbolsAreRestored() {
        XCTAssertEqual(repaired("+9C", drawn: "⇧⌘C"), "⇧⌘C")
        XCTAssertEqual(repaired("+9C,", drawn: "⇧⌘C,"), "⇧⌘C,")
        XCTAssertEqual(repaired("9Z", drawn: "⌘Z"), "⌘Z")
        XCTAssertEqual(repaired("^TH", drawn: "⌃⌥H"), "⌃⌥H")
    }

    /// (what Vision read, the symbol on screen), measured on macOS 27 at 15 and 26 pt.
    private let measured: [(read: String, symbol: String)] = [
        ("4", "⇧"), ("D", "⎋"), ("®", "⌦"), ("&", "⇪"), ("|", "⇪"), ("&", "⏎"), ("«", "⇤"), ("I", "⇤"),
        ("#", "⇞"), ("‡", "⇟"), ("&", "↖"), ("y", "↘"), ("&", "⏏"), ("P", "↩"), ("‡", "≠"), ("+", "±"), ("v", "✓"),
        ("→", "⇥"),
    ]

    /// Retina pixel sizes of 15 pt and 26 pt text, as on a MacBook screen.
    private let retina: [CGFloat] = [30, 52]

    func testEveryMeasuredMisreadIsRestoredOnRetina() {
        for (read, symbol) in measured {
            for size in retina {
                XCTAssertEqual(repaired(read, drawn: symbol, size: size), symbol, "\(read) → \(symbol) at \(size) px")
            }
        }
    }

    func testTinyNonRetinaSymbolsAreRestoredOrFlaggedNeverWrong() {
        for (read, symbol) in measured {
            let result = SymbolRepair.check(read, in: SymbolRepair.render(symbol, size: 15, foreground: dark.fg, background: dark.bg))
            XCTAssertTrue(result.text == symbol || (result.text == read && result.uncertain),
                          "\(read) → \(symbol) at 15 px gave \(result.text), uncertain \(result.uncertain)")
        }
    }

    func testTheRealLookAlikeCharactersStayAsTheyAre() {
        for read in Set(measured.map(\.read)) {
            for size in [CGFloat(15)] + retina {
                let result = SymbolRepair.check(read, in: SymbolRepair.render(read, size: size, foreground: dark.fg, background: dark.bg))
                XCTAssertEqual(result.text, read, "real \(read) at \(size) px")
                XCTAssertFalse(result.uncertain, "real \(read) at \(size) px should not ask for review")
            }
        }
        // And in ordinary words.
        for word in ["4K", "Day", "Pay", "IDE", "y=2", "#42", "A&B", "x|y", "v2"] {
            XCTAssertEqual(repaired(word, drawn: word), word)
        }
    }

    func testWorksOnLightBackgroundAndOtherSizes() {
        XCTAssertEqual(repaired("+9C", drawn: "⇧⌘C", size: 15, colors: light), "⇧⌘C")
        XCTAssertEqual(repaired("9Z", drawn: "⌘Z", size: 44, colors: light), "⌘Z")
    }

    func testRealCharactersAreLeftAlone() {
        XCTAssertEqual(repaired("2026", drawn: "2026"), "2026")
        XCTAssertEqual(repaired("9C", drawn: "9C"), "9C")
        XCTAssertEqual(repaired("T1+V", drawn: "T1+V"), "T1+V")
        XCTAssertEqual(repaired("x^2", drawn: "x^2"), "x^2")
        XCTAssertEqual(repaired("Version", drawn: "Version"), "Version")
    }

    func testWordWithoutSuspectsIsNotTouched() {
        // "Help" has no suspect characters, so even an image of four ⌘ must not change it.
        XCTAssertEqual(SymbolRepair.repair("Help", in: SymbolRepair.render("⌘⌘⌘⌘", size: 28, foreground: 1, background: 0)), "Help")
    }

    func testSliverOfNeighbouringWordAtTheEdgeIsIgnored() {
        // Real crops often catch a pixel or two of the next word ("⇧⌘C, then" cut just after the "t" starts).
        let full = SymbolRepair.render("⇧⌘C, then", size: 28, foreground: dark.fg, background: dark.bg)
        let runs = Ink(full).columnRuns()
        let tStart = runs[4].lowerBound
        let crop = full.cropping(to: CGRect(x: 0, y: 0, width: tStart + 2, height: full.height))!
        XCTAssertEqual(SymbolRepair.repair("+9C,", in: crop), "⇧⌘C,")
    }

    func testDroppedSpaceBetweenSymbolsIsRestored() {
        // Vision read "⌥ ⌃" as one word "T^"; the gap in the image is a space, so put it back.
        XCTAssertEqual(repaired("T^", drawn: "⌥ ⌃", size: 30), "⌥ ⌃")
        // Shortcut combos are drawn tight and must stay together.
        XCTAssertEqual(repaired("+9C", drawn: "⇧⌘C", size: 30), "⇧⌘C")
    }

    func testShortcutWhosePiecesDoNotLineUpIsFlaggedForReview() {
        // A line drawn through the word (or any extra mark) breaks the one-piece-per-character mapping.
        let image = SymbolRepair.render("⇧⌘C | ", size: 30, foreground: dark.fg, background: dark.bg)
        let result = SymbolRepair.check("+9C", in: image)
        XCTAssertEqual(result.text, "+9C")
        XCTAssertTrue(result.uncertain)
    }

    func testOrdinaryWordsWithMismatchedPiecesAreNotFlagged() {
        // Kerned or joined letters can also break the mapping; normal words must not trigger the review panel.
        XCTAssertFalse(SymbolRepair.check("Total", in: SymbolRepair.render("Tota", size: 30, foreground: dark.fg, background: dark.bg)).uncertain)
        XCTAssertFalse(SymbolRepair.check("2026", in: SymbolRepair.render("20 26", size: 30, foreground: dark.fg, background: dark.bg)).uncertain)
        // "#" and "4" touch, so 3 characters give 2 pieces; neither piece looks like a symbol.
        XCTAssertFalse(SymbolRepair.check("#42", in: SymbolRepair.render("#42", size: 30, foreground: dark.fg, background: dark.bg)).uncertain)
        // Underscores join letters into bigger pieces; letters must not be taken for symbols.
        let key = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE"
        XCTAssertFalse(SymbolRepair.check(key, in: SymbolRepair.render(key, size: 30, foreground: dark.fg, background: dark.bg)).uncertain)
    }

    func testCodeInMonospacedFontIsNotFlagged() {
        // Inline code (a commit hash) in the monospaced system font, as in a chat app.
        let image = SymbolRepair.render("a21e64e", size: 26, foreground: dark.fg, background: 0.2, font: .userFixedPitch)
        XCTAssertEqual(SymbolRepair.check("a21e64e", in: image), SymbolRepair.Check(text: "a21e64e", uncertain: false))
    }

    func testMismatchedPiecesLeaveWordAlone() {
        // Image has 3 glyphs but the read text has 2 characters: no safe mapping, so no change.
        XCTAssertEqual(repaired("9C", drawn: "⇧⌘C"), "9C")
    }
}
