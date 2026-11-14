//! Twin of `macOS/Sources/SnaplineCore/GoogleOAuth.swift`: the pure parts of Google sign-in (URLs, callback
//! parsing, token decoding). The network and the loopback listener live in the app.

/// PKCE (RFC 7636) for the OAuth "installed app" flow.
pub mod pkce {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};

    /// A new random code verifier (43 URL-safe characters from 32 secure random bytes).
    #[must_use]
    pub fn make_verifier() -> String {
        // rand's default generator is a CSPRNG seeded from the OS, like SecRandomCopyBytes on macOS.
        let bytes: [u8; 32] = rand::random();
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// The S256 challenge for `verifier`: base64url(SHA-256(verifier)).
    #[must_use]
    pub fn challenge(verifier: &str) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
    }
}

use crate::percent_encoding::{query_items, unreserved};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

pub const SCOPE: &str = "openid email https://www.googleapis.com/auth/drive.file";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";

/// Why a sign-in callback was not accepted, worded for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackError(pub String);

impl std::fmt::Display for CallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CallbackError {}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
}

/// Google's consent page URL for the installed-app flow, asking for a refresh token (offline access).
///
/// # Panics
/// Never: every value is percent-encoded into a fixed URL.
#[must_use]
pub fn authorization_url(
    client_id: &str,
    redirect_uri: &str,
    challenge: &str,
    state: &str,
) -> url::Url {
    let query = [
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("access_type", "offline"),
        // Always return a refresh token, also when the user signed in before.
        ("prompt", "consent"),
    ]
    .iter()
    .map(|(name, value)| format!("{name}={}", unreserved(value)))
    .collect::<Vec<_>>()
    .join("&");
    url::Url::parse(&format!(
        "https://accounts.google.com/o/oauth2/v2/auth?{query}"
    ))
    .expect("valid URL")
}

/// Reads the first line of the HTTP request Google redirects the browser to.
///
/// # Errors
/// A [`CallbackError`] when the state does not match, Google reports an error, or there is no code.
pub fn code_from_request_line(line: &str, expected_state: &str) -> Result<String, CallbackError> {
    let error = |message: &str| Err(CallbackError(message.to_owned()));
    let Some(target) = line.split(' ').filter(|part| !part.is_empty()).nth(1) else {
        return error("Unexpected sign-in response.");
    };
    let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
        return error("Unexpected sign-in response.");
    };
    let items = query_items(url.query().unwrap_or_default());
    // The first value counts when a name repeats.
    let get = |name: &str| {
        items
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };

    // State first: a request that does not carry our state must not decide anything, not even an error.
    if get("state") != Some(expected_state) {
        return error("Sign-in state did not match.");
    }
    if let Some(google_error) = get("error") {
        return error(&format!("Google sign-in failed: {google_error}"));
    }
    match get("code") {
        Some(code) if !code.is_empty() => Ok(code.to_owned()),
        _ => error("No sign-in code received."),
    }
}

/// The branded page the browser shows after the redirect, served by Snapline on 127.0.0.1.
/// Self-contained (inline styles and icon) so it works offline and loads nothing that could see the code in its URL.
/// `reason` is escaped: it can contain text from the request.
#[must_use]
pub fn callback_page(signed_in: bool, reason: &str) -> String {
    let escaped = reason
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;");
    let title = if signed_in {
        "Signed in to Snapline"
    } else {
        "Snapline sign-in did not finish"
    };
    let body = if signed_in {
        "<h1>You’re signed in</h1><p>Snapline can now upload to your Google Drive. You can close this tab and go back to Snapline.</p>".to_owned()
    } else {
        format!(
            "<h1>Sign-in did not finish</h1><p class=reason>{escaped}</p>\
             <p>Close this tab, then click <b>Sign in with Google…</b> in Snapline’s Preferences to try again.</p>"
        )
    };
    format!(
        "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\">\n\
         <title>{title}</title><style>{CALLBACK_STYLE}</style>\n\
         <main>{CALLBACK_ICON}{body}</main>"
    )
}

// Brand colours from BRAND.md: ink, paper, violet deep → cyan.
const CALLBACK_STYLE: &str = "\
:root{color-scheme:light dark;--paper:#f6f5fb;--ink:#0c0819;--muted:#565170;--card:#fff}
@media(prefers-color-scheme:dark){:root{--paper:#0c0819;--ink:#f3f1fa;--muted:#a9a3c2;--card:#17112b}}
body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--paper);color:var(--ink);\
font:17px/1.55 -apple-system,BlinkMacSystemFont,system-ui,sans-serif}
main{width:min(100% - 3rem,30rem);padding:2.5rem;border-radius:20px;background:var(--card);\
box-shadow:0 1px 2px rgba(12,8,25,.06),0 24px 48px -24px rgba(44,20,120,.35)}
svg{width:64px;height:64px;display:block;margin-bottom:1.5rem}
h1{font-size:1.75rem;line-height:1.15;letter-spacing:-.02em;margin:0 0 .6rem}
p{margin:0 0 .75rem;color:var(--muted)}p:last-child{margin:0}
.reason{color:var(--ink);padding:.6rem .8rem;border-left:3px solid #e5484d;background:rgba(229,72,77,.08);border-radius:0 8px 8px 0}";

/// The app icon, drawn inline: a violet → cyan squircle with white selection corners and a spark.
const CALLBACK_ICON: &str = "\
<svg viewBox=\"0 0 64 64\" aria-hidden=\"true\"><defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
<stop offset=\"0\" stop-color=\"#4c1ab8\"/><stop offset=\".5\" stop-color=\"#7c3aed\"/><stop offset=\"1\" stop-color=\"#22d3ee\"/>\
</linearGradient></defs><rect width=\"64\" height=\"64\" rx=\"15\" fill=\"url(#g)\"/>\
<g fill=\"none\" stroke=\"#fff\" stroke-width=\"4.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\
<path d=\"M14 24v-10h10M40 14h10v10M50 40v10h-10M24 50h-10v-10\"/></g>\
<path fill=\"#fff\" d=\"M32 22c.9 6.4 3.6 9.1 10 10c-6.4.9-9.1 3.6-10 10c-.9-6.4-3.6-9.1-10-10c6.4-.9 9.1-3.6 10-10z\"/>\
<path fill=\"#fff\" d=\"M42 18c.3 2 1 2.7 3 3c-2 .3-2.7 1-3 3c-.3-2-1-2.7-3-3c2-.3 2.7-1 3-3z\"/></svg>";

/// True when the token endpoint's error body says the refresh token is expired or revoked
/// (`{"error":"invalid_grant"}`), so the user has to sign in again.
#[must_use]
pub fn is_invalid_grant(body: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        error: String,
    }
    serde_json::from_slice::<ErrorBody>(body).is_ok_and(|b| b.error == "invalid_grant")
}

/// Form body for the token endpoint: `key=value` pairs sorted by key, values percent-encoded.
#[must_use]
pub fn form_body(fields: &[(&str, &str)]) -> String {
    let mut fields = fields.to_vec();
    fields.sort_by_key(|(key, _)| *key);
    fields
        .iter()
        .map(|(key, value)| format!("{key}={}", unreserved(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Email claim from an ID token. The token came straight from Google over TLS,
/// so the signature is not checked; it is only used to show who is signed in.
#[must_use]
pub fn email_from_id_token(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').filter(|part| !part.is_empty()).collect();
    let [_, payload, _] = parts[..] else {
        return None;
    };
    let json = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&json).ok()?;
    claims["email"].as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 7636 Appendix B.
    #[test]
    fn challenge_matches_rfc_example() {
        assert_eq!(
            pkce::challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_is_url_safe_and_long_enough() {
        let verifier = pkce::make_verifier();
        assert!((43..=128).contains(&verifier.len()));
        assert!(!verifier.contains(['+', '/', '=']));
        assert_ne!(pkce::make_verifier(), verifier);
    }

    #[test]
    fn authorization_url_has_all_required_parameters() {
        let url = authorization_url("client-123", "http://127.0.0.1:5555", "abc", "xyz");
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        let mut query = query_items(url.query().unwrap());
        query.sort();
        let mut expected = vec![
            ("client_id", "client-123"),
            ("redirect_uri", "http://127.0.0.1:5555"),
            ("response_type", "code"),
            (
                "scope",
                "openid email https://www.googleapis.com/auth/drive.file",
            ),
            ("code_challenge", "abc"),
            ("code_challenge_method", "S256"),
            ("state", "xyz"),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ];
        expected.sort_unstable();
        let expected: Vec<(String, String)> = expected
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        assert_eq!(query, expected);
    }

    #[test]
    fn callback_returns_the_code() {
        for (line, code) in [
            ("GET /?state=xyz&code=4/0Abc&scope=email HTTP/1.1", "4/0Abc"),
            ("GET /?code=4%2F0Abc&state=xyz HTTP/1.1", "4/0Abc"), // percent-encoded code is decoded
        ] {
            assert_eq!(
                code_from_request_line(line, "xyz"),
                Ok(code.to_owned()),
                "{line}"
            );
        }
    }

    #[test]
    fn callback_rejects() {
        for (line, message) in [
            // State is checked first: a request without our state cannot end sign-in, not even with an error.
            (
                "GET /?error=%3Cscript%3E HTTP/1.1",
                "Sign-in state did not match.",
            ),
            (
                "GET /?state=evil&code=1 HTTP/1.1",
                "Sign-in state did not match.",
            ),
            ("GET /?code=1 HTTP/1.1", "Sign-in state did not match."),
            (
                "GET /?state=evil&state=xyz&code=1 HTTP/1.1",
                "Sign-in state did not match.",
            ), // first value counts
            (
                "GET /?error=access_denied&state=xyz HTTP/1.1",
                "Google sign-in failed: access_denied",
            ),
            (
                "GET /?state=xyz&code= HTTP/1.1",
                "No sign-in code received.",
            ),
            ("GET /?state=xyz HTTP/1.1", "No sign-in code received."),
            ("GET", "Unexpected sign-in response."),
        ] {
            assert_eq!(
                code_from_request_line(line, "xyz"),
                Err(CallbackError(message.to_owned())),
                "{line}"
            );
        }
    }

    #[test]
    fn callback_page_escapes_html() {
        let page = callback_page(false, "failed: <script>alert(1)</script> & \"x\"");
        assert!(!page.contains("<script>"));
        assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;x&quot;"));
    }

    #[test]
    fn callback_page_says_what_happened_and_what_to_do_next() {
        let ok = callback_page(true, "");
        assert!(ok.contains("<title>Signed in to Snapline</title>"));
        assert!(ok.contains("You can close this tab"));

        let failed = callback_page(false, "Google sign-in failed: access_denied");
        assert!(failed.contains("<title>Snapline sign-in did not finish</title>"));
        assert!(failed.contains("Google sign-in failed: access_denied"));
        assert!(failed.contains("Sign in with Google…")); // how to try again
    }

    #[test]
    fn callback_page_loads_nothing_from_the_network() {
        // Served from 127.0.0.1 with the sign-in code in its URL: no fonts, images or links that could leak it or fail offline.
        for signed_in in [true, false] {
            let page = callback_page(signed_in, "x");
            assert!(!page.contains("http")); // also rules out url(https://…) and links
            assert!(!page.contains("src="));
            assert!(!page.contains("//"));
        }
    }

    #[test]
    fn form_body_encodes_everything_but_unreserved_ascii() {
        assert_eq!(
            form_body(&[("b", "x y+z/é"), ("a", "1")]),
            "a=1&b=x%20y%2Bz%2F%C3%A9"
        ); // sorted keys; "é" as UTF-8
    }

    #[test]
    fn token_response_decodes() {
        let json = r#"{"access_token":"ya29.a","expires_in":3599,"refresh_token":"1//r","scope":"email","token_type":"Bearer","id_token":"x"}"#;
        let token: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(token.access_token, "ya29.a");
        assert_eq!(token.refresh_token.as_deref(), Some("1//r"));
        assert_eq!(token.expires_in, 3599);
    }

    #[test]
    fn recognises_an_expired_sign_in() {
        for (body, expired) in [
            (
                r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
                true,
            ),
            (r#"{"error":"invalid_client"}"#, false),
            ("<html>Bad Gateway</html>", false),
        ] {
            assert_eq!(is_invalid_grant(body.as_bytes()), expired, "{body}");
        }
    }

    #[test]
    fn email_from_an_id_token() {
        // header.payload.signature where payload = {"email":"me@gmail.com","email_verified":true}
        let payload = URL_SAFE_NO_PAD.encode(r#"{"email":"me@gmail.com","email_verified":true}"#);
        assert_eq!(
            email_from_id_token(&format!("eyJhbGciOiJSUzI1NiJ9.{payload}.sig")).as_deref(),
            Some("me@gmail.com")
        );
        assert_eq!(email_from_id_token("not-a-jwt"), None);
    }
}
