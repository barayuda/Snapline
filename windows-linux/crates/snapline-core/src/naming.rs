//! Twin of `macOS/Sources/SnaplineCore/Naming.swift`: file names for saved shots and upload keys.

use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};

/// Same pattern as Lightshot: `Screenshot at MMM dd HH-mm-ss.png`, in local time.
#[must_use]
pub fn file_name(date: DateTime<Local>) -> String {
    // chrono's %b is always the English month abbreviation, like DateFormatter with en_US_POSIX.
    format!("Screenshot at {}.png", date.format("%b %d %H-%M-%S"))
}

/// `name` in `dir`, or `name 2`, `name 3`… if taken, so a save never overwrites an earlier shot.
#[must_use]
pub fn unique_file_path(dir: &Path, name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let base = Path::new(name)
        .file_stem()
        .map_or(name.into(), |s| s.to_string_lossy());
    let ext = Path::new(name)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut path = dir.join(name);
    let mut n = 2;
    while exists(&path) {
        path = dir.join(format!("{base} {n}{ext}"));
        n += 1;
    }
    path
}

/// `<prefix>/<yyyy>/<mm>/<10 random chars>.png`. Random, so links cannot be guessed.
#[must_use]
pub fn object_key(prefix: &str, date: DateTime<Local>) -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let name: String = (0..10)
        .map(|_| char::from(ALPHABET[rand::random_range(0..ALPHABET.len())]))
        .collect();
    let month = date.format("%Y/%m").to_string();
    [prefix.trim_matches('/'), &month, &format!("{name}.png")]
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn date() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 3, 7, 9, 5, 2).unwrap()
    }

    #[test]
    fn screenshot_file_name_matches_lightshot_format() {
        assert_eq!(file_name(date()), "Screenshot at Mar 07 09-05-02.png");
    }

    #[test]
    fn object_key_has_prefix_year_month_and_random_name() {
        let key = object_key("/shots/", date());
        let parts: Vec<&str> = key.split('/').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[..3], ["shots", "2026", "03"]);
        assert_eq!(Path::new(parts[3]).extension(), Some("png".as_ref()));
        assert_eq!(parts[3].len(), 14); // 10 random characters + ".png"
        assert_ne!(object_key("shots", date()), key);
    }

    #[test]
    fn object_key_without_prefix() {
        assert_eq!(object_key("", date()).split('/').count(), 3);
    }

    #[test]
    fn unique_file_path_adds_number_when_name_is_taken() {
        let dir = Path::new("/tmp/shots");
        let taken = [
            dir.join("Screenshot at Mar 07 09-05-02.png"),
            dir.join("Screenshot at Mar 07 09-05-02 2.png"),
        ];
        let exists = |p: &Path| taken.iter().any(|t| t == p);
        let free = unique_file_path(dir, "Screenshot at Mar 07 09-05-01.png", exists);
        assert_eq!(
            free.file_name().unwrap(),
            "Screenshot at Mar 07 09-05-01.png"
        );
        let third = unique_file_path(dir, "Screenshot at Mar 07 09-05-02.png", exists);
        assert_eq!(
            third.file_name().unwrap(),
            "Screenshot at Mar 07 09-05-02 3.png"
        );
    }
}
