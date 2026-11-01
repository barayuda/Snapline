//! Twin of `macOS/Sources/SnaplineCore/DriveMultipart.swift`: request body helpers for the Google Drive v3 API.

use crate::percent_encoding::unreserved;

/// The metadata part of a Drive upload.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DriveFileMetadata {
    pub name: String,
    pub parents: Vec<String>,
}

/// `multipart/related` body for `uploadType=multipart`: JSON metadata, then the file.
///
/// # Panics
/// Never: the metadata is plain strings, which always serialize.
#[must_use]
pub fn body(
    boundary: &str,
    metadata: &DriveFileMetadata,
    file_data: &[u8],
    mime_type: &str,
) -> Vec<u8> {
    let mut body = format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n")
        .into_bytes();
    body.extend(serde_json::to_vec(metadata).expect("metadata is plain strings"));
    body.extend(format!("\r\n--{boundary}\r\nContent-Type: {mime_type}\r\n\r\n").as_bytes());
    body.extend(file_data);
    body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// Undocumented by Google but widely used; image search engines need a direct image URL.
///
/// # Panics
/// Never: the file ID is percent-encoded into a fixed URL.
#[must_use]
pub fn direct_image_url(file_id: &str) -> url::Url {
    url::Url::parse(&format!(
        "https://drive.google.com/uc?export=view&id={}",
        unreserved(file_id)
    ))
    .expect("valid URL")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn multipart_related_body_layout() {
        let png = [0x89, 0x50, 0x4E, 0x47];
        let metadata = DriveFileMetadata {
            name: "Screenshot at Oct 28 10-00-00.png".to_owned(),
            parents: vec!["folder1".to_owned()],
        };
        let body = body("BOUNDARY", &metadata, &png, "image/png");
        let head = b"--BOUNDARY\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n";
        assert!(body.starts_with(head));
        assert!(find(&body, b"\r\n--BOUNDARY\r\nContent-Type: image/png\r\n\r\n").is_some());
        assert!(find(&body, &png).is_some());
        assert!(body.ends_with(b"\r\n--BOUNDARY--\r\n"));

        // The JSON part decodes back to the same name and parent folder.
        let json_end = find(&body, b"\r\n--BOUNDARY\r\n").unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body[head.len()..json_end]).unwrap();
        assert_eq!(json["name"], "Screenshot at Oct 28 10-00-00.png");
        assert_eq!(json["parents"], serde_json::json!(["folder1"]));
    }

    #[test]
    fn direct_image_url_for_a_file() {
        assert_eq!(
            direct_image_url("abc").as_str(),
            "https://drive.google.com/uc?export=view&id=abc"
        );
    }
}
