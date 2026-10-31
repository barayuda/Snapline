//! What happens to the finished screenshot: copy, save, save as.

use crate::platform;
use crate::settings::Settings;
use chrono::Local;
use snapline_core::naming;
use snapline_overlay::session::Command;
use snapline_overlay::tiny_skia::Pixmap;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// What a finished action did, for the confirmation popup.
#[derive(Debug, PartialEq, Eq)]
pub enum Done {
    Copied { width: u32, height: u32 },
    Saved(PathBuf),
}

/// Runs `command` on the image and says what it did (`None`: nothing to confirm, e.g. Save As cancelled).
/// The error is a sentence to show the user.
pub fn run(command: Command, image: &Pixmap, settings: &Settings) -> Result<Option<Done>, String> {
    let name = naming::file_name(Local::now());
    match command {
        Command::Copy => {
            copy(image).map_err(|e| format!("Snapline could not copy the screenshot: {e}"))?;
            Ok(Some(Done::Copied {
                width: image.width(),
                height: image.height(),
            }))
        }
        Command::Save => {
            let png = png(image)?;
            let path = save(&png, &settings.save_directory, &name).map_err(|e| {
                format!(
                    "Snapline could not save to {}: {e}",
                    settings.save_directory.display()
                )
            })?;
            Ok(Some(Done::Saved(path)))
        }
        Command::SaveAs => {
            let png = png(image)?;
            let Some(path) = rfd::FileDialog::new()
                .set_directory(&settings.save_directory)
                .set_file_name(&name)
                .add_filter("PNG image", &["png"])
                .save_file()
            else {
                return Ok(None); // cancelled
            };
            std::fs::write(&path, png)
                .map_err(|e| format!("Snapline could not save {}: {e}", path.display()))?;
            Ok(Some(Done::Saved(path)))
        }
        Command::Edit => {
            let file = temp_png(&png(image)?, &name)?;
            edit(&file, settings)
                .map_err(|e| {
                    format!(
                        "Snapline could not open an editor. {e} You can also choose one in Snapline settings → General."
                    )
                })?;
            Ok(None)
        }
        Command::Print => {
            let file = temp_png(&png(image)?, &name)?;
            platform::shell_verb("print", &file)
                .map_err(|e| format!("Snapline could not print. {e}"))?;
            Ok(None) // Windows shows its own print dialog
        }
        // Uploads, Share and Copy Text are handled by the app itself; Cancel has nothing to do.
        Command::Cancel
        | Command::Upload
        | Command::Search(_)
        | Command::CopyText
        | Command::Share => Ok(None),
    }
}

/// Writes `png` to the Snapline temp folder for another app to open. Nothing deletes it but Storage Sense, Disk
/// Cleanup or the user.
pub fn temp_png(png: &[u8], name: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("Snapline");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Snapline could not write a temporary file: {e}"))?;
    // Names change once a second, so Print then Edit in the same second must not share a file.
    let path = naming::unique_file_path(&dir, name, Path::exists);
    std::fs::write(&path, png)
        .map_err(|e| format!("Snapline could not write a temporary file: {e}"))?;
    Ok(path)
}

/// What to say when Share cannot open: the screenshot is shown in Explorer instead.
#[must_use]
pub fn share_unavailable(why: &str) -> String {
    format!(
        "Windows could not open Share here ({why}). The screenshot is selected in Explorer, so you can drag it \
         where you need it."
    )
}

/// The editor chosen in Settings, if it still exists; `None` means the Windows default.
#[must_use]
pub fn editor_to_use(settings: &Settings) -> Option<PathBuf> {
    Some(settings.editor.clone()).filter(|p| !p.as_os_str().is_empty() && p.is_file())
}

/// Opens `file` in the chosen editor, or the default one: the `edit` verb (Paint), or `open` when PNG has none.
fn edit(file: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(editor) = editor_to_use(settings) {
        return std::process::Command::new(editor)
            .arg(file)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string());
    }
    platform::shell_verb("edit", file).or_else(|_| platform::shell_verb("open", file))
}

fn png(image: &Pixmap) -> Result<Vec<u8>, String> {
    image
        .encode_png()
        .map_err(|e| format!("Snapline could not make the PNG: {e}"))
}

/// Runs `f` on the app's clipboard. On X11 a copy is served by the app that made it and is gone once its last
/// `Clipboard` is dropped (unless a clipboard manager takes it over), so one stays open for the life of the app.
fn with_clipboard<T>(
    f: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, arboard::Error> {
    static KEPT: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);
    let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
    let clipboard = match kept.as_mut() {
        Some(clipboard) => clipboard,
        None => kept.insert(arboard::Clipboard::new()?),
    };
    f(clipboard)
}

/// The screenshot is opaque, so tiny-skia's premultiplied bytes are already plain RGBA.
fn copy(image: &Pixmap) -> Result<(), arboard::Error> {
    with_clipboard(|c| {
        c.set_image(arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Borrowed(image.data()),
        })
    })
}

/// Puts `text` on the clipboard (Copy Text, upload links); the error says why it could not.
pub fn copy_text(text: &str) -> Result<(), String> {
    with_clipboard(|c| c.set_text(text.to_owned())).map_err(|e| e.to_string())
}

/// Writes `png` into `dir` as `name`, adding " 2", " 3"… instead of overwriting.
pub fn save(png: &[u8], dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = naming::unique_file_path(dir, name, Path::exists);
    std::fs::write(&path, png)?;
    Ok(path)
}

/// The confirmation after Copy Text: "2 lines: first line…" (at most 80 characters of the text).
#[must_use]
pub fn text_copied_detail(text: &str) -> String {
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut shown: String = flat.chars().take(80).collect();
    if flat.chars().count() > 80 {
        shown.push('…');
    }
    let noun = if lines == 1 { "line" } else { "lines" };
    format!("{lines} {noun}: {shown}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("snapline-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = save(b"one", &dir, "Shot.png").unwrap();
        let second = save(b"two", &dir, "Shot.png").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_reports_the_file() {
        let dir = std::env::temp_dir().join(format!("snapline-report-{}", std::process::id()));
        let settings = Settings {
            save_directory: dir.clone(),
            ..Settings::default()
        };
        let image = Pixmap::new(4, 3).unwrap();
        let Some(Done::Saved(path)) = run(Command::Save, &image, &settings).unwrap() else {
            panic!("expected Saved")
        };
        assert!(path.exists() && path.starts_with(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn text_copied_detail_counts_lines_and_shortens() {
        assert_eq!(text_copied_detail("Hello"), "1 line: Hello");
        let long = format!("{}\nsecond", "x".repeat(100));
        let detail = text_copied_detail(&long);
        assert!(
            detail.starts_with("2 lines: ") && detail.chars().count() <= "2 lines: ".len() + 81,
            "{detail}"
        );
    }

    #[test]
    fn temp_png_writes_into_the_snapline_temp_folder() {
        let path = temp_png(b"png bytes", "Snapline test.png").unwrap();
        assert!(path.starts_with(std::env::temp_dir().join("Snapline")));
        assert_eq!(std::fs::read(&path).unwrap(), b"png bytes");
        std::fs::remove_file(path).unwrap();
    }

    // Phase 5c: names change once a second, so Print then Edit in the same second wrote over the same file.
    #[test]
    fn temp_files_never_collide() {
        let name = format!("Snapline collide {}.png", std::process::id());
        let first = temp_png(b"one", &name).unwrap();
        let second = temp_png(b"two", &name).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
    }

    #[test]
    fn a_missing_editor_falls_back_to_the_default() {
        let settings = Settings {
            editor: "/no/such/editor.exe".into(),
            ..Settings::default()
        };
        assert_eq!(
            editor_to_use(&settings),
            None,
            "a missing editor means the default"
        );
        let here = std::env::current_exe().unwrap();
        let settings = Settings {
            editor: here.clone(),
            ..Settings::default()
        };
        assert_eq!(editor_to_use(&settings), Some(here));
        assert_eq!(editor_to_use(&Settings::default()), None);
    }

    // Found in the Xfce check: on X11 the copy lived only as long as the `Clipboard` that made it, so Copy and
    // Copy Text put nothing on the clipboard.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs an X server: CI runs it under xvfb-run"]
    fn linux_copies_stay_on_the_clipboard_after_the_call() {
        let image = Pixmap::new(4, 3).unwrap();
        copy(&image).unwrap();
        let pasted = arboard::Clipboard::new().unwrap().get_image().unwrap();
        assert_eq!((pasted.width, pasted.height), (4, 3));
        copy_text("4111 1111").unwrap();
        assert_eq!(
            arboard::Clipboard::new().unwrap().get_text().unwrap(),
            "4111 1111"
        );
    }

    #[test]
    fn share_fallback_says_where_the_file_is() {
        let text = share_unavailable("class not registered");
        assert!(text.contains("Explorer") && text.contains("class not registered"));
    }
}
