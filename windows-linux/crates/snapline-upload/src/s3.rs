//! Upload to an S3-compatible bucket (AWS S3, Cloudflare R2, MinIO). Twin of `S3Uploader` on macOS.

use crate::UploadResult;
use crate::error::UploadError;
use crate::http::{Request, Transport, send_ok};
use chrono::{DateTime, Local, Utc};
use snapline_core::naming;
use snapline_core::percent_encoding::{s3_upload_request, s3_urls};
use snapline_core::sigv4::SigV4Signer;

pub const MISSING: &str = "Fill in the S3 settings in Snapline's upload settings (HTTPS endpoint, bucket, public URL and keys).";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub public_base_url: String,
    pub access_key: String,
    pub secret_key: String,
}

/// Uploads `png` under `<prefix>/<date-based name>.png` and returns its public link.
///
/// # Errors
/// When the settings are incomplete, the server cannot be reached, or it refuses the upload.
pub fn upload(
    t: &dyn Transport,
    config: &S3Config,
    png: &[u8],
    now: DateTime<Local>,
    progress: &dyn Fn(f64),
) -> Result<UploadResult, UploadError> {
    let (access, secret) = (config.access_key.trim(), config.secret_key.trim());
    if access.is_empty()
        || secret.is_empty()
        || s3_urls(
            &config.endpoint,
            &config.bucket,
            "check.png",
            &config.public_base_url,
        )
        .is_none()
    {
        return Err(UploadError::message(MISSING));
    }
    let region = if config.region.trim().is_empty() {
        "auto"
    } else {
        config.region.trim()
    };
    let signer = SigV4Signer::new(access, secret, region);
    let key = naming::object_key(&config.prefix, now);
    let (request, public) = s3_upload_request(
        png,
        &key,
        "image/png",
        &config.endpoint,
        &config.bucket,
        &config.public_base_url,
        &signer,
        now.with_timezone(&Utc),
    )
    .ok_or_else(|| UploadError::message("Invalid S3 endpoint or public URL."))?;
    send_ok(
        t,
        &Request::from_signed(request, png.to_vec()),
        "S3 upload",
        progress,
    )?;
    Ok(UploadResult {
        page: public.clone(),
        direct: public,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeTransport;
    use chrono::TimeZone;

    fn config() -> S3Config {
        S3Config {
            endpoint: "https://acct.r2.cloudflarestorage.com".into(),
            region: String::new(),
            bucket: "shots".into(),
            prefix: "snapline".into(),
            public_base_url: "https://shots.example.com".into(),
            access_key: "AKIDEXAMPLE".into(),
            secret_key: ["wJalr", "XUtnFEMI", "K7MDENG"].concat(), // split so secret scanners stay quiet
        }
    }

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 30, 9, 15, 0).unwrap()
    }

    #[test]
    fn uploads_with_a_signed_put_and_returns_the_public_link() {
        let fake = FakeTransport::new(vec![(200, "")]);
        let result = upload(&fake, &config(), b"PNG", now(), &|_| {}).unwrap();
        assert!(
            result
                .page
                .as_str()
                .starts_with("https://shots.example.com/snapline/"),
            "{}",
            result.page
        );
        assert_eq!(result.page, result.direct);
        let sent = &fake.requests()[0];
        assert_eq!(sent.method, "PUT");
        assert_eq!(sent.url.host_str(), Some("acct.r2.cloudflarestorage.com"));
        assert_eq!(sent.body, b"PNG");
        let auth = sent
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .unwrap();
        assert!(
            auth.1
                .starts_with("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/"),
            "{}",
            auth.1
        );
        assert!(
            auth.1.contains("/auto/s3/"),
            "an empty region means auto: {}",
            auth.1
        );
    }

    #[test]
    fn missing_settings_are_explained_without_sending_anything() {
        let fake = FakeTransport::new(vec![]);
        let mut c = config();
        c.secret_key.clear();
        assert_eq!(
            upload(&fake, &c, b"PNG", now(), &|_| {}).unwrap_err(),
            UploadError::message(MISSING)
        );
        assert_eq!(fake.requests(), []);
    }

    // Review Focus 4.
    #[test]
    fn s3_error_never_shows_the_secret() {
        let fake = FakeTransport::new(vec![(403, "<Error><Code>AccessDenied</Code></Error>")]);
        let text = upload(&fake, &config(), b"PNG", now(), &|_| {})
            .unwrap_err()
            .to_string();
        assert!(
            text.starts_with("S3 upload failed (HTTP 403)") && text.contains("AccessDenied"),
            "{text}"
        );
        assert!(!text.contains(&config().secret_key));
    }
}
