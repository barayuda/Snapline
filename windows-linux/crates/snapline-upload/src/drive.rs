//! Upload to Google Drive: into a "Snapline" folder in My Drive, shared as "anyone with the link can view".
//! Twin of `DriveUploader` on macOS.

use crate::UploadResult;
use crate::error::UploadError;
use crate::http::{Request, Transport, send_ok};
use serde::Deserialize;
use snapline_core::drive_multipart::{DriveFileMetadata, body, direct_image_url};
use snapline_core::google_oauth::pkce;
use snapline_core::sigv4::SigV4Signer;

const FILES: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD: &str =
    "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,webViewLink";

pub struct DriveUpload {
    pub result: UploadResult,
    /// The Snapline folder used, for the caller to remember.
    pub folder_id: String,
}

/// `files/<id>/<tail…>`, with the id as one path segment (it comes from settings or the server).
fn file_url(id: &str, tail: &[&str]) -> url::Url {
    let mut url = url::Url::parse(FILES).expect("fixed URL");
    url.path_segments_mut()
        .expect("https URL")
        .push(id)
        .extend(tail);
    url
}

fn authorized(request: Request, token: &str) -> Request {
    request.header("Authorization", &format!("Bearer {token}"))
}

fn parse<T: for<'de> Deserialize<'de>>(body: &[u8], what: &str) -> Result<T, UploadError> {
    serde_json::from_slice(body)
        .map_err(|e| UploadError::message(format!("Drive sent an unexpected {what}: {e}")))
}

/// The saved folder if it still exists and is not trashed; otherwise a new one.
fn folder(t: &dyn Transport, token: &str, saved: Option<&str>) -> Result<String, UploadError> {
    #[derive(Deserialize)]
    struct Folder {
        trashed: bool,
    }
    #[derive(Deserialize)]
    struct Created {
        id: String,
    }
    if let Some(id) = saved.filter(|id| !id.is_empty()) {
        let mut url = file_url(id, &[]);
        url.set_query(Some("fields=id,trashed"));
        let response = t.send(&authorized(Request::new("GET", url), token), &|_| {})?;
        if response.status == 200
            && parse::<Folder>(&response.body, "folder").is_ok_and(|f| !f.trashed)
        {
            return Ok(id.to_owned());
        }
    }
    let mut url = url::Url::parse(FILES).expect("fixed URL");
    url.set_query(Some("fields=id"));
    let request = authorized(Request::new("POST", url), token)
        .json(&serde_json::json!({ "name": "Snapline", "mimeType": "application/vnd.google-apps.folder" }));
    let body = send_ok(t, &request, "Creating the Snapline folder", &|_| {})?;
    Ok(parse::<Created>(&body, "folder")?.id)
}

/// Uploads `png` into the Snapline folder and shares it by link.
///
/// # Errors
/// When any of the Drive requests fails.
///
/// # Panics
/// Never: the Drive URLs are fixed.
pub fn upload(
    t: &dyn Transport,
    access_token: &str,
    folder_id: Option<&str>,
    png: &[u8],
    file_name: &str,
    progress: &dyn Fn(f64),
) -> Result<DriveUpload, UploadError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        id: String,
        web_view_link: String,
    }
    let folder = folder(t, access_token, folder_id)?;
    // Hex only: a multipart boundary may not contain "~", which PKCE strings can.
    let boundary = format!(
        "snapline-{}",
        &SigV4Signer::sha256_hex(pkce::make_verifier().as_bytes())[..32]
    );
    let metadata = DriveFileMetadata {
        name: file_name.to_owned(),
        parents: vec![folder.clone()],
    };
    let request = authorized(
        Request::new("POST", url::Url::parse(UPLOAD).expect("fixed URL")),
        access_token,
    )
    .header(
        "Content-Type",
        &format!("multipart/related; boundary={boundary}"),
    )
    .body(body(&boundary, &metadata, png, "image/png"));
    let file: File = parse(&send_ok(t, &request, "Drive upload", progress)?, "file")?;

    let share = authorized(
        Request::new("POST", file_url(&file.id, &["permissions"])),
        access_token,
    )
    .json(&serde_json::json!({ "type": "anyone", "role": "reader" }));
    send_ok(t, &share, "Drive sharing", &|_| {})?;

    let page = url::Url::parse(&file.web_view_link)
        .map_err(|_| UploadError::message("Drive returned no link."))?;
    Ok(DriveUpload {
        result: UploadResult {
            page,
            direct: direct_image_url(&file.id),
        },
        folder_id: folder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeTransport;

    const FILE: &str = r#"{"id":"X1","webViewLink":"https://drive.google.com/file/d/X1/view"}"#;

    #[test]
    fn first_upload_creates_the_folder_uploads_and_shares() {
        let fake = FakeTransport::new(vec![(200, r#"{"id":"F1"}"#), (200, FILE), (200, "{}")]);
        let done = upload(&fake, "TOKEN", None, b"PNG", "Shot.png", &|_| {}).unwrap();
        assert_eq!(done.folder_id, "F1");
        assert_eq!(
            done.result.page.as_str(),
            "https://drive.google.com/file/d/X1/view"
        );
        assert!(done.result.direct.as_str().contains("X1"));
        let sent = fake.requests();
        assert!(
            String::from_utf8_lossy(&sent[0].body).contains("application/vnd.google-apps.folder")
        );
        let upload_type = sent[1]
            .headers
            .iter()
            .find(|(k, _)| k == "Content-Type")
            .unwrap();
        assert!(
            upload_type
                .1
                .starts_with("multipart/related; boundary=snapline-"),
            "{}",
            upload_type.1
        );
        assert!(String::from_utf8_lossy(&sent[1].body).contains(r#""parents":["F1"]"#));
        assert!(sent[2].url.path().ends_with("/files/X1/permissions"));
        assert!(String::from_utf8_lossy(&sent[2].body).contains(r#""type":"anyone""#));
        assert!(sent.iter().all(|r| {
            r.headers
                .contains(&("Authorization".into(), "Bearer TOKEN".into()))
        }));
    }

    #[test]
    fn a_saved_folder_is_reused() {
        let fake = FakeTransport::new(vec![
            (200, r#"{"id":"F1","trashed":false}"#),
            (200, FILE),
            (200, "{}"),
        ]);
        let done = upload(&fake, "TOKEN", Some("F1"), b"PNG", "Shot.png", &|_| {}).unwrap();
        assert_eq!(done.folder_id, "F1");
        assert_eq!(fake.requests().len(), 3);
    }

    #[test]
    fn a_trashed_or_deleted_folder_is_made_again() {
        let fake = FakeTransport::new(vec![
            (200, r#"{"id":"F1","trashed":true}"#),
            (200, r#"{"id":"F2"}"#),
            (200, FILE),
            (200, "{}"),
        ]);
        assert_eq!(
            upload(&fake, "T", Some("F1"), b"PNG", "Shot.png", &|_| {})
                .unwrap()
                .folder_id,
            "F2"
        );
        let fake = FakeTransport::new(vec![
            (404, "{}"),
            (200, r#"{"id":"F3"}"#),
            (200, FILE),
            (200, "{}"),
        ]);
        assert_eq!(
            upload(&fake, "T", Some("F1"), b"PNG", "Shot.png", &|_| {})
                .unwrap()
                .folder_id,
            "F3"
        );
    }

    #[test]
    fn upload_errors_name_the_step() {
        let fake = FakeTransport::new(vec![(200, r#"{"id":"F1"}"#), (403, "quota")]);
        let text = match upload(&fake, "T", None, b"PNG", "Shot.png", &|_| {}) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected an error"),
        };
        assert_eq!(text, "Drive upload failed (HTTP 403). quota");
    }
}
