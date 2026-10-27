import Foundation

public enum Naming {
    /// Same pattern as Lightshot: `Screenshot at MMM dd HH-mm-ss.png`, in local time.
    public static func fileName(for date: Date) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "MMM dd HH-mm-ss"
        return "Screenshot at \(f.string(from: date)).png"
    }

    /// `<prefix>/<yyyy>/<mm>/<10 random chars>.png`. Random, so links cannot be guessed.
    public static func objectKey(prefix: String, date: Date) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy/MM"
        let alphabet = Array("abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789")
        let name = String((0..<10).map { _ in alphabet.randomElement()! })
        let trimmed = prefix.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        return [trimmed, f.string(from: date), "\(name).png"].filter { !$0.isEmpty }.joined(separator: "/")
    }
}
