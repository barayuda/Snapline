import XCTest
@testable import SnaplineCore

final class NamingTests: XCTestCase {
    private let date: Date = {
        var c = DateComponents()
        (c.year, c.month, c.day, c.hour, c.minute, c.second) = (2026, 3, 7, 9, 5, 2)
        return Calendar(identifier: .gregorian).date(from: c)!
    }()

    func testScreenshotFileNameMatchesLightshotFormat() {
        XCTAssertEqual(Naming.fileName(for: date), "Screenshot at Mar 07 09-05-02.png")
    }

    func testObjectKeyHasPrefixYearMonthAndRandomName() {
        let key = Naming.objectKey(prefix: "/shots/", date: date)
        let parts = key.split(separator: "/")
        XCTAssertEqual(parts.count, 4)
        XCTAssertEqual(Array(parts.prefix(3)), ["shots", "2026", "03"])
        XCTAssertTrue(parts[3].hasSuffix(".png"))
        XCTAssertEqual(parts[3].count, 14) // 10 random chars + ".png"
        XCTAssertNotEqual(Naming.objectKey(prefix: "shots", date: date), key)
    }

    func testObjectKeyWithoutPrefix() {
        XCTAssertEqual(Naming.objectKey(prefix: "", date: date).split(separator: "/").count, 3)
    }
}
