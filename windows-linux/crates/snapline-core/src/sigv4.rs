//! Twin of `macOS/Sources/SnaplineCore/SigV4Signer.swift`: AWS Signature Version 4 for one request,
//! enough for an S3-compatible `PUT`.

use crate::percent_encoding::{query_items, unreserved};
use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use std::fmt::Write;

/// An HTTP request without its body: what gets signed and then sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: url::Url,
    pub headers: Vec<(String, String)>,
}

impl HttpRequest {
    #[must_use]
    pub fn new(method: &str, url: url::Url) -> Self {
        Self {
            method: method.to_owned(),
            url,
            headers: Vec::new(),
        }
    }

    /// Sets a header, replacing any with the same name in any letter case (as `URLRequest.setValue` does).
    pub fn set_header(&mut self, name: &str, value: &str) {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        self.headers.push((name.to_owned(), value.to_owned()));
    }

    /// The value of a header, matched in any letter case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Signs every header already set on the request plus `host` and `x-amz-date`.
pub struct SigV4Signer {
    pub access_key: String,
    secret_key: String,
    pub region: String,
    pub service: String,
}

impl SigV4Signer {
    /// A signer for S3 (service `s3`).
    #[must_use]
    pub fn new(access_key: &str, secret_key: &str, region: &str) -> Self {
        Self::with_service(access_key, secret_key, region, "s3")
    }

    #[must_use]
    pub fn with_service(access_key: &str, secret_key: &str, region: &str, service: &str) -> Self {
        Self {
            access_key: access_key.to_owned(),
            secret_key: secret_key.to_owned(),
            region: region.to_owned(),
            service: service.to_owned(),
        }
    }

    /// Adds `x-amz-date` and `Authorization`, signing every header already set plus `host`.
    /// `payload_hash` is the hex SHA-256 of the body (see [`Self::sha256_hex`]).
    /// Leaves the request unsigned if its URL has no host.
    pub fn sign(&self, request: &mut HttpRequest, payload_hash: &str, date: DateTime<Utc>) {
        let amz_date = date.format("%Y%m%dT%H%M%SZ").to_string();
        let day = &amz_date[..8];
        request.set_header("x-amz-date", &amz_date);

        let Some(host) = request.url.host_str() else {
            return;
        };
        let host = request
            .url
            .port()
            .map_or_else(|| host.to_owned(), |port| format!("{host}:{port}"));
        let mut headers = std::collections::BTreeMap::from([("host".to_owned(), host)]);
        for (name, value) in &request.headers {
            headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
        }
        let signed_headers = headers
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(";");

        let path = percent_encoding::percent_decode_str(request.url.path()).decode_utf8_lossy();
        let canonical_request = [
            request.method.clone(),
            Self::canonical_uri(&path),
            canonical_query(&request.url),
            headers.iter().fold(String::new(), |mut out, (n, v)| {
                let _ = writeln!(out, "{n}:{v}");
                out
            }),
            signed_headers.clone(),
            payload_hash.to_owned(),
        ]
        .join("\n");

        let scope = format!("{day}/{}/{}/aws4_request", self.region, self.service);
        let string_to_sign = [
            "AWS4-HMAC-SHA256",
            &amz_date,
            &scope,
            &Self::sha256_hex(canonical_request.as_bytes()),
        ]
        .join("\n");

        let mut key = format!("AWS4{}", self.secret_key).into_bytes();
        for part in [day, &self.region, &self.service, "aws4_request"] {
            key = hmac(&key, part.as_bytes());
        }
        let signature = hex(&hmac(&key, string_to_sign.as_bytes()));

        request.set_header(
            "Authorization",
            &format!(
                "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
                self.access_key
            ),
        );
    }

    #[must_use]
    pub fn sha256_hex(data: &[u8]) -> String {
        hex(&Sha256::digest(data))
    }

    /// S3 style: each path segment is percent-encoded once with the RFC 3986 unreserved set.
    #[must_use]
    pub fn canonical_uri(path: &str) -> String {
        if path.is_empty() {
            return "/".to_owned();
        }
        path.split('/')
            .map(unreserved)
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// Query items encoded again with the unreserved set, sorted by name then value.
fn canonical_query(url: &url::Url) -> String {
    let mut items: Vec<(String, String)> = query_items(url.query().unwrap_or_default())
        .iter()
        .map(|(name, value)| (unreserved(name), unreserved(value)))
        .collect();
    items.sort();
    items
        .iter()
        .map(|(n, v)| format!("{n}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes a key of any length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Name, method, URL, extra headers, expected `Authorization`.
    type Vector = (
        &'static str,
        &'static str,
        &'static str,
        &'static [(&'static str, &'static str)],
        &'static str,
    );

    /// Cases from AWS's official Signature Version 4 test suite (awslabs/aws-c-auth, tests/aws-signing-test-suite/v4).
    const VECTORS: [Vector; 3] = [
        (
            "get-vanilla",
            "GET",
            "https://example.amazonaws.com/",
            &[],
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31",
        ),
        // Query parameters must be sorted by name in the canonical request.
        (
            "get-vanilla-query-order-key-case",
            "GET",
            "https://example.amazonaws.com/?Param2=value2&Param1=value1",
            &[],
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=b97d918cfa904a5beff61c982a1b6f458b799221646efd99d3219ec94cdf2500",
        ),
        // Extra headers are lowercased, sorted and signed (as the S3 upload does with content-type).
        (
            "post-header-key-sort",
            "POST",
            "https://example.amazonaws.com/",
            &[("My-Header1", "value1")],
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;my-header1;x-amz-date, \
             Signature=c5410059b04c1ee005303aed430f6e6645f61f4dc9e1461ec8f8916fdf18852c",
        ),
    ];

    #[test]
    fn matches_aws_test_suite() {
        let signer = SigV4Signer::with_service(
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "service",
        );
        let date = "2015-08-30T12:36:00Z".parse().unwrap();
        for (name, method, url, headers, authorization) in VECTORS {
            let mut request = HttpRequest::new(method, url.parse().unwrap());
            for (header, value) in headers {
                request.set_header(header, value);
            }
            signer.sign(&mut request, &SigV4Signer::sha256_hex(&[]), date);
            assert_eq!(
                request.header("x-amz-date"),
                Some("20150830T123600Z"),
                "{name}"
            );
            assert_eq!(
                request.header("Authorization"),
                Some(authorization),
                "{name}"
            );
        }
    }

    #[test]
    fn canonical_uri_encodes_each_path_segment_once() {
        assert_eq!(
            SigV4Signer::canonical_uri("/bucket/shots/2026/10/a b+c.png"),
            "/bucket/shots/2026/10/a%20b%2Bc.png"
        );
        assert_eq!(SigV4Signer::canonical_uri(""), "/");
    }

    #[test]
    fn canonical_uri_encodes_non_ascii_once() {
        assert_eq!(
            SigV4Signer::canonical_uri("/shots/a b+é.png"),
            "/shots/a%20b%2B%C3%A9.png"
        );
    }

    #[test]
    fn sha256_of_empty_payload() {
        assert_eq!(
            SigV4Signer::sha256_hex(&[]),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn headers_are_matched_in_any_case() {
        let mut request = HttpRequest::new("PUT", "https://example.com/".parse().unwrap());
        request.set_header("Content-Type", "image/png");
        request.set_header("content-type", "image/jpeg");
        assert_eq!(request.headers.len(), 1);
        assert_eq!(request.header("CONTENT-TYPE"), Some("image/jpeg"));
    }
}
