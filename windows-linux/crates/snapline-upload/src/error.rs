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
            } => write!(f, "{action} failed (HTTP {status}). {}", summary(body)),
        }
    }
}

impl std::error::Error for UploadError {}

/// The useful part of an error body: S3's `<Code>` and `<Message>`, Google's `error.message`, a note for a web page,
/// or else the first 300 bytes of text. Never the whole body: S3 bodies can carry the access key ID.
fn summary(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let text = text.trim();
    let tag = |name: &str| {
        let start = text.find(&format!("<{name}>"))? + name.len() + 2;
        let end = text[start..].find(&format!("</{name}>"))? + start;
        Some(text[start..end].trim().to_owned())
    };
    if text.starts_with("<?xml") || text.starts_with("<Error>") {
        return match (tag("Code"), tag("Message")) {
            (Some(code), Some(message)) => format!("{code}: {message}"),
            (Some(only), None) | (None, Some(only)) => only,
            (None, None) => "The server sent an error without details.".into(),
        };
    }
    let lower = text.get(..15).unwrap_or(text).to_ascii_lowercase();
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return "The server answered with a web page (a proxy or a sign-in page may be in the way).".into();
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        let error = &json["error"];
        if let Some(message) = error["message"].as_str() {
            return message.to_owned();
        }
        if let Some(code) = error.as_str() {
            return match json["error_description"].as_str() {
                Some(description) => format!("{code}: {description}"),
                None => code.to_owned(),
            };
        }
    }
    let cut = &body[..body.len().min(300)];
    String::from_utf8_lossy(cut).trim().to_owned()
}

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

    fn http(status: u16, body: &str) -> String {
        UploadError::Http {
            action: "S3 upload".into(),
            status,
            body: body.as_bytes().to_vec(),
        }
        .to_string()
    }

    // Review finding: raw XML wasted the message and could show the access key ID.
    #[test]
    fn s3_xml_errors_show_code_and_message_only() {
        let body = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Error><Code>SignatureDoesNotMatch</Code>\
                    <Message>The request signature we calculated does not match.</Message>\
                    <AWSAccessKeyId>AKIDEXAMPLE</AWSAccessKeyId></Error>";
        let text = http(403, body);
        assert_eq!(
            text,
            "S3 upload failed (HTTP 403). SignatureDoesNotMatch: The request signature we calculated does not match."
        );
    }

    #[test]
    fn web_pages_are_described_not_dumped() {
        let text = http(
            502,
            "<!DOCTYPE html><html><head><title>Bad gateway</title></head><body>…</body></html>",
        );
        assert_eq!(
            text,
            "S3 upload failed (HTTP 502). The server answered with a web page (a proxy or a sign-in page may be in the way)."
        );
    }

    #[test]
    fn google_json_errors_show_their_message() {
        let body = r#"{"error":{"code":403,"message":"Insufficient Permission","status":"PERMISSION_DENIED"}}"#;
        assert_eq!(
            http(403, body),
            "S3 upload failed (HTTP 403). Insufficient Permission"
        );
        let body =
            r#"{"error":"invalid_client","error_description":"The OAuth client was not found."}"#;
        assert_eq!(
            http(401, body),
            "S3 upload failed (HTTP 401). invalid_client: The OAuth client was not found."
        );
    }
}
