//! What happens to the finished screenshot: copy, save, save as.

use crate::settings::Settings;
use chrono::Local;
use snapline_core::naming;
use snapline_overlay::session::Command;
use snapline_overlay::tiny_skia::Pixmap;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

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
        // Uploads are handled by the app itself; Cancel has nothing to do.
        Command::Cancel | Command::Upload | Command::Search(_) | Command::CopyText => Ok(None),
    }
}

fn png(image: &Pixmap) -> Result<Vec<u8>, String> {
    image
        .encode_png()
        .map_err(|e| format!("Snapline could not make the PNG: {e}"))
}

/// The screenshot is opaque, so tiny-skia's premultiplied bytes are already plain RGBA.
fn copy(image: &Pixmap) -> Result<(), arboard::Error> {
    arboard::Clipboard::new()?.set_image(arboard::ImageData {
        width: image.width() as usize,
        height: image.height() as usize,
        bytes: Cow::Borrowed(image.data()),
    })
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
}
