//! Pure logic shared by the Windows and Linux apps: no UI, no OS calls, no network.
//! Each module is the Rust twin of a file in `macOS/Sources/SnaplineCore/`; keep them in step.

pub mod drive_multipart;
pub mod google_oauth;
pub mod naming;
pub mod percent_encoding;
pub mod reading_order;
pub mod recognition_languages;
pub mod search;
pub mod selection_geometry;
pub mod sensitive_text;
pub mod shortcut_words;
pub mod sigv4;
pub mod uncertain_marks;
