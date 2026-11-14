//! Google sign-in with the OAuth "installed app" flow (browser + PKCE + one request on 127.0.0.1), and
//! access-token refresh. Twin of `GoogleAuth` on macOS; the caller stores the refresh token.

use crate::error::UploadError;
use crate::http::{Request, Transport, send_ok};
use snapline_core::google_oauth::{
    REVOKE_URL, TOKEN_URL, TokenResponse, authorization_url, callback_page, code_from_request_line,
    email_from_id_token, form_body, is_invalid_grant, pkce,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

pub const NOT_SIGNED_IN: &str = "Sign in to Google in Snapline's upload settings.";
pub const EXPIRED: &str = "Google sign-in expired. Sign in again in Snapline's upload settings.";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoogleClient {
    pub id: String,
    pub secret: String,
}

impl GoogleClient {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.id.trim().is_empty() && !self.secret.trim().is_empty()
    }
}

#[derive(Clone)]
pub struct AccessToken {
    pub token: String,
    pub expires: Instant,
}

impl AccessToken {
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        Instant::now() < self.expires
    }

    fn from(response: &TokenResponse) -> Self {
        let seconds = u64::try_from(response.expires_in - 60).unwrap_or(0);
        Self {
            token: response.access_token.clone(),
            expires: Instant::now() + Duration::from_secs(seconds),
        }
    }
}

// Tokens are secrets: `Debug` never prints them, so a panic or a log line cannot leak one.
impl std::fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessToken")
            .field("fresh", &self.is_fresh())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum RefreshError {
    /// Google no longer accepts the refresh token (revoked, or a "Testing" app after 7 days).
    Expired,
    Failed(UploadError),
}

pub struct SignedIn {
    pub refresh_token: String,
    pub email: Option<String>,
    pub access: AccessToken,
}

impl std::fmt::Debug for SignedIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedIn")
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}

fn token_request(t: &dyn Transport, fields: &[(&str, &str)]) -> Result<TokenResponse, UploadError> {
    let request = Request::new("POST", url::Url::parse(TOKEN_URL).expect("fixed URL"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(fields).into_bytes());
    let body = send_ok(t, &request, "Google token request", &|_| {})?;
    serde_json::from_slice(&body)
        .map_err(|e| UploadError::message(format!("Google sent an unexpected answer: {e}")))
}

/// A new access token from the saved refresh token.
///
/// # Errors
/// `Expired` when Google no longer accepts the refresh token; `Failed` for anything else.
pub fn refresh(
    t: &dyn Transport,
    client: &GoogleClient,
    refresh_token: &str,
) -> Result<AccessToken, RefreshError> {
    let fields = [
        ("client_id", client.id.trim()),
        ("client_secret", client.secret.trim()),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    match token_request(t, &fields) {
        Ok(response) => Ok(AccessToken::from(&response)),
        Err(UploadError::Http {
            status: 400, body, ..
        }) if is_invalid_grant(&body) => Err(RefreshError::Expired),
        Err(e) => Err(RefreshError::Failed(e)),
    }
}

/// Signs in through the browser. `open_browser` opens the URL and says whether it could.
///
/// # Errors
/// When the browser cannot be opened, nothing comes back in time, or Google refuses.
pub fn sign_in(
    t: &dyn Transport,
    client: &GoogleClient,
    open_browser: &dyn Fn(&url::Url) -> bool,
    timeout: Duration,
) -> Result<SignedIn, UploadError> {
    let io = |e: std::io::Error| {
        UploadError::message(format!("Google sign-in could not listen on this PC: {e}"))
    };
    let listener = TcpListener::bind("127.0.0.1:0").map_err(io)?;
    let redirect = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().map_err(io)?.port()
    );
    let (state, verifier) = (pkce::make_verifier(), pkce::make_verifier());
    let url = authorization_url(
        client.id.trim(),
        &redirect,
        &pkce::challenge(&verifier),
        &state,
    );
    if !open_browser(&url) {
        return Err(UploadError::message(
            "Snapline could not open the browser for Google sign-in.",
        ));
    }
    let (mut stream, line) = wait_for_redirect(&listener, &state, timeout)?;
    let result = exchange(t, client, &line, &state, &redirect, &verifier);
    let reason = result
        .as_ref()
        .err()
        .map(ToString::to_string)
        .unwrap_or_default();
    reply(&mut stream, result.is_ok(), &reason);
    result
}

fn exchange(
    t: &dyn Transport,
    client: &GoogleClient,
    line: &str,
    state: &str,
    redirect: &str,
    verifier: &str,
) -> Result<SignedIn, UploadError> {
    let code = code_from_request_line(line, state).map_err(|e| UploadError::message(e.0))?;
    let token = token_request(
        t,
        &[
            ("code", &code),
            ("client_id", client.id.trim()),
            ("client_secret", client.secret.trim()),
            ("redirect_uri", redirect),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier),
        ],
    )?;
    let refresh_token = token
        .refresh_token
        .clone()
        .ok_or_else(|| UploadError::message("Google did not return a refresh token."))?;
    let email = token.id_token.as_deref().and_then(email_from_id_token);
    Ok(SignedIn {
        refresh_token,
        email,
        access: AccessToken::from(&token),
    })
}

/// Waits for the request that carries `state`; anything else gets a 404.
fn wait_for_redirect(
    listener: &TcpListener,
    state: &str,
    timeout: Duration,
) -> Result<(TcpStream, String), UploadError> {
    let deadline = Instant::now() + timeout;
    listener
        .set_nonblocking(true)
        .map_err(|e| UploadError::message(e.to_string()))?;
    let wanted = format!("state={state}");
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut buffer = vec![0; 16_384];
                let n = stream.read(&mut buffer).unwrap_or(0);
                let text = String::from_utf8_lossy(&buffer[..n]);
                let line = text.split("\r\n").next().unwrap_or_default().to_owned();
                if line.contains(&wanted) {
                    return Ok((stream, line));
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(UploadError::message(e.to_string())),
        }
    }
    Err(UploadError::message("Google sign-in timed out. Try again."))
}

/// The branded page. Its URL holds the sign-in code: no caching, no referrer, only inline style.
fn reply(stream: &mut TcpStream, signed_in: bool, reason: &str) {
    let html = callback_page(signed_in, reason);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Revokes the refresh token at Google. Failures are ignored: the caller forgets the token either way.
///
/// # Panics
/// Never: the revoke URL is fixed.
pub fn sign_out(t: &dyn Transport, refresh_token: &str) {
    let request = Request::new("POST", url::Url::parse(REVOKE_URL).expect("fixed URL"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(&[("token", refresh_token)]).into_bytes());
    let _ = t.send(&request, &|_| {});
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::fake::FakeTransport;
    use snapline_core::percent_encoding::query_items;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};

    // Payload {"email":"a@example.com"}, base64url.
    const ID_TOKEN: &str = "h.eyJlbWFpbCI6ImFAZXhhbXBsZS5jb20ifQ.s";

    fn client() -> GoogleClient {
        GoogleClient {
            id: "id.apps.googleusercontent.com".into(),
            secret: "client-secret".into(),
        }
    }

    fn get(port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    }

    /// Plays the browser: first a stray favicon request, then Google's redirect with `code`.
    fn browser(page: Arc<Mutex<String>>) -> impl Fn(&url::Url) -> bool {
        move |auth: &url::Url| {
            let query: std::collections::HashMap<_, _> =
                query_items(auth.query().unwrap()).into_iter().collect();
            let redirect = url::Url::parse(&query["redirect_uri"]).unwrap();
            let (port, state) = (redirect.port().unwrap(), query["state"].clone());
            let page = page.clone();
            std::thread::spawn(move || {
                assert!(get(port, "/favicon.ico").starts_with("HTTP/1.1 404"));
                *page.lock().unwrap() = get(port, &format!("/?state={state}&code=CODE"));
            });
            true
        }
    }

    #[test]
    fn refresh_returns_a_fresh_token() {
        let fake = FakeTransport::new(vec![(200, r#"{"access_token":"A","expires_in":3600}"#)]);
        let token = refresh(&fake, &client(), "R").unwrap();
        assert_eq!(token.token, "A");
        assert!(token.is_fresh());
        let body = String::from_utf8(fake.requests()[0].body.clone()).unwrap();
        assert!(
            body.contains("grant_type=refresh_token") && body.contains("refresh_token=R"),
            "{body}"
        );
    }

    // Review Focus 2.
    #[test]
    fn refresh_reports_a_revoked_sign_in() {
        let fake = FakeTransport::new(vec![(400, r#"{"error":"invalid_grant"}"#)]);
        assert!(matches!(
            refresh(&fake, &client(), "R"),
            Err(RefreshError::Expired)
        ));
        let fake = FakeTransport::new(vec![(500, "oops")]);
        assert!(matches!(
            refresh(&fake, &client(), "R"),
            Err(RefreshError::Failed(_))
        ));
    }

    #[test]
    fn sign_in_exchanges_the_code_and_tells_the_browser() {
        let token = format!(
            r#"{{"access_token":"A","expires_in":3600,"refresh_token":"R","id_token":"{ID_TOKEN}"}}"#
        );
        let fake = FakeTransport::new(vec![(200, token.as_str())]);
        let page = Arc::new(Mutex::new(String::new()));
        let signed_in = sign_in(
            &fake,
            &client(),
            &browser(page.clone()),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(signed_in.refresh_token, "R");
        assert_eq!(signed_in.email.as_deref(), Some("a@example.com"));
        let body = String::from_utf8(fake.requests()[0].body.clone()).unwrap();
        assert!(
            body.contains("code=CODE") && body.contains("code_verifier="),
            "{body}"
        );
        std::thread::sleep(Duration::from_millis(200));
        let page = page.lock().unwrap().clone();
        assert!(
            page.starts_with("HTTP/1.1 200 OK") && page.contains("Cache-Control: no-store"),
            "{page}"
        );
    }

    #[test]
    fn sign_in_without_a_refresh_token_fails_and_says_so_in_the_browser() {
        let fake = FakeTransport::new(vec![(200, r#"{"access_token":"A","expires_in":3600}"#)]);
        let page = Arc::new(Mutex::new(String::new()));
        let error = sign_in(&fake, &client(), &browser(page), Duration::from_secs(10)).unwrap_err();
        assert_eq!(error.to_string(), "Google did not return a refresh token.");
    }

    #[test]
    fn sign_in_gives_up_after_the_timeout() {
        let fake = FakeTransport::new(vec![]);
        let error = sign_in(
            &fake,
            &client(),
            &|_: &url::Url| true,
            Duration::from_millis(300),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "Google sign-in timed out. Try again.");
    }

    #[test]
    fn a_browser_that_does_not_open_is_reported() {
        let error = sign_in(
            &FakeTransport::new(vec![]),
            &client(),
            &|_: &url::Url| false,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("could not open the browser"),
            "{error}"
        );
    }
}
