import Foundation
import Testing
@testable import SnaplineCore

struct NamingTests {
    private let date: Date

    init() throws {
        var c = DateComponents()
        (c.year, c.month, c.day, c.hour, c.minute, c.second) = (2026, 3, 7, 9, 5, 2)
        date = try #require(Calendar(identifier: .gregorian).date(from: c))
    }

    @Test func screenshotFileNameMatchesLightshotFormat() {
        #expect(Naming.fileName(for: date) == "Screenshot at Mar 07 09-05-02.png")
    }

    @Test func objectKeyHasPrefixYearMonthAndRandomName() throws {
        let key = Naming.objectKey(prefix: "/shots/", date: date)
        let parts = key.split(separator: "/")
        try #require(parts.count == 4)
        #expect(Array(parts.prefix(3)) == ["shots", "2026", "03"])
        #expect(parts[3].hasSuffix(".png"))
        #expect(parts[3].count == 14)   // 10 random characters + ".png"
        #expect(Naming.objectKey(prefix: "shots", date: date) != key)
    }

    @Test func objectKeyWithoutPrefix() {
        #expect(Naming.objectKey(prefix: "", date: date).split(separator: "/").count == 3)
    }

    @Test func uniqueFileURLAddsNumberWhenNameIsTaken() {
        let dir = URL(fileURLWithPath: "/tmp/shots", isDirectory: true)
        let taken: Set<String> = ["/tmp/shots/Screenshot at Mar 07 09-05-02.png", "/tmp/shots/Screenshot at Mar 07 09-05-02 2.png"]
        let free = Naming.uniqueFileURL(in: dir, name: "Screenshot at Mar 07 09-05-01.png") { taken.contains($0.path) }
        #expect(free.lastPathComponent == "Screenshot at Mar 07 09-05-01.png")
        let third = Naming.uniqueFileURL(in: dir, name: "Screenshot at Mar 07 09-05-02.png") { taken.contains($0.path) }
        #expect(third.lastPathComponent == "Screenshot at Mar 07 09-05-02 3.png")
    }
}
