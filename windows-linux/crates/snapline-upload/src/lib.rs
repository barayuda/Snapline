//! Uploads for the Windows and Linux app: S3-compatible buckets and Google Drive, plus Google sign-in.
//! No UI and no secret storage: callers pass settings and secrets in, and keep what comes back.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // progress fractions

pub mod drive;
pub mod error;
pub mod google;
pub mod http;
pub mod s3;

/// Where an upload ended up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadResult {
    /// The link to copy and open.
    pub page: url::Url,
    /// A direct link to the image, for image search.
    pub direct: url::Url,
}
