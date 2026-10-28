//! Pure logic shared by the Windows and Linux apps: no UI, no OS calls, no network.
//! Each module is the Rust twin of a file in `macOS/Sources/SnaplineCore/`; keep them in step.

pub mod drive_multipart;
pub mod google_oauth;
pub mod percent_encoding;
pub mod sensitive_text;
pub mod sigv4;
