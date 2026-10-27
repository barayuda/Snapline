import XCTest
@testable import SnaplineCore

final class SelectionGeometryTests: XCTestCase {
    func testRectFromTwoPointsInAnyDirection() {
        XCTAssertEqual(SelectionGeometry.rect(from: CGPoint(x: 50, y: 40), to: CGPoint(x: 10, y: 100)),
                       CGRect(x: 10, y: 40, width: 40, height: 60))
    }

    func testVisionBoxToTopLeftViewRect() {
        // Vision: normalized, origin bottom-left. View: points, origin top-left.
        let r = SelectionGeometry.viewRect(fromNormalized: CGRect(x: 0.1, y: 0.2, width: 0.5, height: 0.25),
                                           viewSize: CGSize(width: 1000, height: 800))
        let expected = CGRect(x: 100, y: 440, width: 500, height: 200)
        for (got, want) in [(r.minX, expected.minX), (r.minY, expected.minY), (r.width, expected.width), (r.height, expected.height)] {
            XCTAssertEqual(got, want, accuracy: 0.001)
        }
    }

    func testHorizontalSliceOfWordBoxWithHalfCharacterPadding() {
        // "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE": key is characters 18..<38 of 38.
        let word = CGRect(x: 0, y: 10, width: 380, height: 20)
        let r = SelectionGeometry.slice(word, characters: 18..<38, of: 38)
        XCTAssertEqual(r.minX, 175, accuracy: 0.001)
        XCTAssertEqual(r.maxX, 380, accuracy: 0.001)
        XCTAssertEqual(r.minY, 10)
        XCTAssertEqual(r.height, 20)
        XCTAssertEqual(SelectionGeometry.slice(word, characters: 0..<38, of: 38), word)
    }

    func testMoveIsClampedInsideBounds() {
        let bounds = CGRect(x: 0, y: 0, width: 500, height: 500)
        let moved = SelectionGeometry.move(CGRect(x: 450, y: 10, width: 40, height: 40), by: CGVector(dx: 30, dy: -30), within: bounds)
        XCTAssertEqual(moved, CGRect(x: 460, y: 0, width: 40, height: 40))
    }

    func testHandleHitTestAndResize() {
        let sel = CGRect(x: 100, y: 100, width: 200, height: 100)
        XCTAssertEqual(SelectionGeometry.handle(at: CGPoint(x: 101, y: 99), in: sel, tolerance: 6), .topLeft)
        XCTAssertEqual(SelectionGeometry.handle(at: CGPoint(x: 300, y: 150), in: sel, tolerance: 6), .right)
        XCTAssertNil(SelectionGeometry.handle(at: CGPoint(x: 200, y: 150), in: sel, tolerance: 6))

        let bigger = SelectionGeometry.resize(sel, handle: .bottomRight, to: CGPoint(x: 350, y: 260))
        XCTAssertEqual(bigger, CGRect(x: 100, y: 100, width: 250, height: 160))

        // Dragging a handle past the opposite edge flips instead of producing a negative size.
        let flipped = SelectionGeometry.resize(sel, handle: .left, to: CGPoint(x: 400, y: 0))
        XCTAssertEqual(flipped, CGRect(x: 300, y: 100, width: 100, height: 100))
    }
}
