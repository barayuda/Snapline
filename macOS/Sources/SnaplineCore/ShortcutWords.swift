/// Optional plain-word form of keyboard symbols, for places that cannot show them ("⇧⌘C" → "Shift+Cmd+C").
public enum ShortcutWords {
    static let modifiers: [Character: String] = ["⌘": "Cmd", "⇧": "Shift", "⌥": "Option", "⌃": "Ctrl"]
    static let keys: [Character: String] = [
        "↩": "Return", "⏎": "Return", "⎋": "Esc", "⇥": "Tab", "⇤": "Shift+Tab", "⌫": "Delete", "⌦": "Forward Delete",
        "⇪": "Caps Lock", "⇞": "Page Up", "⇟": "Page Down", "↖": "Home", "↘": "End", "⏏": "Eject",
    ]
    /// Arrows only count as keys inside a shortcut ("⌘←"); in prose ("A → B") they stay arrows.
    static let arrows: [Character: String] = ["←": "Left", "→": "Right", "↑": "Up", "↓": "Down"]
    static let trailing: Set<Character> = [",", ".", ";", ":", "!", "?", ")", "]"]

    /// `text` with keyboard-shortcut symbols written as words; arrows and other symbols in prose stay.
    public static func convert(_ text: String) -> String {
        var out = ""
        var token = ""
        for ch in text + " " {
            if ch.isWhitespace {
                out += word(token)
                token = ""
                out.append(ch)
            } else {
                token.append(ch)
            }
        }
        return String(out.dropLast())
    }

    private static func word(_ token: String) -> String {
        var body = Substring(token), tail = ""
        while let last = body.last, trailing.contains(last) { tail = String(last) + tail; body = body.dropLast() }
        if body.contains(where: { modifiers[$0] != nil }) {
            var parts: [String] = []
            var key = ""
            for ch in body {
                if let name = modifiers[ch] ?? keys[ch] ?? arrows[ch] {
                    if !key.isEmpty { parts.append(key); key = "" }
                    parts.append(name)
                } else {
                    key.append(ch)
                }
            }
            if !key.isEmpty { parts.append(key) }
            return parts.joined(separator: "+") + tail
        }
        if body.count == 1, let ch = body.first, let name = keys[ch] { return name + tail }
        return token
    }
}
