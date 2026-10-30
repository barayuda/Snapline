//! What happens to the finished screenshot: copy, save, save as.

use crate::settings::Settings;
use chrono::Local;
use snapline_core::naming;
use snapline_overlay::session::Command;
use snapline_overlay::tiny_skia::Pixmap;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Runs `command` on the image. The error is a sentence to show the user.
pub fn run(command: Command, image: &Pixmap, settings: &Settings) -> Result<(), String> {
    let name = naming::file_name(Local::now());
    match command {
        Command::Copy => {
            copy(image).map_err(|e| format!("Snapline could not copy the screenshot: {e}"))
        }
        Command::Save => {
            let png = png(image)?;
            save(&png, &settings.save_directory, &name)
                .map(|_| ())
                .map_err(|e| {
                    format!(
                        "Snapline could not save to {}: {e}",
                        settings.save_directory.display()
                    )
                })
        }
        Command::SaveAs => {
            let png = png(image)?;
            let Some(path) = rfd::FileDialog::new()
                .set_directory(&settings.save_directory)
                .set_file_name(&name)
                .add_filter("PNG image", &["png"])
                .save_file()
            else {
                return Ok(()); // cancelled
            };
            std::fs::write(&path, png)
                .map_err(|e| format!("Snapline could not save {}: {e}", path.display()))
        }
        // Uploads are handled by the app itself (Task 8).
        Command::Cancel | Command::Upload | Command::Search(_) => Ok(()),
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
}
