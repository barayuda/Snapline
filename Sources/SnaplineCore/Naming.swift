import Foundation

public enum Naming {
    /// Same pattern as Lightshot: `Screenshot at MMM dd HH-mm-ss.png`, in local time.
    public static func fileName(for date: Date) -> String {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "MMM dd HH-mm-ss"
        return "Screenshot at \(f.string(from: date)).png"
    }

    /// `name` in `dir`, or `name 2`, `name 3`… if taken, so a save never overwrites an earlier shot.
    public static func uniqueFileURL(in dir: URL, name: String, exists: (URL) -> Bool) -> URL {
        let base = (name as NSString).deletingPathExtension, ext = (name as NSString).pathExtension
        var url = dir.appendingPathComponent(name)
        var n = 2
        while exists(url) {
            url = dir.appendingPathComponent("\(base) \(n).\(ext)")
            n += 1
        }
        return url
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
