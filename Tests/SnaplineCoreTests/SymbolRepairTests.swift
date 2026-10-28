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

    func testMismatchedPiecesLeaveWordAlone() {
        // Image has 3 glyphs but the read text has 2 characters: no safe mapping, so no change.
        XCTAssertEqual(repaired("9C", drawn: "⇧⌘C"), "9C")
    }
}
