//! Recording a capture shortcut in the settings window: from keys pressed to a name global-hotkey understands.

use eframe::egui::Modifiers;
use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;

/// The global-hotkey name for the keys pressed ("Ctrl+Shift+1"), or `None` when it cannot be a capture shortcut:
/// no Ctrl or Alt (an F-key may stand alone), or a key global-hotkey does not know. `key` is egui's key name.
/// egui does not report the Windows key, so a recorded shortcut never uses it.
#[must_use]
pub fn from_keys(mods: Modifiers, key: &str) -> Option<String> {
    let function_key = key.len() > 1 && key.starts_with('F') && key[1..].parse::<u8>().is_ok();
    if !(mods.ctrl || mods.alt || function_key) {
        return None;
    }
    let mut name = String::new();
    for (down, part) in [
        (mods.ctrl, "Ctrl"),
        (mods.alt, "Alt"),
        (mods.shift, "Shift"),
    ] {
        if down {
            name.push_str(part);
            name.push('+');
        }
    }
    name.push_str(key);
    name.parse::<HotKey>().is_ok().then_some(name)
}

/// Whether `name` can be a capture shortcut: free, or already one of Snapline's (the tray app holds those).
#[must_use]
pub fn is_free(name: &str, snapline_has: &[String]) -> bool {
    let Ok(key) = name.parse::<HotKey>() else {
        return false;
    };
    if snapline_has
        .iter()
        .any(|n| n.parse::<HotKey>().ok() == Some(key))
    {
        return true;
    }
    let Ok(manager) = GlobalHotKeyManager::new() else {
        return false;
    };
    let free = manager.register(key).is_ok();
    if free {
        let _ = manager.unregister(key);
    }
    free
}

#[cfg(test)]
mod tests {
    use super::*;

    // Review Focus 1.
    #[test]
    fn recorded_shortcuts_need_a_modifier_and_must_parse() {
        let mods = |ctrl, alt, shift| Modifiers {
            ctrl,
            alt,
            shift,
            ..Modifiers::default()
        };
        assert_eq!(
            from_keys(mods(true, false, true), "1"),
            Some("Ctrl+Shift+1".into())
        );
        assert_eq!(
            from_keys(mods(false, false, false), "A"),
            None,
            "no modifier"
        );
        assert_eq!(
            from_keys(mods(false, false, true), "A"),
            None,
            "Shift alone is typing"
        );
        assert_eq!(
            from_keys(mods(false, false, false), "F9"),
            Some("F9".into()),
            "F-keys stand alone"
        );
        assert_eq!(from_keys(mods(true, false, false), "NotAKey"), None);
        assert_eq!(
            from_keys(mods(true, true, false), "S"),
            Some("Ctrl+Alt+S".into())
        );
    }
}
