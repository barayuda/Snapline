//! Twin of `macOS/Sources/SnaplineCore/ShortcutWords.swift`: optional plain-word form of keyboard symbols,
//! for places that cannot show them ("⇧⌘C" → "Shift+Cmd+C").

/// `text` with keyboard-shortcut symbols written as words; arrows and other symbols in prose stay.
#[must_use]
pub fn convert(text: &str) -> String {
    let mut out = String::new();
    let mut token = String::new();
    for ch in text.chars().chain([' ']) {
        if ch.is_whitespace() {
            out += &word(&token);
            token.clear();
            out.push(ch);
        } else {
            token.push(ch);
        }
    }
    out.pop(); // the space added above
    out
}

fn modifier(ch: char) -> Option<&'static str> {
    Some(match ch {
        '⌘' => "Cmd",
        '⇧' => "Shift",
        '⌥' => "Option",
        '⌃' => "Ctrl",
        _ => return None,
    })
}

fn key(ch: char) -> Option<&'static str> {
    Some(match ch {
        '↩' | '⏎' => "Return",
        '⎋' => "Esc",
        '⇥' => "Tab",
        '⇤' => "Shift+Tab",
        '⌫' => "Delete",
        '⌦' => "Forward Delete",
        '⇪' => "Caps Lock",
        '⇞' => "Page Up",
        '⇟' => "Page Down",
        '↖' => "Home",
        '↘' => "End",
        '⏏' => "Eject",
        _ => return None,
    })
}

/// Arrows only count as keys inside a shortcut ("⌘←"); in prose ("A → B") they stay arrows.
fn arrow(ch: char) -> Option<&'static str> {
    Some(match ch {
        '←' => "Left",
        '→' => "Right",
        '↑' => "Up",
        '↓' => "Down",
        _ => return None,
    })
}

const TRAILING: [char; 8] = [',', '.', ';', ':', '!', '?', ')', ']'];

fn word(token: &str) -> String {
    let body = token.trim_end_matches(TRAILING);
    let tail = &token[body.len()..];
    if body.chars().any(|ch| modifier(ch).is_some()) {
        let mut parts: Vec<String> = Vec::new();
        let mut rest = String::new();
        for ch in body.chars() {
            if let Some(name) = modifier(ch).or_else(|| key(ch)).or_else(|| arrow(ch)) {
                if !rest.is_empty() {
                    parts.push(std::mem::take(&mut rest));
                }
                parts.push(name.to_owned());
            } else {
                rest.push(ch);
            }
        }
        if !rest.is_empty() {
            parts.push(rest);
        }
        return parts.join("+") + tail;
    }
    let mut chars = body.chars();
    if let (Some(ch), None) = (chars.next(), chars.next())
        && let Some(name) = key(ch)
    {
        return format!("{name}{tail}");
    }
    token.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_keyboard_symbols_to_words() {
        for (input, expected) in [
            ("press ⇧⌘C, then paste.", "press Shift+Cmd+C, then paste."),
            (
                "Save As ⌥⌘S and ⌃⌥H",
                "Save As Option+Cmd+S and Ctrl+Option+H",
            ),
            (
                "⌘↩ sends, ⌘← goes home",
                "Cmd+Return sends, Cmd+Left goes home",
            ),
            (
                "Press ⎋ to cancel or ↩ to confirm, ⌫ deletes",
                "Press Esc to cancel or Return to confirm, Delete deletes",
            ),
            ("A → B, 5 ≠ 6, done ✓", "A → B, 5 ≠ 6, done ✓"), // arrows and symbols in prose stay
            ("plain text", "plain text"),
        ] {
            assert_eq!(convert(input), expected, "{input}");
        }
    }
}
