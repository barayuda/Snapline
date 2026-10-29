//! Settings from `settings.toml` in the user's config folder (`%APPDATA%\Snapline\` on Windows).
//! Phase 2 only reads it; the Preferences window (Phase 5) will write it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayClick {
    /// Clicking the tray icon opens the menu (the macOS default too).
    #[default]
    Menu,
    /// Clicking the tray icon takes a screenshot, like Lightshot.
    Capture,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Where Ctrl+S saves.
    pub save_directory: PathBuf,
    /// Capture shortcuts, as global-hotkey names (`PrintScreen`, `Ctrl+Shift+Digit1`).
    pub hotkeys: Vec<String>,
    pub tray_click: TrayClick,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            save_directory: dirs::desktop_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_default(),
            hotkeys: vec!["PrintScreen".into(), "Ctrl+Shift+Digit1".into()],
            tray_click: TrayClick::Menu,
        }
    }
}

impl Settings {
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("Snapline").join("settings.toml"))
    }

    /// Reads `path`. A missing file gives the defaults. A file that cannot be read gives the defaults and
    /// the reason, and is left untouched so the user can fix it.
    #[must_use]
    pub fn load(path: &Path) -> (Self, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(settings) => (settings, None),
                Err(e) => (Self::default(), Some(format!("{}:\n{e}", path.display()))),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(e) => (Self::default(), Some(format!("{}: {e}", path.display()))),
        }
    }
}

/// How a shortcut name shows in the tray menu: `Ctrl+Shift+Digit1` → `Ctrl+Shift+1`, `PrintScreen` → `PrtScn`.
#[must_use]
pub fn shortcut_label(name: &str) -> String {
    name.replace("PrintScreen", "PrtScn")
        .replace("Digit", "")
        .replace("Key", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, contents: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snapline-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn defaults_listen_to_prtscn_and_ctrl_shift_1() {
        let s = Settings::default();
        assert_eq!(s.hotkeys, ["PrintScreen", "Ctrl+Shift+Digit1"]);
        assert_eq!(s.tray_click, TrayClick::Menu);
        for key in &s.hotkeys {
            assert!(
                key.parse::<global_hotkey::hotkey::HotKey>().is_ok(),
                "{key} must parse"
            );
        }
    }

    #[test]
    fn missing_file_gives_defaults_without_complaint() {
        let (s, problem) = Settings::load(Path::new("/no/such/snapline/settings.toml"));
        assert_eq!(s, Settings::default());
        assert!(problem.is_none());
    }

    #[test]
    fn a_partial_file_keeps_defaults_for_the_rest() {
        let path = temp_file("partial", "tray_click = \"capture\"\n");
        let (s, problem) = Settings::load(&path);
        assert!(problem.is_none());
        assert_eq!(s.tray_click, TrayClick::Capture);
        assert_eq!(s.hotkeys, Settings::default().hotkeys);
    }

    // Review Focus 3.
    #[test]
    fn broken_settings_file_gives_defaults_and_a_reason() {
        let text = "hotkeys = [\"PrintScreen\"\n";
        let path = temp_file("broken", text);
        let (s, problem) = Settings::load(&path);
        assert_eq!(s, Settings::default());
        assert!(problem.unwrap().contains("settings.toml"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            text,
            "the file must be left as it was"
        );
    }

    #[test]
    fn shortcut_labels_read_the_windows_way() {
        assert_eq!(shortcut_label("PrintScreen"), "PrtScn");
        assert_eq!(shortcut_label("Ctrl+Shift+Digit1"), "Ctrl+Shift+1");
        assert_eq!(shortcut_label("Alt+KeyQ"), "Alt+Q");
    }
}
