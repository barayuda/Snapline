//! Upload errors and the sentence the user sees.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UploadError {
    /// A sentence for the user.
    Message(String),
    /// A request answered with a status outside 200–299. The body helps people fix their settings.
    Http {
        action: String,
        status: u16,
        body: Vec<u8>,
    },
}

impl UploadError {
    pub fn message(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(text) => f.write_str(text),
            Self::Http {
                action,
                status,
                body,
            } => {
                let start = &body[..body.len().min(300)];
                write!(
                    f,
                    "{action} failed (HTTP {status}). {}",
                    String::from_utf8_lossy(start).trim()
                )
            }
        }
    }
}

impl std::error::Error for UploadError {}

#[cfg(test)]
mod tests {
    use super::*;

    // Review Focus 1.
    #[test]
    fn http_error_message_is_short_and_safe() {
        let body = format!("a{}", "é".repeat(400)).into_bytes(); // 801 bytes, cut inside a character
        let text = UploadError::Http {
            action: "S3 upload".into(),
            status: 403,
            body,
        }
        .to_string();
        assert!(text.starts_with("S3 upload failed (HTTP 403). a"), "{text}");
        assert!(
            text.chars().count() < 340,
            "{} characters",
            text.chars().count()
        );
    }

    #[test]
    fn a_message_is_shown_as_is() {
        assert_eq!(
            UploadError::message("Sign in first.").to_string(),
            "Sign in first."
        );
    }
}
