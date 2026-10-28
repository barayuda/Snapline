//! Twin of `macOS/Sources/SnaplineCore/PercentEncoding.swift`: RFC 3986 percent-encoding, and where an
//! S3-compatible upload goes.

use crate::sigv4::{HttpRequest, SigV4Signer};
use chrono::{DateTime, Utc};

/// Only the unreserved ASCII characters stay as they are (`A–Z a–z 0–9 - . _ ~`); everything else,
/// including non-ASCII letters, is encoded as UTF-8 bytes.
#[must_use]
pub fn unreserved(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, NOT_UNRESERVED).to_string()
}

const NOT_UNRESERVED: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Plain http is accepted only for these: the keys never leave the computer (e.g. a local MinIO or floci).
const LOOPBACK_HOSTS: [&str; 4] = ["localhost", "127.0.0.1", "::1", "[::1]"];

/// The upload URL and the public link it will have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S3Urls {
    pub upload: url::Url,
    pub public: url::Url,
}

/// Path-style upload URL (`<endpoint>/<bucket>/<key>`, which works for AWS, Cloudflare R2 and MinIO) and the
/// public link (`<public_base_url>/<key>`). Each key segment is percent-encoded once. `None` unless the
/// endpoint is HTTPS (the keys are sent with it; plain http only on this computer) and bucket and public URL are set.
#[must_use]
pub fn s3_urls(endpoint: &str, bucket: &str, key: &str, public_base_url: &str) -> Option<S3Urls> {
    let endpoint = endpoint.trim_matches('/');
    let base = public_base_url.trim_matches('/');
    if bucket.is_empty() || base.is_empty() {
        return None;
    }
    let check = url::Url::parse(endpoint).ok()?;
    let host = check.host_str()?;
    if !(check.scheme() == "https" || (check.scheme() == "http" && LOOPBACK_HOSTS.contains(&host)))
    {
        return None;
    }
    let encoded_key = SigV4Signer::canonical_uri(key);
    let upload = url::Url::parse(&format!(
        "{endpoint}/{}/{encoded_key}",
        SigV4Signer::canonical_uri(bucket)
    ))
    .ok()?;
    let public = url::Url::parse(&format!("{base}/{encoded_key}")).ok()?;
    Some(S3Urls { upload, public })
}

/// The signed `PUT` that uploads `data` to `key`, plus the public link it will have afterwards.
/// The body is not attached: send `body` with the request. `None` for incomplete settings (see [`s3_urls`]).
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors S3Location.uploadRequest
pub fn s3_upload_request(
    body: &[u8],
    key: &str,
    content_type: &str,
    endpoint: &str,
    bucket: &str,
    public_base_url: &str,
    signer: &SigV4Signer,
    date: DateTime<Utc>,
) -> Option<(HttpRequest, url::Url)> {
    let urls = s3_urls(endpoint, bucket, key, public_base_url)?;
    let mut request = HttpRequest::new("PUT", urls.upload);
    let hash = SigV4Signer::sha256_hex(body);
    request.set_header("Content-Type", content_type);
    request.set_header("x-amz-content-sha256", &hash);
    signer.sign(&mut request, &hash, date);
    Some((request, urls.public))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_keeps_only_unreserved() {
        for (input, expected) in [
            ("abc-._~XYZ09", "abc-._~XYZ09"), // unreserved: unchanged
            ("a b", "a%20b"),
            ("a+b/c", "a%2Bb%2Fc"),
            ("é", "%C3%A9"), // non-ASCII letters are encoded too
        ] {
            assert_eq!(unreserved(input), expected, "{input}");
        }
    }

    #[test]
    fn joins_endpoint_bucket_and_key_and_encodes_the_key() {
        let urls = s3_urls(
            "https://acc.r2.cloudflarestorage.com/",
            "shots",
            "snapline/2026/10/ab cd.png",
            "https://shots.example.com/",
        )
        .unwrap();
        assert_eq!(
            urls.upload.as_str(),
            "https://acc.r2.cloudflarestorage.com/shots/snapline/2026/10/ab%20cd.png"
        );
        assert_eq!(
            urls.public.as_str(),
            "https://shots.example.com/snapline/2026/10/ab%20cd.png"
        );
    }

    #[test]
    fn keeps_an_endpoint_path_and_port() {
        let urls = s3_urls(
            "https://minio.example.com:9000/s3",
            "b",
            "k.png",
            "https://cdn.example.com/b",
        )
        .unwrap();
        assert_eq!(
            urls.upload.as_str(),
            "https://minio.example.com:9000/s3/b/k.png"
        );
        assert_eq!(urls.public.as_str(), "https://cdn.example.com/b/k.png");
    }

    #[test]
    fn allows_plain_http_only_on_this_computer() {
        // Loopback never leaves the computer, so a local MinIO or floci may use http.
        for endpoint in [
            "http://localhost:4566",
            "http://127.0.0.1:9000",
            "http://[::1]:9000",
        ] {
            let urls = s3_urls(endpoint, "b", "k.png", &format!("{endpoint}/b")).unwrap();
            assert_eq!(urls.upload.as_str(), format!("{endpoint}/b/k.png"));
        }
    }

    #[test]
    fn rejects_incomplete_settings() {
        for (endpoint, bucket, public_base) in [
            (
                "http://insecure.example.com",
                "b",
                "https://cdn.example.com",
            ), // HTTPS only: keys are sent with it
            ("http://192.168.1.5:9000", "b", "https://cdn.example.com"), // other machines need HTTPS, even on the LAN
            (
                "http://localhost.example.com",
                "b",
                "https://cdn.example.com",
            ), // not loopback
            ("https://ok.example.com", "", "https://cdn.example.com"),   // no bucket
            ("https://ok.example.com", "b", ""),                         // no public URL
            ("not a url", "b", "https://cdn.example.com"),
        ] {
            assert_eq!(
                s3_urls(endpoint, bucket, "k.png", public_base),
                None,
                "{endpoint} {bucket} {public_base}"
            );
        }
    }

    #[test]
    fn signed_put_carries_body_hash_type_and_signature() {
        let png = [0x89, 0x50, 0x4E, 0x47];
        let signer = SigV4Signer::new("AKIDEXAMPLE", "secret", "auto");
        let date = "2026-10-28T06:00:00Z".parse().unwrap();
        let (request, public) = s3_upload_request(
            &png,
            "snapline/2026/10/a b.png",
            "image/png",
            "https://acc.r2.cloudflarestorage.com",
            "shots",
            "https://shots.example.com",
            &signer,
            date,
        )
        .unwrap();

        assert_eq!(request.method, "PUT");
        assert_eq!(
            request.url.as_str(),
            "https://acc.r2.cloudflarestorage.com/shots/snapline/2026/10/a%20b.png"
        );
        assert_eq!(
            public.as_str(),
            "https://shots.example.com/snapline/2026/10/a%20b.png"
        );
        assert_eq!(request.header("Content-Type"), Some("image/png"));
        assert_eq!(
            request.header("x-amz-content-sha256"),
            Some(SigV4Signer::sha256_hex(&png).as_str())
        );
        assert_eq!(request.header("x-amz-date"), Some("20261028T060000Z"));
        let auth = request.header("Authorization").unwrap();
        assert!(auth.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20261028/auto/s3/aws4_request, "
        ));
        assert!(auth.contains(
            "SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature="
        ));
    }

    #[test]
    fn no_request_for_incomplete_settings() {
        let signer = SigV4Signer::new("a", "b", "auto");
        let request = s3_upload_request(
            &[],
            "k.png",
            "image/png",
            "http://insecure.example.com",
            "b",
            "https://cdn.example.com",
            &signer,
            Utc::now(),
        );
        assert_eq!(request, None);
    }
}
