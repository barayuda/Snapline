//! Settings from `settings.toml` in the user's config folder (`%APPDATA%\Snapline\` on Windows).
//! Phase 2 only reads it; the Preferences window (Phase 5) will write it.

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadTarget {
    #[default]
    S3,
    Drive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Where Ctrl+S saves.
    pub save_directory: PathBuf,
    /// Capture shortcuts, as global-hotkey names (`PrintScreen`, `Ctrl+Shift+Digit1`).
    pub hotkeys: Vec<String>,
    pub tray_click: TrayClick,
    pub upload_target: UploadTarget,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_prefix: String,
    pub s3_public_url: String,
    pub google_client_id: String,
    /// Shown in the settings window after sign-in.
    pub google_email: String,
    /// The Snapline folder in My Drive, once created.
    pub drive_folder_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            save_directory: dirs::desktop_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_default(),
            hotkeys: vec!["PrintScreen".into(), "Ctrl+Shift+Digit1".into()],
            tray_click: TrayClick::Menu,
            upload_target: UploadTarget::S3,
            s3_endpoint: String::new(),
            s3_region: "auto".into(),
            s3_bucket: String::new(),
            s3_prefix: "snapline".into(),
            s3_public_url: String::new(),
            google_client_id: String::new(),
            google_email: String::new(),
            drive_folder_id: String::new(),
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

impl Settings {
    /// Writes the file (through a temporary file, so a crash never leaves half a file).
    ///
    /// # Errors
    /// When the folder or file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    }

    /// Loads the latest file, applies `change` and saves it. Both Snapline processes write this file,
    /// so neither keeps a stale copy. A file that does not parse is left alone.
    ///
    /// # Errors
    /// When the file cannot be read as settings, or cannot be written.
    pub fn update(path: &Path, change: impl FnOnce(&mut Self)) -> Result<Self, String> {
        let (mut settings, problem) = Self::load(path);
        if let Some(problem) = problem {
            return Err(format!(
                "Snapline did not save, because this file could not be read:\n{problem}"
            ));
        }
        change(&mut settings);
        settings.save(path)?;
        Ok(settings)
    }
}

/// How a shortcut name shows in the tray menu: `Ctrl+Shift+Digit1` → `Ctrl+Shift+1`, `PrintScreen` → `PrtScn`.
#[must_use]
pub fn shortcut_label(name: &str) -> String {
    name.replace("PrintScreen", "PrtScn")
        .replace("Digit", "")
        .replace("Key", "")
}

/// What to tell the user about capture shortcuts that could not be registered (`None` when all worked).
/// `failed` and `working` hold shortcut names as in the settings file.
#[must_use]
pub fn shortcut_note(failed: &[String], working: &[String]) -> Option<String> {
    if failed.is_empty() {
        return None;
    }
    let labels = |list: &[String]| {
        list.iter()
            .map(|n| shortcut_label(n))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut note = format!(
        "Snapline could not use {}. Another app is using it.",
        labels(failed)
    );
    if failed.iter().any(|n| n == "PrintScreen") {
        note.push_str(
            "\n\nOn Windows 11 this is usually the Snipping Tool. To give PrtScn to Snapline, turn off Settings → \
             Accessibility → Keyboard → \"Use the Print screen key to open screen capture\", then restart Snapline.",
        );
    }
    if working.is_empty() {
        note.push_str("\n\nYou can still take a screenshot from the tray icon.");
    } else {
        let _ = write!(note, "\n\n{} still works.", labels(working));
    }
    if let Some(path) = Settings::path() {
        let _ = write!(
            note,
            "

To use other keys, change hotkeys in {}.",
            path.display()
        );
    }
    Some(note)
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

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn no_note_when_every_shortcut_works() {
        assert_eq!(
            shortcut_note(&[], &names(&["PrintScreen", "Ctrl+Shift+Digit1"])),
            None
        );
    }

    // The common Windows 11 case: the Snipping Tool owns PrtScn, Ctrl+Shift+1 still works.
    #[test]
    fn taken_prtscn_is_explained_with_the_snipping_tool_fix() {
        let note = shortcut_note(&names(&["PrintScreen"]), &names(&["Ctrl+Shift+Digit1"])).unwrap();
        assert!(
            note.contains("PrtScn")
                && note.contains("Snipping Tool")
                && note.contains("Ctrl+Shift+1"),
            "{note}"
        );
    }

    #[test]
    fn no_working_shortcut_points_to_the_tray() {
        let note = shortcut_note(&names(&["Alt+KeyQ"]), &[]).unwrap();
        assert!(
            note.contains("Alt+Q") && note.contains("tray icon") && !note.contains("Snipping Tool"),
            "{note}"
        );
    }

    #[test]
    fn upload_settings_round_trip_through_the_file() {
        let path = temp_file("roundtrip", "");
        let s = Settings {
            upload_target: UploadTarget::Drive,
            google_client_id: "id.apps.googleusercontent.com".into(),
            ..Settings::default()
        };
        s.save(&path).unwrap();
        let (loaded, problem) = Settings::load(&path);
        assert!(problem.is_none());
        assert_eq!(loaded, s);
        assert_eq!(
            (
                Settings::default().s3_region.as_str(),
                Settings::default().s3_prefix.as_str()
            ),
            ("auto", "snapline")
        );
    }

    #[test]
    fn update_keeps_what_it_does_not_change() {
        let path = temp_file("update", "tray_click = \"capture\"\n");
        Settings::update(&path, |s| s.upload_target = UploadTarget::Drive).unwrap();
        let (s, _) = Settings::load(&path);
        assert_eq!(
            (s.tray_click, s.upload_target),
            (TrayClick::Capture, UploadTarget::Drive)
        );
    }

    // Review Focus 5.
    #[test]
    fn update_refuses_to_overwrite_a_broken_file() {
        let text = "upload_target = \"s3\n";
        let path = temp_file("broken-update", text);
        assert!(Settings::update(&path, |s| s.s3_bucket = "x".into()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}
