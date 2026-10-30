# Phase 3: Windows uploads — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On Windows, Ctrl+D (or the Upload button) uploads the screenshot to an S3-compatible bucket or Google Drive, copies the link, and shows progress and the result in a popup near the tray. Search similar images uploads and then opens Google Lens or TinEye. Copy and save show a short confirmation in the same popup. An upload settings window holds the bucket details, the Google client, the Google sign-in, and keys stored in Windows Credential Manager.

**Architecture:** A new crate, `snapline-upload`, does the network work with no UI and no storage: S3 PUT, Drive upload and sharing, Google sign-in and token refresh. All of it goes through a small `Transport` trait, so tests use a scripted fake, and CI runs them on Windows and Linux. `snapline-overlay` gains the Upload and Search buttons and a popup (model, layout, drawing) that is tested like the overlay. The app crate wires it together: secrets in Credential Manager, settings read fresh for every upload, a worker thread per upload, the tray icon showing progress, and an egui settings window that runs as a second process (`snapline.exe --upload-settings`).

**Tech Stack:** Rust 2024 (stable), ureq 3 (sync HTTP, rustls with ring), keyring-core 1 with windows-native-keyring-store (apple-native-keyring-store for local runs on macOS), eframe/egui 0.37 (glow), open 5, serde_json, plus everything from Phase 2.

**Spec:** [docs/windows-linux.md](../windows-linux.md) (Phase 3), with the macOS app as the behaviour reference: `macOS/Sources/Snapline/Uploaders.swift`, `UploadPopover.swift`, `StatusPopover.swift`, `GoogleAuth.swift`, and the Upload tab in `Preferences.swift`.

## Decisions made for this phase

Barayuda's choices on 30 October 2026:

| Question | Choice |
|---|---|
| Where people enter keys and the Google client before the Preferences window exists | An **upload settings window** now (the Upload tab of Preferences, brought forward), opened from the tray. Secrets go to Credential Manager. Phase 5 adds the other tabs around it. |
| Search similar images | **In Phase 3**: it is an upload followed by opening Google Lens or TinEye |
| "Screenshot copied" / "Screenshot saved" confirmations | **In Phase 3**, in the same popup as uploads |

Choices made in this plan (each is cheap to change later):
- **ureq instead of reqwest.** reqwest 0.13 brings tokio and aws-lc, which needs extra build tools on Windows. ureq is synchronous, uses rustls with ring, and an upload runs on its own thread anyway. Cost: it trusts the bundled web root certificates, not the Windows certificate store. Networks that inspect TLS with their own root will fail until a later phase adds platform certificates.
- **keyring-core + windows-native-keyring-store.** keyring 4 is now mostly a command-line tool; the library is keyring-core plus one store per platform. keyring-core's mock store does not keep values between entries, so tests use our own in-memory `SecretStore`.
- **egui for the settings window, as a separate process.** The design already chose egui for Preferences. A second process keeps eframe's event loop away from the tray app's. The two processes share `settings.toml` (read-modify-write on save) and Credential Manager.
- **`SearchEngine` lives in `snapline-core`.** Both the overlay (button labels) and the app (search links) need it. Nothing in it touches the OS.

Behaviour copied from macOS:
- The link is copied as soon as the upload finishes. The popup closes after 4 seconds unless the pointer is over it.
- A failure stays open, with **Upload settings…** and **Try again**.
- The tray icon shows a progress ring, then ✓ (4 s) or ! (6 s).
- Drive files go to a "Snapline" folder (created again if it was deleted or trashed) and are shared as "anyone with the link can view".
- An expired Google sign-in (`invalid_grant`) removes the saved token and asks to sign in again.

Left for later phases: **Email** in the upload popup (Phase 5, with email); the Preferences tabs other than Upload (5); Windows certificate-store trust (later, if needed).

## Global Constraints

- Workspace: `windows-linux/`, edition 2024, the toolchain in `rust-toolchain.toml` (stable).
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` (pedantic) and, for the app, `cargo clippy -p snapline --all-targets --target x86_64-pc-windows-msvc -- -D warnings` must pass. Crate roots may allow only `clippy::cast_possible_truncation`, `clippy::cast_sign_loss`, `clippy::cast_precision_loss`. Rust 1.99 wants empty checks written as `assert_eq!(v, [])`.
- Add crates with `cargo add` (latest stable). This plan's code matches ureq 3.5, keyring-core 1.0, windows-native-keyring-store 1.1, apple-native-keyring-store 1.0 (feature `keychain`), eframe 0.37, open 5. If a newer major comes in, fix the call sites, not the behaviour.
- `snapline-upload` and `snapline-overlay` must not depend on winit, tray-icon, eframe, keyring or any OS API; both must build and test on Linux. Only the app crate touches the OS.
- Secrets (S3 access key ID and secret, Google client secret, Google refresh token) live only in the OS credential store, under service `dev.barayuda.snapline` with the macOS account names (`s3AccessKey`, `s3SecretKey`, `googleClientSecret`, `googleRefreshToken`). They never go into `settings.toml`, error messages or logs.
- `settings.toml` is read fresh for every upload. Writers load, change and save the latest file, and never overwrite a file that does not parse.
- All popup and toolbar sizes are macOS points multiplied by the monitor's scale.
- Copy uses plain English and sentence case, and names the Windows UI the Windows way ("upload settings", "tray icon").
- Git: commit after each task, GPG-signed, plain sentence-case messages, no AI co-author or "generated with" lines. Check `git log --format='%h %G?'` shows `G` before pushing.

## Review Focus

1. **Real servers failing**: wrong keys, a missing bucket, an HTML error page, no network, a slow upload. The user must get a short readable message and a working **Try again**; nothing may hang the tray app. Tests: `http_error_message_is_short_and_safe`, `unreachable_server_gives_a_sentence` (Task 1).
2. **A Google sign-in that expired or was revoked** (apps left in "Testing" lose tokens after 7 days). The saved token must go, the message must say to sign in again, and signing in again must work. Tests: `refresh_reports_a_revoked_sign_in` (Task 3), `expired_sign_in_clears_the_token` (Task 8).
3. **Taskbar on any screen edge, or the tray icon hidden in the overflow area.** The popup must sit fully inside the work area, next to the tray when it can. Test: `popup_stays_in_the_work_area_for_every_taskbar_edge` (Task 7).
4. **Secrets leaking.** No secret in `settings.toml`, and no secret in any error message. Tests: `secrets_never_reach_the_settings_file` (Task 9), `s3_error_never_shows_the_secret` (Task 2).
5. **Settings changed while Snapline runs.** The next upload uses them without a restart, and a broken `settings.toml` is never overwritten by a save. Tests: `upload_target_follows_the_file`, `update_refuses_to_overwrite_a_broken_file` (Task 5).

---

## File structure

```
windows-linux/crates/
├── snapline-core/src/search.rs          new: SearchEngine and its search links (Task 1)
├── snapline-upload/                      new: network work, no UI, no storage; tested on Windows + Linux
│   └── src/
│       ├── lib.rs                        UploadResult, module list
│       ├── error.rs                      UploadError and its user-facing text
│       ├── http.rs                       Request, Response, Transport, UreqTransport, FakeTransport (feature "fake")
│       ├── s3.rs                         S3Config, upload
│       ├── google.rs                     GoogleClient, AccessToken, refresh, sign_in (loopback), sign_out
│       └── drive.rs                      upload to the Snapline folder, share by link
├── snapline-overlay/src/
│   ├── toolbar.rs, session.rs, icons.rs  Upload and Search buttons, search menu, Ctrl+D (Task 6)
│   └── popup.rs                          new: the status popup — model, layout near the tray, drawing (Task 7)
└── snapline/src/
    ├── secrets.rs                        new: SecretStore, Keyring (Credential Manager), MemoryStore for tests
    ├── settings.rs                       upload fields, save, update
    ├── upload_job.rs                     new: one upload from settings + secrets to an outcome (testable)
    ├── popup_window.rs                   new: the popup's winit window
    ├── icon.rs                           tray icon states: progress ring, success, failure
    ├── actions.rs                        reports what copy and save did, for the confirmation
    ├── upload_settings.rs                new: the egui upload settings window and its form model
    ├── platform.rs                       work area of the monitor under a point
    ├── app.rs, main.rs                   wiring
```

---

### Task 1: The upload crate — errors, HTTP transport, search links

**Files:**
- Create: `windows-linux/crates/snapline-core/src/search.rs`; modify `snapline-core/src/lib.rs` (`pub mod search;`)
- Create: `windows-linux/crates/snapline-upload/Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/http.rs`

**Interfaces:**
- Consumes: `snapline_core::percent_encoding::unreserved`, `snapline_core::sigv4::HttpRequest`.
- Produces:
  - `snapline_core::search::SearchEngine { GoogleLens, TinEye }` with `ALL`, `title() -> &'static str`, `url(&url::Url) -> url::Url`.
  - `snapline_upload::UploadResult { page: Url, direct: Url }`.
  - `snapline_upload::error::UploadError { Message(String), Http { action: String, status: u16, body: Vec<u8> } }` with `UploadError::message(impl Into<String>)` and `Display`.
  - `snapline_upload::http::{Request, Response, Transport, UreqTransport, send_ok}`: `Request { method, url, headers: Vec<(String, String)>, body: Vec<u8> }` (`Clone`, `Debug`) with `new(method, url)`, `header(k, v) -> Self`, `body(Vec<u8>) -> Self`, `json(&serde_json::Value) -> Self`, `from_signed(HttpRequest, Vec<u8>) -> Self`. `Response { status: u16, body: Vec<u8> }`. `trait Transport: Send + Sync { fn send(&self, &Request, progress: &dyn Fn(f64)) -> Result<Response, UploadError>; }`. `send_ok(&dyn Transport, &Request, action: &str, progress) -> Result<Vec<u8>, UploadError>`.
  - With feature `fake`: `snapline_upload::http::fake::FakeTransport` with `new(Vec<(u16, &str)>)` and `requests() -> Vec<Request>`.

- [ ] **Step 1: Search links, test first**

`snapline-core/src/search.rs`:

```rust
//! Reverse image search links. Twin of `SearchEngine` in macOS `CaptureSession.swift`.

use crate::percent_encoding::unreserved;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEngine {
    GoogleLens,
    TinEye,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_links_carry_the_encoded_image_link() {
        let image = url::Url::parse("https://shots.example.com/snapline/2026/a.png").unwrap();
        assert_eq!(
            SearchEngine::GoogleLens.url(&image).as_str(),
            "https://lens.google.com/uploadbyurl?url=https%3A%2F%2Fshots.example.com%2Fsnapline%2F2026%2Fa.png"
        );
        assert_eq!(
            SearchEngine::TinEye.url(&image).as_str(),
            "https://tineye.com/search?url=https%3A%2F%2Fshots.example.com%2Fsnapline%2F2026%2Fa.png"
        );
    }

    #[test]
    fn titles_match_macos() {
        assert_eq!(SearchEngine::GoogleLens.title(), "Search with Google Lens");
        assert_eq!(SearchEngine::TinEye.title(), "Search with TinEye");
    }
}
```

Add `pub mod search;` to `snapline-core/src/lib.rs`. Run `cd windows-linux && cargo test -p snapline-core search` → fails (no `url`/`title`). Then add:

```rust
impl SearchEngine {
    pub const ALL: [Self; 2] = [Self::GoogleLens, Self::TinEye];

    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::GoogleLens => "Search with Google Lens",
            Self::TinEye => "Search with TinEye",
        }
    }

    /// # Panics
    /// Never: the base is fixed and the image link is percent-encoded.
    #[must_use]
    pub fn url(self, image: &url::Url) -> url::Url {
        let encoded = unreserved(image.as_str());
        let link = match self {
            Self::GoogleLens => format!("https://lens.google.com/uploadbyurl?url={encoded}"),
            Self::TinEye => format!("https://tineye.com/search?url={encoded}"),
        };
        url::Url::parse(&link).expect("fixed base with an encoded value")
    }
}
```

Run again → passes.

- [ ] **Step 2: Create the upload crate with the error type, test first**

`windows-linux/crates/snapline-upload/Cargo.toml`:

```toml
[package]
name = "snapline-upload"
version = "0.1.0"
edition.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[features]
# A scripted Transport for tests in other crates.
fake = []

[dependencies]
snapline-core = { path = "../snapline-core" }
```

Run `cd windows-linux && cargo add -p snapline-upload url chrono serde_json ureq && cargo add -p snapline-upload serde --features derive`.

`src/lib.rs`:

```rust
//! Uploads for the Windows and Linux app: S3-compatible buckets and Google Drive, plus Google sign-in.
//! No UI and no secret storage: callers pass settings and secrets in, and keep what comes back.

pub mod error;
pub mod http;

/// Where an upload ended up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadResult {
    /// The link to copy and open.
    pub page: url::Url,
    /// A direct link to the image, for image search.
    pub direct: url::Url,
}
```

`src/error.rs` (tests first):

```rust
//! Upload errors and the sentence the user sees.

use std::fmt;

#[cfg(test)]
mod tests {
    use super::*;

    // Review Focus 1.
    #[test]
    fn http_error_message_is_short_and_safe() {
        let body = format!("a{}", "é".repeat(400)).into_bytes(); // 801 bytes, cut inside a character
        let text = UploadError::Http { action: "S3 upload".into(), status: 403, body }.to_string();
        assert!(text.starts_with("S3 upload failed (HTTP 403). a"), "{text}");
        assert!(text.chars().count() < 340, "{} characters", text.chars().count());
    }

    #[test]
    fn a_message_is_shown_as_is() {
        assert_eq!(UploadError::message("Sign in first.").to_string(), "Sign in first.");
    }
}
```

Run `cargo test -p snapline-upload` → fails. Then add above the tests:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UploadError {
    /// A sentence for the user.
    Message(String),
    /// A request answered with a status outside 200–299. The body helps people fix their settings.
    Http { action: String, status: u16, body: Vec<u8> },
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
            Self::Http { action, status, body } => {
                let start = &body[..body.len().min(300)];
                write!(f, "{action} failed (HTTP {status}). {}", String::from_utf8_lossy(start).trim())
            }
        }
    }
}

impl std::error::Error for UploadError {}
```

Add `pub mod error;` (already in lib.rs). Run → passes.

- [ ] **Step 3: The transport, test first**

`src/http.rs` tests (the ureq tests talk to a tiny server on 127.0.0.1, so they run offline):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Answers one request with `reply` and hands back what it received.
    fn one_shot_server(reply: &'static str) -> (url::Url, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = url::Url::parse(&format!("http://{}/upload", listener.local_addr().unwrap())).unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                head.push_str(&line);
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let mut stream = stream;
            stream.write_all(reply.as_bytes()).unwrap();
            head + &String::from_utf8(body).unwrap()
        });
        (url, handle)
    }

    #[test]
    fn ureq_sends_the_body_with_its_length_and_reports_progress() {
        let (url, server) = one_shot_server("HTTP/1.1 201 Created\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let request = Request::new("PUT", url).header("Content-Type", "image/png").body(b"hello".to_vec());
        let response = UreqTransport::new().send(&request, &move |p| log.lock().unwrap().push(p)).unwrap();
        assert_eq!((response.status, response.body.as_slice()), (201, b"ok".as_slice()));
        let received = server.join().unwrap().to_ascii_lowercase();
        assert!(received.contains("content-length: 5") && received.ends_with("hello"), "{received}");
        assert!((seen.lock().unwrap().last().copied().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn error_statuses_are_returned_not_raised() {
        let (url, _server) = one_shot_server("HTTP/1.1 404 Not Found\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnope");
        let response = UreqTransport::new().send(&Request::new("GET", url), &|_| {}).unwrap();
        assert_eq!(response.status, 404);
    }

    // Review Focus 1.
    #[test]
    fn unreachable_server_gives_a_sentence() {
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port(); // closed once dropped
        let url = url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let error = UreqTransport::new().send(&Request::new("GET", url), &|_| {}).unwrap_err();
        assert!(error.to_string().starts_with("Snapline could not reach 127.0.0.1"), "{error}");
    }

    #[test]
    fn send_ok_turns_error_statuses_into_errors() {
        let fake = fake::FakeTransport::new(vec![(200, "fine"), (500, "boom")]);
        let request = Request::new("GET", url::Url::parse("https://example.com/").unwrap());
        assert_eq!(send_ok(&fake, &request, "Test", &|_| {}).unwrap(), b"fine");
        assert_eq!(send_ok(&fake, &request, "Test", &|_| {}).unwrap_err().to_string(), "Test failed (HTTP 500). boom");
        assert_eq!(fake.requests().len(), 2);
    }
}
```

Run → fails. Then write the code above the tests:

```rust
//! HTTP for uploads: plain request and response values, a `Transport` to send them, and the ureq transport.

use crate::error::UploadError;
use snapline_core::sigv4::HttpRequest;
use std::io::Read;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: url::Url,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    #[must_use]
    pub fn new(method: &str, url: url::Url) -> Self {
        Self { method: method.to_owned(), url, headers: Vec::new(), body: Vec::new() }
    }

    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    #[must_use]
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// # Panics
    /// Never: a `serde_json::Value` always serializes.
    #[must_use]
    pub fn json(self, value: &serde_json::Value) -> Self {
        self.header("Content-Type", "application/json").body(serde_json::to_vec(value).expect("JSON value"))
    }

    /// A request signed by `SigV4Signer`, with the body it was signed for.
    #[must_use]
    pub fn from_signed(signed: HttpRequest, body: Vec<u8>) -> Self {
        Self { method: signed.method, url: signed.url, headers: signed.headers, body }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait Transport: Send + Sync {
    /// Sends `request`. `progress` gets 0…1 as the body goes out. Any HTTP status is a `Response`;
    /// only a failure to talk to the server is an error.
    fn send(&self, request: &Request, progress: &dyn Fn(f64)) -> Result<Response, UploadError>;
}

/// Sends and returns the body, turning statuses outside 200–299 into `UploadError::Http`.
pub fn send_ok(t: &dyn Transport, request: &Request, action: &str, progress: &dyn Fn(f64)) -> Result<Vec<u8>, UploadError> {
    let response = t.send(request, progress)?;
    if (200..300).contains(&response.status) {
        Ok(response.body)
    } else {
        Err(UploadError::Http { action: action.to_owned(), status: response.status, body: response.body })
    }
}

/// The real transport: ureq with rustls. Two minutes per request, so a dead network never hangs the app.
pub struct UreqTransport(ureq::Agent);

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .user_agent("Snapline")
            .build();
        Self(config.into())
    }
}

/// Reads the body out and reports how much has gone.
struct Progress<'a> {
    body: &'a [u8],
    sent: usize,
    report: &'a dyn Fn(f64),
}

impl Read for Progress<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = (&self.body[self.sent..]).read(buf)?;
        self.sent += n;
        (self.report)(self.sent as f64 / self.body.len().max(1) as f64);
        Ok(n)
    }
}

impl Transport for UreqTransport {
    fn send(&self, request: &Request, progress: &dyn Fn(f64)) -> Result<Response, UploadError> {
        let host = request.url.host_str().unwrap_or("the server").to_owned();
        let unreachable = |e: ureq::Error| UploadError::message(format!("Snapline could not reach {host}: {e}"));
        let mut builder = ureq::http::Request::builder().method(request.method.as_str()).uri(request.url.as_str());
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let mut response = if request.body.is_empty() {
            let r = builder.body(()).map_err(|e| UploadError::message(e.to_string()))?;
            self.0.run(r).map_err(unreachable)?
        } else {
            // An explicit length, so S3 gets Content-Length rather than chunked encoding.
            builder = builder.header("Content-Length", request.body.len().to_string());
            let mut reader = Progress { body: &request.body, sent: 0, report: progress };
            let r = builder
                .body(ureq::SendBody::from_reader(&mut reader))
                .map_err(|e| UploadError::message(e.to_string()))?;
            self.0.run(r).map_err(unreachable)?
        };
        progress(1.0);
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_vec().map_err(unreachable)?;
        Ok(Response { status, body })
    }
}

/// A scripted transport for tests: answers with the given replies in order and records every request.
#[cfg(any(test, feature = "fake"))]
pub mod fake {
    use super::{Request, Response, Transport};
    use crate::error::UploadError;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    pub struct FakeTransport {
        sent: Mutex<Vec<Request>>,
        replies: Mutex<VecDeque<Response>>,
    }

    impl FakeTransport {
        #[must_use]
        pub fn new(replies: Vec<(u16, &str)>) -> Self {
            let replies = replies.into_iter().map(|(status, body)| Response { status, body: body.as_bytes().to_vec() });
            Self { sent: Mutex::new(Vec::new()), replies: Mutex::new(replies.collect()) }
        }

        /// # Panics
        /// If a previous test thread panicked while holding the lock.
        #[must_use]
        pub fn requests(&self) -> Vec<Request> {
            self.sent.lock().unwrap().clone()
        }
    }

    impl Transport for FakeTransport {
        fn send(&self, request: &Request, progress: &dyn Fn(f64)) -> Result<Response, UploadError> {
            self.sent.lock().unwrap().push(request.clone());
            progress(1.0);
            Ok(self.replies.lock().unwrap().pop_front().unwrap_or(Response { status: 599, body: b"no reply scripted".to_vec() }))
        }
    }
}
```

If the ureq 3 builder API differs (for example `SendBody::from_reader` taking an owned reader), adapt the two `run` calls without changing behaviour. If an explicit `Content-Length` with a reader body is rejected, send `request.body.as_slice()` instead and report progress only at the end; record that ruling.

- [ ] **Step 4: Run the tests**

Run: `cd windows-linux && cargo test -p snapline-core -p snapline-upload && cargo clippy -p snapline-core -p snapline-upload --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass; clean.

- [ ] **Step 5: Commit**

```bash
git add windows-linux
git commit -m "Add the upload crate with its HTTP transport, and search links"
```

---

### Task 2: S3 upload

**Files:**
- Create: `windows-linux/crates/snapline-upload/src/s3.rs`; add `pub mod s3;` to `lib.rs`

**Interfaces:**
- Consumes: `send_ok`, `Request::from_signed`, `snapline_core::{percent_encoding::{s3_urls, s3_upload_request}, sigv4::SigV4Signer, naming::object_key}`.
- Produces: `s3::S3Config { endpoint, region, bucket, prefix, public_base_url, access_key, secret_key: String }` (`Clone`, `Debug`, `Default`, `PartialEq`), `s3::MISSING: &str`, `s3::upload(&dyn Transport, &S3Config, png: &[u8], now: DateTime<Local>, progress: &dyn Fn(f64)) -> Result<UploadResult, UploadError>`.

- [ ] **Step 1: Write the failing tests**

```rust
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
        assert!(result.page.as_str().starts_with("https://shots.example.com/snapline/"), "{}", result.page);
        assert_eq!(result.page, result.direct);
        let sent = &fake.requests()[0];
        assert_eq!(sent.method, "PUT");
        assert_eq!(sent.url.host_str(), Some("acct.r2.cloudflarestorage.com"));
        assert_eq!(sent.body, b"PNG");
        let auth = sent.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("authorization")).unwrap();
        assert!(auth.1.starts_with("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/"), "{}", auth.1);
        assert!(auth.1.contains("/auto/s3/"), "an empty region means auto: {}", auth.1);
    }

    #[test]
    fn missing_settings_are_explained_without_sending_anything() {
        let fake = FakeTransport::new(vec![]);
        let mut c = config();
        c.secret_key.clear();
        assert_eq!(upload(&fake, &c, b"PNG", now(), &|_| {}).unwrap_err(), UploadError::message(MISSING));
        assert_eq!(fake.requests(), []);
    }

    // Review Focus 4.
    #[test]
    fn s3_error_never_shows_the_secret() {
        let fake = FakeTransport::new(vec![(403, "<Error><Code>AccessDenied</Code></Error>")]);
        let text = upload(&fake, &config(), b"PNG", now(), &|_| {}).unwrap_err().to_string();
        assert!(text.starts_with("S3 upload failed (HTTP 403)") && text.contains("AccessDenied"), "{text}");
        assert!(!text.contains(&config().secret_key));
    }
}
```

Run `cargo test -p snapline-upload s3` → fails.

- [ ] **Step 2: Write the uploader**

```rust
//! Upload to an S3-compatible bucket (AWS S3, Cloudflare R2, MinIO). Twin of `S3Uploader` on macOS.

use crate::UploadResult;
use crate::error::UploadError;
use crate::http::{Request, Transport, send_ok};
use chrono::{DateTime, Local, Utc};
use snapline_core::naming;
use snapline_core::percent_encoding::{s3_upload_request, s3_urls};
use snapline_core::sigv4::SigV4Signer;

pub const MISSING: &str =
    "Fill in the S3 settings in Snapline's upload settings (HTTPS endpoint, bucket, public URL and keys).";

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
        || s3_urls(&config.endpoint, &config.bucket, "check.png", &config.public_base_url).is_none()
    {
        return Err(UploadError::message(MISSING));
    }
    let region = if config.region.trim().is_empty() { "auto" } else { config.region.trim() };
    let signer = SigV4Signer::new(access, secret, region);
    let key = naming::object_key(&config.prefix, now);
    let (signed, public) = s3_upload_request(
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
    send_ok(t, &Request::from_signed(signed, png.to_vec()), "S3 upload", progress)?;
    Ok(UploadResult { page: public.clone(), direct: public })
}
```

- [ ] **Step 3: Run the tests**

Run: `cd windows-linux && cargo test -p snapline-upload && cargo clippy -p snapline-upload --all-targets -- -D warnings`
Expected: pass. If the credential scope in the Authorization header does not contain `/auto/s3/`, check how `SigV4Signer` writes the scope and adjust the assertion to the signer's real format (the region must be `auto`).

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Upload screenshots to S3-compatible buckets"
```

---

### Task 3: Google sign-in and token refresh

**Files:**
- Create: `windows-linux/crates/snapline-upload/src/google.rs`; add `pub mod google;` to `lib.rs`

**Interfaces:**
- Consumes: `snapline_core::google_oauth::{pkce, authorization_url, code_from_request_line, callback_page, form_body, is_invalid_grant, email_from_id_token, TokenResponse, TOKEN_URL, REVOKE_URL}`, `snapline_core::percent_encoding::query_items`, `send_ok`.
- Produces:
  - `google::GoogleClient { id: String, secret: String }` with `is_complete() -> bool`.
  - `google::AccessToken { token: String, expires: Instant }` with `is_fresh() -> bool`.
  - `google::RefreshError { Expired, Failed(UploadError) }`.
  - `google::refresh(&dyn Transport, &GoogleClient, refresh_token: &str) -> Result<AccessToken, RefreshError>`.
  - `google::SignedIn { refresh_token: String, email: Option<String>, access: AccessToken }`.
  - `google::sign_in(&dyn Transport, &GoogleClient, open_browser: &dyn Fn(&url::Url) -> bool, timeout: Duration) -> Result<SignedIn, UploadError>`.
  - `google::sign_out(&dyn Transport, refresh_token: &str)` (best effort).
  - `google::EXPIRED: &str`, `google::NOT_SIGNED_IN: &str`.

How sign-in works (same as macOS): listen on `127.0.0.1:<random port>`, open Google's page in the browser with PKCE and a random `state`, wait up to `timeout` for the one request that carries our `state` (anything else, like `/favicon.ico`, gets a 404), exchange the code, and only then answer the browser with the branded page, so it says "signed in" only when that is true.

- [ ] **Step 1: Write the failing tests**

```rust
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
        GoogleClient { id: "id.apps.googleusercontent.com".into(), secret: "client-secret".into() }
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
            let query: std::collections::HashMap<_, _> = query_items(auth.query().unwrap()).into_iter().collect();
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
        assert!(body.contains("grant_type=refresh_token") && body.contains("refresh_token=R"), "{body}");
    }

    // Review Focus 2.
    #[test]
    fn refresh_reports_a_revoked_sign_in() {
        let fake = FakeTransport::new(vec![(400, r#"{"error":"invalid_grant"}"#)]);
        assert!(matches!(refresh(&fake, &client(), "R"), Err(RefreshError::Expired)));
        let fake = FakeTransport::new(vec![(500, "oops")]);
        assert!(matches!(refresh(&fake, &client(), "R"), Err(RefreshError::Failed(_))));
    }

    #[test]
    fn sign_in_exchanges_the_code_and_tells_the_browser() {
        let token = format!(r#"{{"access_token":"A","expires_in":3600,"refresh_token":"R","id_token":"{ID_TOKEN}"}}"#);
        let fake = FakeTransport::new(vec![(200, token.as_str())]);
        let page = Arc::new(Mutex::new(String::new()));
        let signed_in = sign_in(&fake, &client(), &browser(page.clone()), Duration::from_secs(10)).unwrap();
        assert_eq!(signed_in.refresh_token, "R");
        assert_eq!(signed_in.email.as_deref(), Some("a@example.com"));
        let body = String::from_utf8(fake.requests()[0].body.clone()).unwrap();
        assert!(body.contains("code=CODE") && body.contains("code_verifier="), "{body}");
        std::thread::sleep(Duration::from_millis(200));
        let page = page.lock().unwrap().clone();
        assert!(page.starts_with("HTTP/1.1 200 OK") && page.contains("Cache-Control: no-store"), "{page}");
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
        let error = sign_in(&fake, &client(), &|_: &url::Url| true, Duration::from_millis(300)).unwrap_err();
        assert_eq!(error.to_string(), "Google sign-in timed out. Try again.");
    }

    #[test]
    fn a_browser_that_does_not_open_is_reported() {
        let error = sign_in(&FakeTransport::new(vec![]), &client(), &|_: &url::Url| false, Duration::from_secs(1)).unwrap_err();
        assert!(error.to_string().contains("could not open the browser"), "{error}");
    }
}
```

Run `cargo test -p snapline-upload google` → fails.

- [ ] **Step 2: Write sign-in and refresh**

```rust
//! Google sign-in with the OAuth "installed app" flow (browser + PKCE + one request on 127.0.0.1), and
//! access-token refresh. Twin of `GoogleAuth` on macOS; the caller stores the refresh token.

use crate::error::UploadError;
use crate::http::{Request, Transport, send_ok};
use snapline_core::google_oauth::{
    REVOKE_URL, TOKEN_URL, TokenResponse, authorization_url, callback_page, code_from_request_line, email_from_id_token,
    form_body, is_invalid_grant, pkce,
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

#[derive(Clone, Debug)]
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
        Self { token: response.access_token.clone(), expires: Instant::now() + Duration::from_secs(seconds) }
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

fn token_request(t: &dyn Transport, fields: &[(&str, &str)]) -> Result<TokenResponse, UploadError> {
    let request = Request::new("POST", url::Url::parse(TOKEN_URL).expect("fixed URL"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(fields).into_bytes());
    let body = send_ok(t, &request, "Google token request", &|_| {})?;
    serde_json::from_slice(&body).map_err(|e| UploadError::message(format!("Google sent an unexpected answer: {e}")))
}

/// A new access token from the saved refresh token.
pub fn refresh(t: &dyn Transport, client: &GoogleClient, refresh_token: &str) -> Result<AccessToken, RefreshError> {
    let fields = [
        ("client_id", client.id.trim()),
        ("client_secret", client.secret.trim()),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    match token_request(t, &fields) {
        Ok(response) => Ok(AccessToken::from(&response)),
        Err(UploadError::Http { status: 400, body, .. }) if is_invalid_grant(&body) => Err(RefreshError::Expired),
        Err(e) => Err(RefreshError::Failed(e)),
    }
}

/// Signs in through the browser. `open_browser` opens the URL and says whether it could.
pub fn sign_in(
    t: &dyn Transport,
    client: &GoogleClient,
    open_browser: &dyn Fn(&url::Url) -> bool,
    timeout: Duration,
) -> Result<SignedIn, UploadError> {
    let io = |e: std::io::Error| UploadError::message(format!("Google sign-in could not listen on this PC: {e}"));
    let listener = TcpListener::bind("127.0.0.1:0").map_err(io)?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr().map_err(io)?.port());
    let (state, verifier) = (pkce::make_verifier(), pkce::make_verifier());
    let url = authorization_url(client.id.trim(), &redirect, &pkce::challenge(&verifier), &state);
    if !open_browser(&url) {
        return Err(UploadError::message("Snapline could not open the browser for Google sign-in."));
    }
    let (mut stream, line) = wait_for_redirect(&listener, &state, timeout)?;
    let result = (|| {
        let code = code_from_request_line(&line, &state).map_err(|e| UploadError::message(e.0))?;
        let token = token_request(
            t,
            &[
                ("code", &code),
                ("client_id", client.id.trim()),
                ("client_secret", client.secret.trim()),
                ("redirect_uri", &redirect),
                ("grant_type", "authorization_code"),
                ("code_verifier", &verifier),
            ],
        )?;
        let refresh_token =
            token.refresh_token.clone().ok_or_else(|| UploadError::message("Google did not return a refresh token."))?;
        let email = token.id_token.as_deref().and_then(email_from_id_token);
        Ok(SignedIn { refresh_token, email, access: AccessToken::from(&token) })
    })();
    let reason = result.as_ref().err().map(ToString::to_string).unwrap_or_default();
    reply(&mut stream, result.is_ok(), &reason);
    result
}

/// Waits for the request that carries `state`; anything else gets a 404.
fn wait_for_redirect(listener: &TcpListener, state: &str, timeout: Duration) -> Result<(TcpStream, String), UploadError> {
    let deadline = Instant::now() + timeout;
    listener.set_nonblocking(true).map_err(|e| UploadError::message(e.to_string()))?;
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
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
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
pub fn sign_out(t: &dyn Transport, refresh_token: &str) {
    let request = Request::new("POST", url::Url::parse(REVOKE_URL).expect("fixed URL"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(&[("token", refresh_token)]).into_bytes());
    let _ = t.send(&request, &|_| {});
}
```

If `CallbackError` has no public `.0`, use its `Display`. `serde_json` must be in `[dependencies]` (Task 1 added it).

- [ ] **Step 3: Run the tests**

Run: `cd windows-linux && cargo test -p snapline-upload && cargo clippy -p snapline-upload --all-targets -- -D warnings`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Sign in to Google and refresh access tokens"
```

---

### Task 4: Google Drive upload

**Files:**
- Create: `windows-linux/crates/snapline-upload/src/drive.rs`; add `pub mod drive;` to `lib.rs`

**Interfaces:**
- Consumes: `send_ok`, `Request`, `snapline_core::drive_multipart::{body, direct_image_url, DriveFileMetadata}`, `snapline_core::sigv4::SigV4Signer::sha256_hex`, `pkce::make_verifier`.
- Produces: `drive::DriveUpload { result: UploadResult, folder_id: String }`, `drive::upload(&dyn Transport, access_token: &str, folder_id: Option<&str>, png: &[u8], file_name: &str, progress: &dyn Fn(f64)) -> Result<DriveUpload, UploadError>`.

- [ ] **Step 1: Write the failing tests**

```rust
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
        assert_eq!(done.result.page.as_str(), "https://drive.google.com/file/d/X1/view");
        assert!(done.result.direct.as_str().contains("X1"));
        let sent = fake.requests();
        assert!(String::from_utf8_lossy(&sent[0].body).contains("application/vnd.google-apps.folder"));
        let upload_type = sent[1].headers.iter().find(|(k, _)| k == "Content-Type").unwrap();
        assert!(upload_type.1.starts_with("multipart/related; boundary=snapline-"), "{}", upload_type.1);
        assert!(String::from_utf8_lossy(&sent[1].body).contains(r#""parents":["F1"]"#));
        assert!(sent[2].url.path().ends_with("/files/X1/permissions"));
        assert!(String::from_utf8_lossy(&sent[2].body).contains(r#""type":"anyone""#));
        assert!(sent.iter().all(|r| r.headers.contains(&("Authorization".into(), "Bearer TOKEN".into()))));
    }

    #[test]
    fn a_saved_folder_is_reused() {
        let fake = FakeTransport::new(vec![(200, r#"{"id":"F1","trashed":false}"#), (200, FILE), (200, "{}")]);
        let done = upload(&fake, "TOKEN", Some("F1"), b"PNG", "Shot.png", &|_| {}).unwrap();
        assert_eq!(done.folder_id, "F1");
        assert_eq!(fake.requests().len(), 3);
    }

    #[test]
    fn a_trashed_or_deleted_folder_is_made_again() {
        let fake = FakeTransport::new(vec![(200, r#"{"id":"F1","trashed":true}"#), (200, r#"{"id":"F2"}"#), (200, FILE), (200, "{}")]);
        assert_eq!(upload(&fake, "T", Some("F1"), b"PNG", "Shot.png", &|_| {}).unwrap().folder_id, "F2");
        let fake = FakeTransport::new(vec![(404, "{}"), (200, r#"{"id":"F3"}"#), (200, FILE), (200, "{}")]);
        assert_eq!(upload(&fake, "T", Some("F1"), b"PNG", "Shot.png", &|_| {}).unwrap().folder_id, "F3");
    }

    #[test]
    fn upload_errors_name_the_step() {
        let fake = FakeTransport::new(vec![(200, r#"{"id":"F1"}"#), (403, "quota")]);
        let text = upload(&fake, "T", None, b"PNG", "Shot.png", &|_| {}).unwrap_err().to_string();
        assert_eq!(text, "Drive upload failed (HTTP 403). quota");
    }
}
```

Run `cargo test -p snapline-upload drive` → fails.

- [ ] **Step 2: Write the Drive uploader**

```rust
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
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,webViewLink";

pub struct DriveUpload {
    pub result: UploadResult,
    /// The Snapline folder used, for the caller to remember.
    pub folder_id: String,
}

/// `files/<id>/<tail…>`, with the id as one path segment (it comes from settings or the server).
fn file_url(id: &str, tail: &[&str]) -> url::Url {
    let mut url = url::Url::parse(FILES).expect("fixed URL");
    url.path_segments_mut().expect("https URL").push(id).extend(tail);
    url
}

fn authorized(request: Request, token: &str) -> Request {
    request.header("Authorization", &format!("Bearer {token}"))
}

fn parse<T: for<'de> Deserialize<'de>>(body: &[u8], what: &str) -> Result<T, UploadError> {
    serde_json::from_slice(body).map_err(|e| UploadError::message(format!("Drive sent an unexpected {what}: {e}")))
}

/// The saved folder if it still exists and is not trashed; otherwise a new one.
fn folder(t: &dyn Transport, token: &str, saved: Option<&str>) -> Result<String, UploadError> {
    #[derive(Deserialize)]
    struct Folder {
        trashed: bool,
    }
    if let Some(id) = saved.filter(|id| !id.is_empty()) {
        let mut url = file_url(id, &[]);
        url.set_query(Some("fields=id,trashed"));
        let response = t.send(&authorized(Request::new("GET", url), token), &|_| {})?;
        if response.status == 200 && parse::<Folder>(&response.body, "folder").is_ok_and(|f| !f.trashed) {
            return Ok(id.to_owned());
        }
    }
    #[derive(Deserialize)]
    struct Created {
        id: String,
    }
    let mut url = url::Url::parse(FILES).expect("fixed URL");
    url.set_query(Some("fields=id"));
    let request = authorized(Request::new("POST", url), token)
        .json(&serde_json::json!({ "name": "Snapline", "mimeType": "application/vnd.google-apps.folder" }));
    let body = send_ok(t, &request, "Creating the Snapline folder", &|_| {})?;
    Ok(parse::<Created>(&body, "folder")?.id)
}

pub fn upload(
    t: &dyn Transport,
    access_token: &str,
    folder_id: Option<&str>,
    png: &[u8],
    file_name: &str,
    progress: &dyn Fn(f64),
) -> Result<DriveUpload, UploadError> {
    let folder = folder(t, access_token, folder_id)?;
    // Hex only: a multipart boundary may not contain "~", which PKCE strings can.
    let boundary = format!("snapline-{}", &SigV4Signer::sha256_hex(pkce::make_verifier().as_bytes())[..32]);
    let metadata = DriveFileMetadata { name: file_name.to_owned(), parents: vec![folder.clone()] };
    let request = authorized(Request::new("POST", url::Url::parse(UPLOAD).expect("fixed URL")), access_token)
        .header("Content-Type", &format!("multipart/related; boundary={boundary}"))
        .body(body(&boundary, &metadata, png, "image/png"));
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        id: String,
        web_view_link: String,
    }
    let file: File = parse(&send_ok(t, &request, "Drive upload", progress)?, "file")?;

    let share = authorized(Request::new("POST", file_url(&file.id, &["permissions"])), access_token)
        .json(&serde_json::json!({ "type": "anyone", "role": "reader" }));
    send_ok(t, &share, "Drive sharing", &|_| {})?;

    let page = url::Url::parse(&file.web_view_link).map_err(|_| UploadError::message("Drive returned no link."))?;
    Ok(DriveUpload { result: UploadResult { page, direct: direct_image_url(&file.id) }, folder_id: folder })
}
```

Run `cargo add -p snapline-upload serde --features derive` first if it is not there yet (Task 1 added it).

- [ ] **Step 3: Run the tests**

Run: `cd windows-linux && cargo test -p snapline-upload && cargo clippy -p snapline-upload --all-targets -- -D warnings`
Expected: pass. The JSON key order in `"type":"anyone"` comes from `serde_json::json!` (sorted by default); if the assertion string differs only in spacing or order, assert on the parsed JSON instead.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Upload screenshots to Google Drive and share them by link"
```

---

### Task 5: Secrets, upload settings, and where an upload goes

**Files:**
- Create: `windows-linux/crates/snapline/src/secrets.rs`, `windows-linux/crates/snapline/src/upload_job.rs` (the `destination` part; Task 8 adds `run`)
- Modify: `windows-linux/crates/snapline/src/settings.rs`, `main.rs` (`mod secrets; mod upload_job;`), `Cargo.toml`

**Interfaces:**
- Consumes: `snapline_upload::{s3::S3Config, google::GoogleClient}`.
- Produces:
  - `secrets::Secret { S3AccessKey, S3SecretKey, GoogleClientSecret, GoogleRefreshToken }` with `account() -> &'static str`.
  - `secrets::SecretStore` (`get(Secret) -> Option<String>`, `put(Secret, &str) -> Result<(), String>`, `remove(Secret) -> Result<(), String>`), `secrets::save(&dyn SecretStore, Secret, &str) -> Result<(), String>` (trims; empty removes), `secrets::Keyring` (`open() -> Result<Keyring, String>`), and for tests `secrets::MemoryStore` (`Default`).
  - `settings::UploadTarget { S3, Drive }`; new `Settings` fields `upload_target`, `s3_endpoint`, `s3_region` (default `"auto"`), `s3_bucket`, `s3_prefix` (default `"snapline"`), `s3_public_url`, `google_client_id`, `google_email`, `drive_folder_id`; `Settings::save(&self, &Path) -> Result<(), String>`; `Settings::update(&Path, impl FnOnce(&mut Settings)) -> Result<Settings, String>`.
  - `upload_job::Destination { S3(S3Config), Drive { client: GoogleClient, refresh_token: Option<String>, folder_id: Option<String> } }`, `upload_job::destination(&Settings, &dyn SecretStore) -> Destination`.

- [ ] **Step 1: Add dependencies**

```bash
cd windows-linux
cargo add -p snapline keyring-core
cargo add -p snapline windows-native-keyring-store --target 'cfg(windows)'
cargo add -p snapline apple-native-keyring-store --target 'cfg(target_os = "macos")' --features keychain
cargo add -p snapline snapline-upload --path crates/snapline-upload
cargo add -p snapline --dev snapline-upload --path crates/snapline-upload --features fake
```

- [ ] **Step 2: Write the failing tests**

`src/secrets.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_match_the_macos_keychain_names() {
        let names: Vec<_> = [Secret::S3AccessKey, Secret::S3SecretKey, Secret::GoogleClientSecret, Secret::GoogleRefreshToken]
            .map(Secret::account)
            .into();
        assert_eq!(names, ["s3AccessKey", "s3SecretKey", "googleClientSecret", "googleRefreshToken"]);
    }

    #[test]
    fn saving_trims_and_an_empty_value_removes_the_secret() {
        let store = MemoryStore::default();
        save(&store, Secret::S3SecretKey, "  abc \n").unwrap();
        assert_eq!(store.get(Secret::S3SecretKey).as_deref(), Some("abc"));
        save(&store, Secret::S3SecretKey, "   ").unwrap();
        assert_eq!(store.get(Secret::S3SecretKey), None);
        save(&store, Secret::S3SecretKey, "").unwrap(); // removing what is not there is fine
    }
}
```

Add to `mod tests` in `src/settings.rs`:

```rust
    #[test]
    fn upload_settings_round_trip_through_the_file() {
        let path = temp_file("roundtrip", "");
        let mut s = Settings::default();
        s.upload_target = UploadTarget::Drive;
        s.google_client_id = "id.apps.googleusercontent.com".into();
        s.save(&path).unwrap();
        let (loaded, problem) = Settings::load(&path);
        assert!(problem.is_none());
        assert_eq!(loaded, s);
        assert_eq!((Settings::default().s3_region.as_str(), Settings::default().s3_prefix.as_str()), ("auto", "snapline"));
    }

    #[test]
    fn update_keeps_what_it_does_not_change() {
        let path = temp_file("update", "tray_click = \"capture\"\n");
        Settings::update(&path, |s| s.upload_target = UploadTarget::Drive).unwrap();
        let (s, _) = Settings::load(&path);
        assert_eq!((s.tray_click, s.upload_target), (TrayClick::Capture, UploadTarget::Drive));
    }

    // Review Focus 5.
    #[test]
    fn update_refuses_to_overwrite_a_broken_file() {
        let text = "upload_target = \"s3\n";
        let path = temp_file("broken-update", text);
        assert!(Settings::update(&path, |s| s.s3_bucket = "x".into()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
```

`src/upload_job.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{MemoryStore, Secret, SecretStore};
    use crate::settings::UploadTarget;

    // Review Focus 5: every upload reads the file again, so a change in the settings window applies at once.
    #[test]
    fn upload_target_follows_the_file() {
        let store = MemoryStore::default();
        store.put(Secret::S3AccessKey, "AK").unwrap();
        store.put(Secret::S3SecretKey, "SK").unwrap();
        store.put(Secret::GoogleClientSecret, "CS").unwrap();
        let dir = std::env::temp_dir().join(format!("snapline-dest-{}", std::process::id()));
        let path = dir.join("settings.toml");
        Settings::update(&path, |s| {
            s.s3_bucket = "shots".into();
            s.s3_endpoint = "https://acct.r2.cloudflarestorage.com".into();
        })
        .unwrap();
        let Destination::S3(config) = destination(&Settings::load(&path).0, &store) else { panic!("expected S3") };
        assert_eq!((config.bucket.as_str(), config.access_key.as_str(), config.secret_key.as_str()), ("shots", "AK", "SK"));

        Settings::update(&path, |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
        })
        .unwrap();
        let Destination::Drive { client, refresh_token, folder_id } = destination(&Settings::load(&path).0, &store) else {
            panic!("expected Drive")
        };
        assert_eq!((client.id.as_str(), client.secret.as_str()), ("ID", "CS"));
        assert_eq!((refresh_token, folder_id), (None, None));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
```

Add `mod secrets;` and `mod upload_job;` (with `#[allow(dead_code)] // wired up in Task 8`) to `main.rs`. Run `cargo test -p snapline` → fails.

- [ ] **Step 3: Write the secret store**

Above the tests in `src/secrets.rs`:

```rust
//! Secrets in the OS credential store (Credential Manager on Windows, the Keychain on macOS), never in
//! `settings.toml`. Same service and account names as the macOS app.

use std::collections::HashMap;
use std::sync::Mutex;

const SERVICE: &str = "dev.barayuda.snapline";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Secret {
    S3AccessKey,
    S3SecretKey,
    GoogleClientSecret,
    GoogleRefreshToken,
}

impl Secret {
    #[must_use]
    pub const fn account(self) -> &'static str {
        match self {
            Self::S3AccessKey => "s3AccessKey",
            Self::S3SecretKey => "s3SecretKey",
            Self::GoogleClientSecret => "googleClientSecret",
            Self::GoogleRefreshToken => "googleRefreshToken",
        }
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, secret: Secret) -> Option<String>;
    fn put(&self, secret: Secret, value: &str) -> Result<(), String>;
    /// Removing a secret that is not there succeeds.
    fn remove(&self, secret: Secret) -> Result<(), String>;
}

/// Saves `value` without surrounding spaces; an empty value removes the secret.
pub fn save(store: &dyn SecretStore, secret: Secret, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() { store.remove(secret) } else { store.put(secret, value) }
}

/// The OS credential store, chosen once with `open`.
#[derive(Clone, Copy)]
pub struct Keyring;

impl Keyring {
    /// # Errors
    /// When the OS store cannot be opened.
    pub fn open() -> Result<Self, String> {
        #[cfg(windows)]
        let store = windows_native_keyring_store::Store::new().map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        let store = apple_native_keyring_store::keychain::Store::new().map_err(|e| e.to_string())?;
        // ponytail: Linux gets the Secret Service store in Phase 6; until then secrets last for one run.
        #[cfg(not(any(windows, target_os = "macos")))]
        let store = keyring_core::mock::Store::new().map_err(|e| e.to_string())?;
        keyring_core::set_default_store(store);
        Ok(Self)
    }

    fn entry(secret: Secret) -> Result<keyring_core::Entry, String> {
        keyring_core::Entry::new(SERVICE, secret.account()).map_err(|e| e.to_string())
    }
}

impl SecretStore for Keyring {
    fn get(&self, secret: Secret) -> Option<String> {
        Self::entry(secret).ok()?.get_password().ok()
    }

    fn put(&self, secret: Secret, value: &str) -> Result<(), String> {
        Self::entry(secret)?.set_password(value).map_err(|e| e.to_string())
    }

    fn remove(&self, secret: Secret) -> Result<(), String> {
        match Self::entry(secret)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// An in-memory store for tests (keyring-core's mock store forgets values between entries).
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<Secret, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, secret: Secret) -> Option<String> {
        self.0.lock().ok()?.get(&secret).cloned()
    }

    fn put(&self, secret: Secret, value: &str) -> Result<(), String> {
        self.0.lock().map_err(|e| e.to_string())?.insert(secret, value.to_owned());
        Ok(())
    }

    fn remove(&self, secret: Secret) -> Result<(), String> {
        self.0.lock().map_err(|e| e.to_string())?.remove(&secret);
        Ok(())
    }
}
```

If clippy flags `MemoryStore` as unused outside tests, mark it `#[cfg(test)]`; Task 9's tests use it too.

- [ ] **Step 4: Extend the settings**

In `src/settings.rs`, add the target and fields:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadTarget {
    #[default]
    S3,
    Drive,
}
```

Add to `Settings` (after `tray_click`):

```rust
    pub upload_target: UploadTarget,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_bucket: String,
    pub s3_prefix: String,
    pub s3_public_url: String,
    pub google_client_id: String,
    /// Shown in the settings window after sign-in.
    pub google_email: String,
    /// The Snapline folder in My Drive, once created.
    pub drive_folder_id: String,
```

and to `Default`: `upload_target: UploadTarget::S3, s3_endpoint: String::new(), s3_region: "auto".into(), s3_bucket: String::new(), s3_prefix: "snapline".into(), s3_public_url: String::new(), google_client_id: String::new(), google_email: String::new(), drive_folder_id: String::new(),`.

Add to `impl Settings`:

```rust
    /// Writes the file (through a temporary file, so a crash never leaves half a file).
    ///
    /// # Errors
    /// When the folder or file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    }

    /// Loads the latest file, applies `change` and saves it. Both Snapline processes write this file,
    /// so neither keeps a stale copy. A file that does not parse is left alone.
    ///
    /// # Errors
    /// When the file cannot be read as settings, or cannot be written.
    pub fn update(path: &Path, change: impl FnOnce(&mut Self)) -> Result<Self, String> {
        let (mut settings, problem) = Self::load(path);
        if let Some(problem) = problem {
            return Err(format!("Snapline did not save, because this file could not be read:\n{problem}"));
        }
        change(&mut settings);
        settings.save(path)?;
        Ok(settings)
    }
```

- [ ] **Step 5: Where an upload goes**

Above the tests in `src/upload_job.rs`:

```rust
//! One upload: from the saved settings and secrets to a result. Task 8 adds `run`.

use crate::secrets::{Secret, SecretStore};
use crate::settings::{Settings, UploadTarget};
use snapline_upload::google::GoogleClient;
use snapline_upload::s3::S3Config;

pub enum Destination {
    S3(S3Config),
    Drive { client: GoogleClient, refresh_token: Option<String>, folder_id: Option<String> },
}

/// Where the next upload goes, from settings read just now and the stored secrets.
pub fn destination(settings: &Settings, secrets: &dyn SecretStore) -> Destination {
    match settings.upload_target {
        UploadTarget::S3 => Destination::S3(S3Config {
            endpoint: settings.s3_endpoint.clone(),
            region: settings.s3_region.clone(),
            bucket: settings.s3_bucket.clone(),
            prefix: settings.s3_prefix.clone(),
            public_base_url: settings.s3_public_url.clone(),
            access_key: secrets.get(Secret::S3AccessKey).unwrap_or_default(),
            secret_key: secrets.get(Secret::S3SecretKey).unwrap_or_default(),
        }),
        UploadTarget::Drive => Destination::Drive {
            client: GoogleClient {
                id: settings.google_client_id.clone(),
                secret: secrets.get(Secret::GoogleClientSecret).unwrap_or_default(),
            },
            refresh_token: secrets.get(Secret::GoogleRefreshToken),
            folder_id: Some(settings.drive_folder_id.clone()).filter(|id| !id.is_empty()),
        },
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cd windows-linux && cargo test -p snapline && cargo clippy -p snapline --all-targets -- -D warnings && cargo clippy -p snapline --all-targets --target x86_64-pc-windows-msvc -- -D warnings`
Expected: pass; clean on both targets.

- [ ] **Step 7: Commit**

```bash
git add windows-linux
git commit -m "Store upload secrets in the OS credential store and add upload settings"
```

---

### Task 6: Upload and Search in the overlay

**Files:**
- Modify: `windows-linux/crates/snapline-overlay/src/session.rs`, `toolbar.rs`, `icons.rs`, `render.rs`

**Interfaces:**
- Consumes: `snapline_core::search::SearchEngine`; Phase 2's `Session`, `toolbar::layout`, `Button`.
- Produces:
  - `session::Command` gains `Upload` and `Search(SearchEngine)`.
  - `Session` gains `pub search_open: bool`.
  - `toolbar::Button` gains `Upload`, `Search`, `Engine(SearchEngine)`; `MAIN_BAR` becomes `[Upload, Search, Copy, Save, Close]` (the macOS order, without the phase-5 buttons).
  - `toolbar::layout(sel, bounds, scale, palette_open, search_open) -> Vec<Bar>`: order is main bar, draw bar, then the palette and the search menu when open.

Behaviour, from macOS: Upload has the tip "Upload and copy link (Ctrl+D)" and Ctrl+D runs it. Search shows a small menu with "Search with Google Lens" and "Search with TinEye"; picking one runs `Command::Search(engine)` and closes the menu. A click anywhere else closes the menu, as it does the palette.

- [ ] **Step 1: Write the failing tests**

In `toolbar.rs` tests: change every `layout(…, palette)` call to `layout(…, palette, false)`, and in `bars_never_overlap` loop over all four `(palette, search)` pairs. Then add:

```rust
    #[test]
    fn search_menu_sits_under_the_search_button() {
        let bars = layout(Rect::new(300.0, 100.0, 300.0, 200.0), SCREEN, 1.0, false, true);
        assert_eq!(bars.len(), 3);
        let menu = &bars[2];
        let search = bars[0].buttons.iter().find(|(b, _)| *b == Button::Search).unwrap().1;
        assert!(menu.frame.y >= bars[0].frame.y + bars[0].frame.height, "{:?}", menu.frame);
        assert!((menu.frame.x - (search.x - 4.0)).abs() < 1.0);
        let engines: Vec<_> = menu.buttons.iter().map(|(b, _)| *b).collect();
        assert_eq!(engines, [Button::Engine(SearchEngine::GoogleLens), Button::Engine(SearchEngine::TinEye)]);
    }
```

In `session.rs` tests (fix any `layout` calls the same way):

```rust
    #[test]
    fn ctrl_d_uploads() {
        let mut s = selected(1.0);
        assert_eq!(s.key(Key::Char('d'), ctrl()).command, Some(Command::Upload));
        assert!(session(1, 1.0).key(Key::Char('d'), ctrl()).beep, "no selection, nothing to upload");
    }

    #[test]
    fn search_opens_a_menu_and_runs_the_picked_engine() {
        let mut s = selected(1.0);
        click_button(&mut s, Button::Search, Mods::default());
        assert!(s.search_open);
        let effect = click_button(&mut s, Button::Engine(SearchEngine::TinEye), Mods::default());
        assert_eq!(effect.command, Some(Command::Search(SearchEngine::TinEye)));
        assert!(!s.search_open);
    }

    #[test]
    fn a_click_elsewhere_closes_the_search_menu() {
        let mut s = selected(1.0);
        click_button(&mut s, Button::Search, Mods::default());
        drag(&mut s, 0, pt(50.0, 30.0), pt(50.0, 30.0));
        assert!(!s.search_open);
    }
```

In `render.rs` tests:

```rust
    #[test]
    fn search_menu_shows_its_labels() {
        let mut s = screen_session(1);
        select(&mut s, Rect::new(60.0, 40.0, 150.0, 80.0));
        s.search_open = true;
        let menu = s.bars(0)[2].buttons[0].1;
        let out = rendered(&s, 0);
        let row = (menu.y + menu.height / 2.0) as u32;
        let light = (menu.x as u32..(menu.x + menu.width) as u32).filter(|&x| rgb(&out, x, row).0 > 200).count();
        assert!(light > 5, "the label text should be drawn");
    }
```

Add `use snapline_core::search::SearchEngine;` where needed. Run `cargo test -p snapline-overlay` → fails.

- [ ] **Step 2: Implement**

`session.rs`:
- `Command`: add `Upload,` and `Search(SearchEngine),`.
- `Session`: add `pub search_open: bool,` (false in `new`).
- `bars()`: pass `self.search_open` to `toolbar::layout`.
- `press()`: at the top, also `if !matches!(button, Button::Search | Button::Engine(_)) { self.search_open = false; }`. New arms:

```rust
            Button::Upload => self.request(Command::Upload),
            Button::Search => {
                self.search_open = !self.search_open;
                self.palette_open = false;
                Effect::all()
            }
            Button::Engine(engine) => {
                self.search_open = false;
                self.request(Command::Search(engine))
            }
```

- `pointer_down()`: next to `self.palette_open = false;` add `self.search_open = false;`.
- `key()`: in the Ctrl match add `'d' => self.request(Command::Upload),`.

`toolbar.rs`:
- `Button`: add `Upload`, `Search`, `Engine(SearchEngine)`. Tips: `Upload => "Upload and copy link (Ctrl+D)"`, `Search => "Search similar images"`, `Engine(_) => ""` (its label is on the button).
- `pub const MAIN_BAR: [Button; 5] = [Button::Upload, Button::Search, Button::Copy, Button::Save, Button::Close];`
- Text buttons for the menu:

```rust
/// The search menu's entries are text, 200 × 28 points.
const MENU_ITEM: Size = Size { width: 200.0, height: 28.0 };

fn menu(buttons: &[Button], origin: Point, s: f64) -> Bar {
    let (w, h, pad, sp) = (MENU_ITEM.width * s, MENU_ITEM.height * s, PAD * s, SPACING * s);
    let n = buttons.len() as f64;
    let frame = Rect::new(origin.x, origin.y, w + 2.0 * pad, 2.0 * pad + n * h + (n - 1.0) * sp);
    let buttons = buttons
        .iter()
        .enumerate()
        .map(|(k, b)| (*b, Rect::new(origin.x + pad, origin.y + pad + k as f64 * (h + sp), w, h)))
        .collect();
    Bar { frame, buttons }
}
```

- `layout(…, palette_open, search_open)`: after the palette, when `search_open`:

```rust
    if search_open {
        let engines = SearchEngine::ALL.map(Button::Engine);
        let probe = menu(&engines, Point::default(), s);
        let search = bars[0].buttons.iter().find(|(b, _)| *b == Button::Search).map_or(main_frame, |(_, r)| *r);
        let below = main_frame.y + main_frame.height + gap;
        let y = if below + probe.frame.height <= bottom { below } else { main_frame.y - probe.frame.height - gap };
        let frame = Rect::new(fit(search.x - PAD * s, right - probe.frame.width), fit(y, bottom - probe.frame.height),
                              probe.frame.width, probe.frame.height);
        let frame = step_aside(step_aside(frame, draw_frame, gap, bounds), main_frame, gap, bounds);
        bars.push(menu(&engines, Point { x: frame.x, y: frame.y }, s));
    }
```

  When both palette and search are open (not reachable from the UI, since each closes the other, but tested), also `step_aside` the menu from the palette frame.

`icons.rs`: `outline()` gains

```rust
        Button::Upload => {
            pb.move_to(12.0, 15.0);
            pb.line_to(12.0, 4.0);
            pb.move_to(7.0, 9.0);
            pb.line_to(12.0, 4.0);
            pb.line_to(17.0, 9.0);
            pb.move_to(5.0, 15.0);
            pb.line_to(5.0, 19.0);
            pb.line_to(19.0, 19.0);
            pb.line_to(19.0, 15.0);
        }
        Button::Search => {
            pb.push_circle(10.0, 10.0, 6.0);
            pb.move_to(14.5, 14.5);
            pb.line_to(20.0, 20.0);
        }
        Button::Engine(_) => {} // a text label, drawn by render
```

`render.rs` `draw_bars`: for `Button::Engine(engine)` draw the label instead of an icon:

```rust
            if let Button::Engine(engine) = button {
                let size = (13.0 * scale) as f32;
                let h = session.font.measure(engine.title(), size).height;
                let at = Point { x: r.x + 8.0 * scale, y: r.y + (r.height - h) / 2.0 };
                session.font.draw(out, engine.title(), at, size, WHITE);
                continue;
            }
```

- [ ] **Step 3: Run the tests**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: pass. The app crate will not compile yet where it matches on `Command`; add `Command::Upload | Command::Search(_) => Ok(())` to `actions::run` for now (Task 8 replaces it) and check `cargo clippy --workspace --all-targets -- -D warnings`.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Add Upload and Search similar images to the overlay toolbar"
```

---

### Task 7: The status popup

**Files:**
- Create: `windows-linux/crates/snapline-overlay/src/popup.rs`; add `pub mod popup;` to `lib.rs`

**Interfaces:**
- Consumes: `TextFont`, `render::fill_rounded` (made `pub(crate)` by this task), `session::contains`, colours.
- Produces:
  - `popup::PopupState { Uploading { progress: f64, file: String }, Done { link: String }, Failed { message: String }, Message { title: String, detail: String } }`.
  - `popup::PopupButton { Copy, Open, Settings, Retry, Close }` with `label() -> &'static str`.
  - `popup::Popup { state: PopupState, hover: Option<PopupButton> }` with `new(PopupState)`, `size(&TextFont, scale) -> Size`, `buttons(&TextFont, scale) -> Vec<(PopupButton, Rect)>`, `hit(&TextFont, scale, Point) -> Option<PopupButton>`, `render(&mut Pixmap, &TextFont, scale)`. All rects are in popup pixels (top-left 0, 0).
  - `popup::origin(anchor: Option<Rect>, size: Size, work_area: Rect, gap: f64) -> Point` (screen pixels).
  - `popup::wrap(&TextFont, &str, size: f32, width: f64) -> Vec<String>`, `popup::shorten_middle(&TextFont, &str, size: f32, width: f64) -> String`.

What it shows (macOS `UploadView` and `StatusPopover.message`), 340 points wide, 14-point padding, on ink at 96 %:

| State | Title (15 pt) | Body (13 pt) | Buttons |
|---|---|---|---|
| Uploading | "Uploading…" (white) | progress bar (violet on a light track), then the file name | — |
| Done | "Uploaded. Link copied." (green #34C759) | the link, shortened in the middle | Copy, Open |
| Failed | "Upload failed" (red #FF3B30) | the message, wrapped | Upload settings…, Try again |
| Message | the title (green) | the detail, wrapped | — |

Every state has a small × at the top-right (`Close`).

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Color;

    const WORK: Rect = Rect::new(0.0, 0.0, 1920.0, 1040.0);
    const SIZE: Size = Size { width: 340.0, height: 120.0 };

    fn inside(p: Point, work: Rect) -> bool {
        p.x >= work.x && p.y >= work.y && p.x + SIZE.width <= work.x + work.width && p.y + SIZE.height <= work.y + work.height
    }

    // Review Focus 3.
    #[test]
    fn popup_stays_in_the_work_area_for_every_taskbar_edge() {
        // Taskbar at the bottom: above the tray icon.
        let p = origin(Some(Rect::new(1800.0, 1048.0, 24.0, 24.0)), SIZE, WORK, 8.0);
        assert!(inside(p, WORK) && p.y + SIZE.height <= 1040.0 && p.x + SIZE.width >= 1800.0, "{p:?}");
        // Taskbar at the top: below it.
        let work = Rect::new(0.0, 40.0, 1920.0, 1040.0);
        let p = origin(Some(Rect::new(1800.0, 8.0, 24.0, 24.0)), SIZE, work, 8.0);
        assert!(inside(p, work) && p.y >= 40.0, "{p:?}");
        // Taskbar on the right: to its left.
        let work = Rect::new(0.0, 0.0, 1860.0, 1080.0);
        let p = origin(Some(Rect::new(1880.0, 1000.0, 24.0, 24.0)), SIZE, work, 8.0);
        assert!(inside(p, work) && p.x + SIZE.width <= 1872.0, "{p:?}");
        // Taskbar on the left: to its right.
        let work = Rect::new(60.0, 0.0, 1860.0, 1080.0);
        let p = origin(Some(Rect::new(16.0, 1000.0, 24.0, 24.0)), SIZE, work, 8.0);
        assert!(inside(p, work) && p.x >= 48.0, "{p:?}");
        // Tray icon unknown (hidden in the overflow): bottom-right corner of the work area.
        let p = origin(None, SIZE, WORK, 8.0);
        assert_eq!(p, Point { x: 1920.0 - 340.0 - 8.0, y: 1040.0 - 120.0 - 8.0 });
    }

    #[test]
    fn long_messages_wrap_within_the_width_and_long_words_break() {
        let font = TextFont::bundled();
        let lines = wrap(&font, "Snapline could not reach shots.example.com: connection refused by the server", 13.0, 200.0);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| font.measure(l, 13.0).width <= 200.0), "{lines:?}");
        let word = wrap(&font, &"x".repeat(200), 13.0, 100.0);
        assert!(word.len() > 1 && word.iter().all(|l| !l.is_empty()));
    }

    #[test]
    fn links_are_shortened_in_the_middle() {
        let font = TextFont::bundled();
        let link = "https://shots.example.com/snapline/2026/10/30/Screenshot-2026-10-30-at-09.15.00.png";
        let short = shorten_middle(&font, link, 13.0, 200.0);
        assert!(short.starts_with("https://") && short.ends_with(".png") && short.contains('…'), "{short}");
        assert!(font.measure(&short, 13.0).width <= 200.0);
    }

    #[test]
    fn buttons_depend_on_the_state_and_can_be_hit() {
        let font = TextFont::bundled();
        let done = Popup::new(PopupState::Done { link: "https://x.example/a.png".into() });
        let kinds: Vec<_> = done.buttons(&font, 1.0).iter().map(|(b, _)| *b).collect();
        assert_eq!(kinds, [PopupButton::Close, PopupButton::Copy, PopupButton::Open]);
        let failed = Popup::new(PopupState::Failed { message: "S3 upload failed (HTTP 403).".into() });
        let retry = failed.buttons(&font, 1.0).into_iter().find(|(b, _)| *b == PopupButton::Retry).unwrap().1;
        let centre = Point { x: retry.x + retry.width / 2.0, y: retry.y + retry.height / 2.0 };
        assert_eq!(failed.hit(&font, 1.0, centre), Some(PopupButton::Retry));
        assert_eq!(failed.hit(&font, 1.0, Point { x: 2.0, y: 2.0 }), None);
    }

    #[test]
    fn progress_bar_fills_to_the_progress() {
        let font = TextFont::bundled();
        let popup = Popup::new(PopupState::Uploading { progress: 0.5, file: "Shot.png".into() });
        let size = popup.size(&font, 1.0);
        let mut out = Pixmap::new(size.width as u32, size.height as u32).unwrap();
        out.fill(Color::BLACK);
        popup.render(&mut out, &font, 1.0);
        let bar = popup.progress_bar(&font, 1.0).unwrap();
        let y = (bar.y + bar.height / 2.0) as u32;
        let at = |f: f64| out.pixel((bar.x + bar.width * f) as u32, y).unwrap().demultiply();
        let (filled, empty) = (at(0.25), at(0.75));
        assert!(filled.blue() > 200 && filled.red() > 100, "violet fill: {filled:?}");
        assert!(empty.blue() < 120, "track only: {empty:?}");
    }

    #[test]
    fn failed_popups_grow_with_their_message() {
        let font = TextFont::bundled();
        let short = Popup::new(PopupState::Failed { message: "Oops.".into() }).size(&font, 1.0);
        let long = Popup::new(PopupState::Failed { message: "word ".repeat(80) }).size(&font, 1.0);
        assert!(long.height > short.height && (long.width - 340.0).abs() < f64::EPSILON);
    }
}
```

Run `cargo test -p snapline-overlay popup` → fails.

- [ ] **Step 2: Write the popup**

Make `fill_rounded` in `render.rs` `pub(crate)`. Then above the tests:

```rust
//! The status popup next to the tray icon: upload progress and result, and short confirmations.
//! Twin of the macOS menu bar popovers (`UploadPopover.swift`, `StatusPopover.message`).

use crate::color::{INK, Rgb, VIOLET, WHITE};
use crate::render::fill_rounded;
use crate::text::TextFont;
use snapline_core::selection_geometry::{Point, Rect, Size};
use tiny_skia::Pixmap;

const WIDTH: f64 = 340.0;
const PAD: f64 = 14.0;
const GAP: f64 = 8.0;
const TITLE: f32 = 15.0;
const BODY: f32 = 13.0;
const BUTTON_H: f64 = 28.0;
const BAR_H: f64 = 6.0;
const CLOSE: f64 = 20.0;
const GREEN: Rgb = Rgb(0x34, 0xC7, 0x59);
const RED: Rgb = Rgb(0xFF, 0x3B, 0x30);
const GREY: Rgb = Rgb(0xB8, 0xB5, 0xC8);

#[derive(Clone, Debug, PartialEq)]
pub enum PopupState {
    Uploading { progress: f64, file: String },
    Done { link: String },
    Failed { message: String },
    Message { title: String, detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupButton {
    Copy,
    Open,
    Settings,
    Retry,
    Close,
}

impl PopupButton {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Open => "Open",
            Self::Settings => "Upload settings…",
            Self::Retry => "Try again",
            Self::Close => "×",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    pub state: PopupState,
    pub hover: Option<PopupButton>,
}

/// Splits `text` into lines no wider than `width`, breaking long words.
#[must_use]
pub fn wrap(font: &TextFont, text: &str, size: f32, width: f64) -> Vec<String> {
    let fits = |s: &str| font.measure(s, size).width <= width;
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
        if fits(&candidate) {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        for c in word.chars() {
            line.push(c);
            if !fits(&line) && line.chars().count() > 1 {
                line.pop();
                lines.push(std::mem::take(&mut line));
                line.push(c);
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// `text` with its middle replaced by "…" until it fits `width`.
#[must_use]
pub fn shorten_middle(font: &TextFont, text: &str, size: f32, width: f64) -> String {
    if font.measure(text, size).width <= width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut keep = chars.len();
    while keep > 2 {
        keep -= 1;
        let head: String = chars[..keep.div_ceil(2)].iter().collect();
        let tail: String = chars[chars.len() - keep / 2..].iter().collect();
        let short = format!("{head}…{tail}");
        if font.measure(&short, size).width <= width {
            return short;
        }
    }
    "…".to_owned()
}

/// Where the popup goes: next to the tray icon (above, below, left or right, whichever fits the work area),
/// or the bottom-right corner of the work area when the icon's place is unknown.
#[must_use]
pub fn origin(anchor: Option<Rect>, size: Size, work: Rect, gap: f64) -> Point {
    let (right, bottom) = (work.x + work.width, work.y + work.height);
    let clamp_x = |x: f64| x.min(right - size.width).max(work.x);
    let clamp_y = |y: f64| y.min(bottom - size.height).max(work.y);
    let fits = |p: &Point| p.x >= work.x && p.y >= work.y && p.x + size.width <= right && p.y + size.height <= bottom;
    if let Some(a) = anchor {
        let centre_x = clamp_x(a.x + a.width / 2.0 - size.width / 2.0);
        let centre_y = clamp_y(a.y + a.height / 2.0 - size.height / 2.0);
        let candidates = [
            Point { x: centre_x, y: a.y - size.height - gap },
            Point { x: centre_x, y: a.y + a.height + gap },
            Point { x: a.x - size.width - gap, y: centre_y },
            Point { x: a.x + a.width + gap, y: centre_y },
        ];
        if let Some(p) = candidates.into_iter().find(fits) {
            return p;
        }
    }
    Point { x: right - size.width - gap, y: bottom - size.height - gap }
}

impl Popup {
    #[must_use]
    pub fn new(state: PopupState) -> Self {
        Self { state, hover: None }
    }

    fn title(&self) -> (&str, Rgb) {
        match &self.state {
            PopupState::Uploading { .. } => ("Uploading…", WHITE),
            PopupState::Done { .. } => ("Uploaded. Link copied.", GREEN),
            PopupState::Failed { .. } => ("Upload failed", RED),
            PopupState::Message { title, .. } => (title, GREEN),
        }
    }

    fn inner(s: f64) -> f64 {
        (WIDTH - 2.0 * PAD) * s
    }

    fn lines(&self, font: &TextFont, s: f64) -> Vec<String> {
        let (size, width) = (BODY * s as f32, Self::inner(s));
        match &self.state {
            PopupState::Uploading { file, .. } => vec![shorten_middle(font, file, size, width)],
            PopupState::Done { link } => vec![shorten_middle(font, link, size, width)],
            PopupState::Failed { message } => wrap(font, message, size, width),
            PopupState::Message { detail, .. } => wrap(font, detail, size, width),
        }
    }

    fn actions(&self) -> &'static [PopupButton] {
        match self.state {
            PopupState::Done { .. } => &[PopupButton::Copy, PopupButton::Open],
            PopupState::Failed { .. } => &[PopupButton::Settings, PopupButton::Retry],
            _ => &[],
        }
    }

    fn line_height(font: &TextFont, size: f32) -> f64 {
        font.measure("Ag", size).height
    }

    /// The progress bar, when uploading.
    #[must_use]
    pub fn progress_bar(&self, font: &TextFont, s: f64) -> Option<Rect> {
        matches!(self.state, PopupState::Uploading { .. }).then(|| {
            let y = PAD * s + Self::line_height(font, TITLE * s as f32) + GAP * s;
            Rect::new(PAD * s, y, Self::inner(s), BAR_H * s)
        })
    }

    #[must_use]
    pub fn size(&self, font: &TextFont, s: f64) -> Size {
        let mut height = PAD * s + Self::line_height(font, TITLE * s as f32) + GAP * s;
        if self.progress_bar(font, s).is_some() {
            height += (BAR_H + GAP) * s;
        }
        height += self.lines(font, s).len() as f64 * Self::line_height(font, BODY * s as f32);
        if !self.actions().is_empty() {
            height += (GAP + BUTTON_H) * s;
        }
        Size { width: WIDTH * s, height: height + PAD * s }
    }

    /// Close first, then the action buttons from left to right (right-aligned at the bottom).
    #[must_use]
    pub fn buttons(&self, font: &TextFont, s: f64) -> Vec<(PopupButton, Rect)> {
        let size = self.size(font, s);
        let mut buttons = vec![(PopupButton::Close, Rect::new(size.width - (PAD / 2.0 + CLOSE) * s, PAD / 2.0 * s, CLOSE * s, CLOSE * s))];
        let y = size.height - (PAD + BUTTON_H) * s;
        let mut x = size.width - PAD * s;
        let mut row = Vec::new();
        for b in self.actions().iter().rev() {
            let w = font.measure(b.label(), BODY * s as f32).width + 24.0 * s;
            x -= w;
            row.push((*b, Rect::new(x, y, w, BUTTON_H * s)));
            x -= GAP * s;
        }
        row.reverse();
        buttons.extend(row);
        buttons
    }

    #[must_use]
    pub fn hit(&self, font: &TextFont, s: f64, p: Point) -> Option<PopupButton> {
        self.buttons(font, s).into_iter().find(|(_, r)| crate::session::contains(*r, p)).map(|(b, _)| b)
    }

    pub fn render(&self, out: &mut Pixmap, font: &TextFont, s: f64) {
        let size = self.size(font, s);
        out.fill(tiny_skia::Color::TRANSPARENT);
        fill_rounded(out, Rect::new(0.0, 0.0, size.width, size.height), (10.0 * s) as f32, INK, 245);
        let (title, colour) = self.title();
        let mut y = PAD * s;
        font.draw(out, title, Point { x: PAD * s, y }, TITLE * s as f32, colour);
        y += Self::line_height(font, TITLE * s as f32) + GAP * s;
        if let (Some(bar), PopupState::Uploading { progress, .. }) = (self.progress_bar(font, s), &self.state) {
            fill_rounded(out, bar, (BAR_H / 2.0 * s) as f32, WHITE, 50);
            let filled = Rect { width: bar.width * progress.clamp(0.0, 1.0), ..bar };
            fill_rounded(out, filled, (BAR_H / 2.0 * s) as f32, VIOLET, 255);
            y += (BAR_H + GAP) * s;
        }
        let line_h = Self::line_height(font, BODY * s as f32);
        for line in self.lines(font, s) {
            font.draw(out, &line, Point { x: PAD * s, y }, BODY * s as f32, GREY);
            y += line_h;
        }
        for (button, r) in self.buttons(font, s) {
            let hovered = self.hover == Some(button);
            if button == PopupButton::Close {
                if hovered {
                    fill_rounded(out, r, (r.height / 2.0) as f32, WHITE, 40);
                }
            } else {
                fill_rounded(out, r, (6.0 * s) as f32, WHITE, if hovered { 70 } else { 40 });
            }
            let label_size = BODY * s as f32;
            let m = font.measure(button.label(), label_size);
            let at = Point { x: r.x + (r.width - m.width) / 2.0, y: r.y + (r.height - m.height) / 2.0 };
            font.draw(out, button.label(), at, label_size, WHITE);
        }
    }
}
```

- [ ] **Step 3: Run the tests and look at it**

Run: `cd windows-linux && cargo test -p snapline-overlay && cargo clippy -p snapline-overlay --all-targets -- -D warnings`
Expected: pass. Then write a throwaway test (do not commit) that renders each of the four states at scale 1.5 to `/tmp/popup-<state>.png`, and look at them: title colour, wrapping, the shortened link, the buttons and the × must all be clear and inside the rounded box.

- [ ] **Step 4: Commit**

```bash
git add windows-linux
git commit -m "Add the status popup for uploads and confirmations"
```

---

### Task 8: Uploading from the app — worker, tray icon states, popup window, confirmations

**Files:**
- Modify: `windows-linux/crates/snapline/src/upload_job.rs` (add `Job`, `Outcome`, `run`), `icon.rs`, `actions.rs`, `app.rs`, `platform.rs`, `main.rs`
- Create: `windows-linux/crates/snapline/src/popup_window.rs`

**Interfaces:**
- Consumes: Tasks 1–7.
- Produces:
  - `upload_job::Job { png: Vec<u8>, file_name: String, search: Option<SearchEngine> }` (`Clone`), `upload_job::Outcome { Done { result: UploadResult, token: Option<AccessToken> }, Failed(String) }`, `upload_job::run(&Job, settings_path: &Path, &dyn SecretStore, &dyn Transport, token: Option<AccessToken>, progress: &dyn Fn(f64)) -> Outcome`.
  - `icon::TrayState { Normal, Progress(f64), Success, Failure }`, `icon::tray_rgba(size: u32, state: TrayState) -> Vec<u8>`.
  - `actions::Done { Copied { width: u32, height: u32 }, Saved(PathBuf) }`; `actions::run(...) -> Result<Option<Done>, String>` (`None`: Save As was cancelled).
  - `popup_window::PopupWindow` with `open`, `place`, `present`.
  - `platform::work_area(x: i32, y: i32) -> Option<Rect>`, `platform::open_upload_settings()`.

- [ ] **Step 1: Write the failing tests**

`upload_job.rs`, add to `mod tests`:

```rust
    use snapline_upload::http::fake::FakeTransport;

    fn job() -> Job {
        Job { png: b"PNG".to_vec(), file_name: "Shot.png".into(), search: None }
    }

    fn settings_file(name: &str, change: impl FnOnce(&mut Settings)) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("snapline-job-{}-{name}", std::process::id())).join("settings.toml");
        let _ = std::fs::remove_file(&path);
        Settings::update(&path, change).unwrap();
        path
    }

    fn drive_store() -> MemoryStore {
        let store = MemoryStore::default();
        store.put(Secret::GoogleClientSecret, "CS").unwrap();
        store.put(Secret::GoogleRefreshToken, "R").unwrap();
        store
    }

    #[test]
    fn s3_job_returns_the_link() {
        let store = MemoryStore::default();
        store.put(Secret::S3AccessKey, "AK").unwrap();
        store.put(Secret::S3SecretKey, "SK").unwrap();
        let path = settings_file("s3", |s| {
            s.s3_endpoint = "https://acct.r2.cloudflarestorage.com".into();
            s.s3_bucket = "shots".into();
            s.s3_public_url = "https://shots.example.com".into();
        });
        let Outcome::Done { result, .. } = run(&job(), &path, &store, &FakeTransport::new(vec![(200, "")]), None, &|_| {}) else {
            panic!("expected Done")
        };
        assert!(result.page.as_str().starts_with("https://shots.example.com/snapline/"));
    }

    #[test]
    fn drive_job_remembers_the_folder_it_made() {
        let path = settings_file("drive", |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
        });
        let fake = FakeTransport::new(vec![
            (200, r#"{"access_token":"A","expires_in":3600}"#),
            (200, r#"{"id":"F1"}"#),
            (200, r#"{"id":"X1","webViewLink":"https://drive.google.com/file/d/X1/view"}"#),
            (200, "{}"),
        ]);
        let outcome = run(&job(), &path, &drive_store(), &fake, None, &|_| {});
        assert!(matches!(outcome, Outcome::Done { token: Some(_), .. }));
        assert_eq!(Settings::load(&path).0.drive_folder_id, "F1");
    }

    // Review Focus 2.
    #[test]
    fn expired_sign_in_clears_the_token() {
        let path = settings_file("expired", |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
            s.google_email = "a@example.com".into();
        });
        let store = drive_store();
        let fake = FakeTransport::new(vec![(400, r#"{"error":"invalid_grant"}"#)]);
        let Outcome::Failed(message) = run(&job(), &path, &store, &fake, None, &|_| {}) else { panic!("expected Failed") };
        assert_eq!(message, snapline_upload::google::EXPIRED);
        assert_eq!(store.get(Secret::GoogleRefreshToken), None);
        assert_eq!(Settings::load(&path).0.google_email, "");
    }

    #[test]
    fn drive_without_sign_in_asks_for_it_and_sends_nothing() {
        let path = settings_file("signed-out", |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
        });
        let store = MemoryStore::default();
        store.put(Secret::GoogleClientSecret, "CS").unwrap();
        let fake = FakeTransport::new(vec![]);
        let Outcome::Failed(message) = run(&job(), &path, &store, &fake, None, &|_| {}) else { panic!("expected Failed") };
        assert_eq!(message, snapline_upload::google::NOT_SIGNED_IN);
        assert_eq!(fake.requests(), []);
    }
```

`icon.rs`: change the existing test to `tray_rgba(32, TrayState::Normal)` and add:

```rust
    fn px(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
        rgba[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4].try_into().unwrap()
    }

    #[test]
    fn progress_ring_fills_clockwise_from_the_top() {
        let rgba = super::tray_rgba(32, super::TrayState::Progress(0.5));
        let right = px(&rgba, 27, 16); // on the ring, in the first half
        let left = px(&rgba, 5, 16); // on the ring, not reached yet
        assert!(right[0] > 230 && right[1] > 230, "{right:?}");
        assert!(left[0] < 200, "{left:?}");
    }

    #[test]
    fn success_and_failure_add_a_coloured_badge() {
        let ok = px(&super::tray_rgba(32, super::TrayState::Success), 22, 22);
        assert!(ok[1] > 150 && ok[0] < 120, "green badge: {ok:?}");
        let bad = px(&super::tray_rgba(32, super::TrayState::Failure), 22, 22);
        assert!(bad[0] > 200 && bad[1] < 120, "red badge: {bad:?}");
    }
```

`actions.rs`, add to `mod tests`:

```rust
    #[test]
    fn saving_reports_the_file() {
        let dir = std::env::temp_dir().join(format!("snapline-report-{}", std::process::id()));
        let settings = Settings { save_directory: dir.clone(), ..Settings::default() };
        let image = Pixmap::new(4, 3).unwrap();
        let Some(Done::Saved(path)) = run(Command::Save, &image, &settings).unwrap() else { panic!("expected Saved") };
        assert!(path.exists() && path.starts_with(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }
```

Run `cargo test -p snapline` → fails.

- [ ] **Step 2: The upload job**

Add to `upload_job.rs` (imports: `chrono::Local`, `snapline_core::search::SearchEngine`, `snapline_upload::{UploadResult, drive, s3, google::{self, AccessToken, RefreshError}, http::Transport}`, `std::path::Path`):

```rust
#[derive(Clone)]
pub struct Job {
    pub png: Vec<u8>,
    pub file_name: String,
    /// Open this image search once the upload is done.
    pub search: Option<SearchEngine>,
}

pub enum Outcome {
    /// `token` is the Google access token to reuse for the next upload.
    Done { result: UploadResult, token: Option<AccessToken> },
    Failed(String),
}

/// One upload from start to finish. Reads the settings file again (so changes apply at once), uploads,
/// and keeps what Drive hands back: the folder, or a dead sign-in to forget.
pub fn run(
    job: &Job,
    settings_path: &Path,
    secrets: &dyn SecretStore,
    t: &dyn Transport,
    token: Option<AccessToken>,
    progress: &dyn Fn(f64),
) -> Outcome {
    let (settings, problem) = Settings::load(settings_path);
    if let Some(problem) = problem {
        return Outcome::Failed(format!("Snapline could not read its settings:\n{problem}"));
    }
    match destination(&settings, secrets) {
        Destination::S3(config) => match s3::upload(t, &config, &job.png, Local::now(), progress) {
            Ok(result) => Outcome::Done { result, token },
            Err(e) => Outcome::Failed(e.to_string()),
        },
        Destination::Drive { client, refresh_token, folder_id } => {
            if !client.is_complete() {
                return Outcome::Failed("Fill in the Google client ID and secret in Snapline's upload settings.".into());
            }
            let Some(refresh) = refresh_token else { return Outcome::Failed(google::NOT_SIGNED_IN.into()) };
            let access = match token.filter(AccessToken::is_fresh) {
                Some(access) => access,
                None => match google::refresh(t, &client, &refresh) {
                    Ok(access) => access,
                    Err(RefreshError::Expired) => {
                        let _ = secrets.remove(Secret::GoogleRefreshToken);
                        let _ = Settings::update(settings_path, |s| {
                            s.google_email.clear();
                            s.drive_folder_id.clear();
                        });
                        return Outcome::Failed(google::EXPIRED.into());
                    }
                    Err(RefreshError::Failed(e)) => return Outcome::Failed(e.to_string()),
                },
            };
            match drive::upload(t, &access.token, folder_id.as_deref(), &job.png, &job.file_name, progress) {
                Ok(done) => {
                    if folder_id.as_deref() != Some(done.folder_id.as_str()) {
                        let _ = Settings::update(settings_path, |s| s.drive_folder_id.clone_from(&done.folder_id));
                    }
                    Outcome::Done { result: done.result, token: Some(access) }
                }
                Err(e) => Outcome::Failed(e.to_string()),
            }
        }
    }
}
```

- [ ] **Step 3: Tray icon states**

In `icon.rs`, move the current drawing into `fn draw_base(pixmap: &mut Pixmap, grid: Transform)`, then:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrayState {
    Normal,
    /// 0…1, drawn as a ring that fills clockwise from the top.
    Progress(f64),
    /// ✓ on a green badge.
    Success,
    /// ! on a red badge.
    Failure,
}

/// Straight (not premultiplied) RGBA, `size`×`size`.
///
/// # Panics
/// If `size` is 0.
#[must_use]
pub fn tray_rgba(size: u32, state: TrayState) -> Vec<u8> {
    let mut pixmap = Pixmap::new(size, size).expect("icon size is not zero");
    let unit = size as f32 / 32.0;
    let grid = Transform::from_scale(unit, unit);
    draw_base(&mut pixmap, grid);
    let paint = |r: u8, g: u8, b: u8, a: u8| {
        let mut p = Paint { anti_alias: true, ..Paint::default() };
        p.set_color_rgba8(r, g, b, a);
        p
    };
    let stroke = |width: f32| Stroke { width, line_cap: LineCap::Round, ..Stroke::default() };
    match state {
        TrayState::Normal => {}
        TrayState::Progress(done) => {
            // Dim the glyph, then a track and the filled part of the ring.
            if let Some(r) = tiny_skia::Rect::from_xywh(1.0, 1.0, 30.0, 30.0) {
                pixmap.fill_rect(r, &paint(0x0C, 0x08, 0x19, 140), grid, None);
            }
            if let Some(ring) = PathBuilder::from_circle(16.0, 16.0, 11.0) {
                pixmap.stroke_path(&ring, &paint(255, 255, 255, 70), &stroke(3.0), grid, None);
            }
            let mut arc = PathBuilder::new();
            let steps = 48;
            let end = (done.clamp(0.0, 1.0) * f64::from(steps)).round() as u32;
            for k in 0..=end {
                let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * f64::from(k) / f64::from(steps);
                let (x, y) = ((16.0 + 11.0 * angle.cos()) as f32, (16.0 + 11.0 * angle.sin()) as f32);
                if k == 0 { arc.move_to(x, y) } else { arc.line_to(x, y) }
            }
            if let Some(path) = arc.finish() {
                pixmap.stroke_path(&path, &paint(255, 255, 255, 255), &stroke(3.0), grid, None);
            }
        }
        TrayState::Success | TrayState::Failure => {
            let (r, g, b) = if state == TrayState::Success { (0x34, 0xC7, 0x59) } else { (0xFF, 0x3B, 0x30) };
            if let Some(badge) = PathBuilder::from_circle(24.0, 24.0, 7.5) {
                pixmap.fill_path(&badge, &paint(r, g, b, 255), FillRule::Winding, grid, None);
            }
            let mut mark = PathBuilder::new();
            if state == TrayState::Success {
                mark.move_to(20.5, 24.0);
                mark.line_to(23.0, 26.5);
                mark.line_to(27.5, 21.5);
            } else {
                mark.move_to(24.0, 20.5);
                mark.line_to(24.0, 24.5);
                mark.move_to(24.0, 27.3);
                mark.line_to(24.0, 27.4);
            }
            if let Some(path) = mark.finish() {
                pixmap.stroke_path(&path, &paint(255, 255, 255, 255), &stroke(2.0), grid, None);
            }
        }
    }
    pixmap
        .pixels()
        .iter()
        .flat_map(|c| {
            let c = c.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}
```

Update the callers in `app.rs` to `icon::tray_rgba(32, TrayState::Normal)`.

- [ ] **Step 4: Copy and save report what they did**

In `actions.rs`:

```rust
/// What a finished action did, for the confirmation popup.
#[derive(Debug, PartialEq, Eq)]
pub enum Done {
    Copied { width: u32, height: u32 },
    Saved(PathBuf),
}
```

Change `run` to return `Result<Option<Done>, String>`: Copy → `Some(Done::Copied { width: image.width(), height: image.height() })`; Save → `Some(Done::Saved(path))`; Save As → `Some(Done::Saved(path))`, or `None` when cancelled; `Cancel`, `Upload` and `Search(_)` → `None` (the app handles uploads itself).

- [ ] **Step 5: Platform: the work area and the settings window**

Add to `platform.rs` (add the `Win32_Graphics_Gdi` feature: `cargo add -p snapline windows-sys --target 'cfg(windows)' --features Win32_Graphics_Gdi`):

```rust
/// The usable part (without the taskbar) of the monitor under `(x, y)`, in physical pixels.
#[cfg(windows)]
pub fn work_area(x: i32, y: i32) -> Option<snapline_overlay::Rect> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint};
    // SAFETY: plain calls; `info` is a valid MONITORINFO with its size set.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = u32::try_from(std::mem::size_of::<MONITORINFO>()).ok()?;
        if GetMonitorInfoW(monitor, &raw mut info) == 0 {
            return None;
        }
        let r = info.rcWork;
        Some(snapline_overlay::Rect::new(f64::from(r.left), f64::from(r.top), f64::from(r.right - r.left), f64::from(r.bottom - r.top)))
    }
}

#[cfg(not(windows))]
pub fn work_area(_x: i32, _y: i32) -> Option<snapline_overlay::Rect> {
    None // the caller falls back to the monitor's full area
}

/// Opens the upload settings window: this program again, as a second process.
pub fn open_upload_settings() {
    match std::env::current_exe() {
        Ok(exe) => {
            if let Err(e) = std::process::Command::new(exe).arg("--upload-settings").spawn() {
                show_error(&format!("Snapline could not open its upload settings: {e}"));
            }
        }
        Err(e) => show_error(&format!("Snapline could not open its upload settings: {e}")),
    }
}
```

- [ ] **Step 6: The popup window**

`src/popup_window.rs`:

```rust
//! The status popup's window: borderless, always on top, never taking the focus, drawn through softbuffer.

use snapline_overlay::popup::Popup;
use snapline_overlay::text::TextFont;
use snapline_overlay::tiny_skia::Pixmap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowLevel};

pub struct PopupWindow {
    pub window: Rc<Window>,
    _context: softbuffer::Context<Rc<Window>>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    frame: Pixmap,
    /// Pixels per point on the monitor the popup is on.
    pub scale: f64,
}

impl PopupWindow {
    pub fn open(event_loop: &ActiveEventLoop, at: PhysicalPosition<i32>, size: PhysicalSize<u32>, scale: f64) -> Result<Self, String> {
        let attributes = Window::default_attributes()
            .with_title("Snapline")
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_active(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_position(at)
            .with_inner_size(size)
            .with_visible(false);
        #[cfg(windows)]
        let attributes = {
            use winit::platform::windows::WindowAttributesExtWindows;
            attributes.with_skip_taskbar(true)
        };
        let window = Rc::new(event_loop.create_window(attributes).map_err(|e| e.to_string())?);
        let context = softbuffer::Context::new(window.clone()).map_err(|e| e.to_string())?;
        let surface = softbuffer::Surface::new(&context, window.clone()).map_err(|e| e.to_string())?;
        let frame = Pixmap::new(size.width.max(1), size.height.max(1)).ok_or("empty popup")?;
        let mut popup = Self { window, _context: context, surface, frame, scale };
        popup.place(at, size)?;
        Ok(popup)
    }

    /// Moves and resizes (the height changes with the content).
    pub fn place(&mut self, at: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> Result<(), String> {
        let (w, h) = (size.width.max(1), size.height.max(1));
        if (w, h) != (self.frame.width(), self.frame.height()) {
            self.frame = Pixmap::new(w, h).ok_or("empty popup")?;
        }
        let (nw, nh) = NonZeroU32::new(w).zip(NonZeroU32::new(h)).ok_or("empty popup")?;
        self.surface.resize(nw, nh).map_err(|e| e.to_string())?;
        let _ = self.window.request_inner_size(PhysicalSize::new(w, h));
        self.window.set_outer_position(at);
        Ok(())
    }

    pub fn present(&mut self, popup: &Popup, font: &TextFont) -> Result<(), String> {
        popup.render(&mut self.frame, font, self.scale);
        let mut buffer = self.surface.buffer_mut().map_err(|e| e.to_string())?;
        for (dst, src) in buffer.iter_mut().zip(self.frame.pixels()) {
            // softbuffer has no alpha; the rounded corners show the ink colour, as on a solid popup.
            let c = src.demultiply();
            *dst = (u32::from(c.red()) << 16) | (u32::from(c.green()) << 8) | u32::from(c.blue());
        }
        buffer.present().map_err(|e| e.to_string())?;
        self.window.set_visible(true);
        Ok(())
    }
}
```

softbuffer has no alpha channel, so the corners are drawn in the background colour: `Popup::render` fills transparent first, which `demultiply` turns into black. Ruling allowed: if the black corners look wrong on Windows, fill the corner pixels with the ink colour (a square popup with a 10-point radius drawn inside) and ledger it.

- [ ] **Step 7: Wire it into the app**

In `app.rs`:

1. `UserEvent` gains `UploadProgress(f64)` and `UploadFinished(Outcome)`.
2. `App` gains:

```rust
    secrets: Option<Keyring>,
    settings_item: MenuId,                 // "Upload Settings…"
    popup: Option<(Popup, PopupWindow)>,
    popup_close_at: Option<Instant>,
    icon_reset_at: Option<Instant>,
    last_job: Option<Job>,
    uploading: bool,
    google_token: Option<AccessToken>,
    popup_pointer: Point,
```

3. `start()`: `self.secrets = Keyring::open().map_err(|e| show_error(&format!("Snapline cannot reach Credential Manager: {e}"))).ok();`. In `build_tray`, add `MenuItem::with_id(self.settings_item.clone(), "Upload Settings…", true, None)` before the separator.
4. `user_event`:
   - `Menu(id) if id == self.settings_item` → `platform::open_upload_settings()`.
   - `UploadProgress(p)` → if the popup is `Uploading`, set `progress = p` and redraw it; `self.set_tray(TrayState::Progress(p))`.
   - `UploadFinished(outcome)` → `self.uploading = false;` then:
     - `Done { result, token }`: `self.google_token = token`; copy `result.page` as text (`arboard::Clipboard::new()?.set_text(…)`); if `last_job.search` is `Some(engine)`, `open::that(engine.url(&result.direct).as_str())`; show `PopupState::Done { link }`; tray `Success`; `icon_reset_at` and `popup_close_at` = now + 4 s.
     - `Failed(message)`: show `PopupState::Failed { message }`; tray `Failure`; `icon_reset_at` = now + 6 s; no auto-close.
5. In `apply()`, before running actions: for `Command::Upload` and `Command::Search(engine)`, encode the exported image (`image.encode_png()`) and call `self.start_upload(Job { png, file_name: naming::file_name(Local::now()), search: … }, event_loop)`. Pass `event_loop` into `apply` (it needs it to open the popup). For Copy, Save and Save As, show the confirmation from `actions::run`'s `Done`:
   - `Copied { width, height }` → `Message { title: "Screenshot copied", detail: "{width} × {height} pixels" }`
   - `Saved(path)` → `Message { title: "Screenshot saved", detail: <file name> }`
   - Either closes after 2.5 s.
6. `start_upload(job, event_loop)`:

```rust
    fn start_upload(&mut self, job: Job, event_loop: &ActiveEventLoop) {
        if self.uploading {
            return self.show_popup(event_loop, PopupState::Message {
                title: "Upload in progress".into(),
                detail: "Wait for the current upload to finish, then try again.".into(),
            }, Some(2.5));
        }
        let (Some(secrets), Some(path)) = (self.secrets, Settings::path()) else {
            return self.show_popup(event_loop, PopupState::Failed { message: "Snapline cannot reach its settings or Credential Manager.".into() }, None);
        };
        self.uploading = true;
        self.last_job = Some(job.clone());
        self.show_popup(event_loop, PopupState::Uploading { progress: 0.0, file: job.file_name.clone() }, None);
        self.set_tray(TrayState::Progress(0.0));
        let proxy = self.proxy.clone();
        let token = self.google_token.clone();
        std::thread::spawn(move || {
            let last = std::sync::atomic::AtomicU32::new(0);
            let report = |p: f64| {
                let percent = (p * 100.0) as u32;
                if percent != last.swap(percent, std::sync::atomic::Ordering::Relaxed) {
                    let _ = proxy.send_event(UserEvent::UploadProgress(p));
                }
            };
            let outcome = upload_job::run(&job, &path, &secrets, &UreqTransport::new(), token, &report);
            let _ = proxy.send_event(UserEvent::UploadFinished(outcome));
        });
    }
```

7. `show_popup(event_loop, state, close_after: Option<f64>)`:
   - Anchor: the tray icon's rectangle (`self.tray.as_ref().and_then(TrayIcon::rect)`), as an overlay `Rect` in physical pixels.
   - Scale: from the winit monitor that contains the anchor (or the primary monitor, or 1.0).
   - Work area: `platform::work_area` at the anchor's centre, or that monitor's full rectangle.
   - Size: `Popup::size(&self.font, scale)`.
   - Place it at `popup::origin(anchor, size, work, 8.0 * scale)`: create the `PopupWindow` or `place` the existing one, then `present`.
   - Set `popup_close_at` from `close_after`.
8. `window_event` for the popup's window id:
   - `CursorMoved` → store the point and update `hover`, redrawing when it changes.
   - `CursorLeft` → clear `hover`, and if `popup_close_at` has passed, close 1.5 s later.
   - Left `MouseInput` pressed → act on `popup.hit(…)`:
     - `Copy` → copy the link again
     - `Open` → `open::that(link)`, then close
     - `Settings` → `platform::open_upload_settings()`, then close
     - `Retry` → `start_upload(last_job.clone())`
     - `Close` → close
   - `RedrawRequested` → `present`.
9. `about_to_wait(event_loop)`:
   - Close the popup when `popup_close_at` has passed and the pointer is not over it.
   - Reset the tray icon to `Normal` when `icon_reset_at` has passed.
   - Then `event_loop.set_control_flow(…)`: `ControlFlow::WaitUntil(the next of the two)`, or `ControlFlow::Wait` when neither is set.
10. `set_tray(state)`: `tray.set_icon(Some(Icon::from_rgba(icon::tray_rgba(32, state), 32, 32)))`.

Add `cargo add -p snapline open` and `mod popup_window;` to `main.rs`. Remove the `#[allow(dead_code)]` on `upload_job`.

- [ ] **Step 8: Build, lint and try it on this Mac**

Run: `cd windows-linux && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo clippy -p snapline --all-targets --target x86_64-pc-windows-msvc -- -D warnings`
Expected: all pass. Then `cargo run -p snapline` and check:
- Capture, select, press Ctrl+D with no upload settings: the popup appears near the menu bar icon with "Upload failed", the S3 settings message, and both buttons. The tray icon shows the red badge, then returns to normal.
- Ctrl+C shows "Screenshot copied", which fades.

Writing real uploads needs Barayuda's bucket or Google client; that is in Task 10's checklist.

- [ ] **Step 9: Commit**

```bash
git add windows-linux
git commit -m "Upload from the overlay with progress in the tray and a status popup"
```

---

### Task 9: The upload settings window

**Files:**
- Create: `windows-linux/crates/snapline/src/upload_settings.rs`
- Modify: `main.rs` (handle `--upload-settings` before anything else), `platform.rs` (`claim(name)`), `Cargo.toml`

**Interfaces:**
- Consumes: `Settings`, `UploadTarget`, `secrets::{save, Keyring, Secret, SecretStore}`, `snapline_upload::google::{sign_in, sign_out, GoogleClient}`, `UreqTransport`, `icon::tray_rgba`.
- Produces: `upload_settings::UploadForm` (fields below) with `load(&Settings, &dyn SecretStore)`, `save(&mut self, &Path, &dyn SecretStore)`, `status: Option<(String, bool)>`; `upload_settings::run_window()`.

The window mirrors the macOS Upload tab:
- A choice between "S3-compatible bucket" and "Google Drive".
- **S3:**
  - Bucket fields: Endpoint, Region, Bucket, Key prefix, Public URL.
  - Keys, stored in Credential Manager: Access key ID, Secret access key.
  - **Save**, with the result beside it.
  - A note: "Uploads are public to anyone with the link. Do not upload guest or customer data."
- **Drive:**
  - Client ID, Client secret, **Save**, and links to the Google Cloud Console and the README guide.
  - Account: "Signed in as …" with **Sign out**, or **Sign in with Google…** (it saves first).
  - A note about the "Snapline" folder and link sharing.
- Saving reports "Saved", "Saved. Still empty: …", or why it could not save. Same messages as macOS, with "Credential Manager" for "Keychain".

- [ ] **Step 1: Add eframe**

Run: `cd windows-linux && cargo add -p snapline eframe --no-default-features --features glow,default_fonts,accesskit,wayland,x11`. Check `cargo tree -p snapline -i winit` shows one winit version. If eframe needs another, record a ruling; two winit versions only cost binary size.

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("snapline-form-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("settings.toml")
    }

    fn s3_form() -> UploadForm {
        UploadForm {
            target: UploadTarget::S3,
            endpoint: "https://acct.r2.cloudflarestorage.com".into(),
            bucket: "shots".into(),
            public_url: "https://shots.example.com".into(),
            access_key: "AKIDEXAMPLE".into(),
            secret_key: ["wJalr", "XUtnFEMI", "K7MDENG"].concat(),
            ..UploadForm::default()
        }
    }

    // Review Focus 4.
    #[test]
    fn secrets_never_reach_the_settings_file() {
        let (path, store) = (temp_path("secrets"), MemoryStore::default());
        let mut form = s3_form();
        form.client_secret = "client-secret-value".into();
        form.save(&path, &store);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("shots") && !text.contains("AKIDEXAMPLE") && !text.contains(&form.secret_key), "{text}");
        assert!(!text.contains("client-secret-value"));
        assert_eq!(store.get(Secret::S3SecretKey).as_deref(), Some(form.secret_key.as_str()));
        assert_eq!(form.status, Some(("Saved".into(), true)));
    }

    #[test]
    fn saving_lists_what_is_still_empty() {
        let (path, store) = (temp_path("empty"), MemoryStore::default());
        let mut form = UploadForm { secret_key: String::new(), ..s3_form() };
        form.save(&path, &store);
        assert_eq!(form.status, Some(("Saved. Still empty: secret access key.".into(), false)));
        let mut drive = UploadForm { target: UploadTarget::Drive, client_id: "ID".into(), ..UploadForm::default() };
        drive.save(&path, &store);
        assert_eq!(drive.status, Some(("Saved. Still empty: Client secret.".into(), false)));
    }

    #[test]
    fn a_broken_settings_file_is_reported_and_left_alone() {
        let (path, store) = (temp_path("broken"), MemoryStore::default());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "s3_bucket = \"x\n").unwrap();
        let mut form = s3_form();
        form.save(&path, &store);
        let (text, ok) = form.status.clone().unwrap();
        assert!(!ok && text.contains("could not be read"), "{text}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "s3_bucket = \"x\n");
    }

    #[test]
    fn the_form_shows_what_is_saved() {
        let (path, store) = (temp_path("load"), MemoryStore::default());
        s3_form().save(&path, &store);
        let form = UploadForm::load(&Settings::load(&path).0, &store);
        assert_eq!((form.bucket.as_str(), form.access_key.as_str()), ("shots", "AKIDEXAMPLE"));
        assert!(!form.signed_in);
    }
}
```

Run `cargo test -p snapline upload_settings` → fails.

- [ ] **Step 3: The form model**

```rust
//! The upload settings window (the Upload tab of Preferences, brought forward to Phase 3). It runs as a second
//! process, `snapline --upload-settings`, and shares `settings.toml` and Credential Manager with the tray app.

use crate::secrets::{self, Keyring, Secret, SecretStore};
use crate::settings::{Settings, UploadTarget};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UploadForm {
    pub target: UploadTarget,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub public_url: String,
    pub access_key: String,
    pub secret_key: String,
    pub client_id: String,
    pub client_secret: String,
    pub email: String,
    pub signed_in: bool,
    /// The result of the last Save: text, and whether everything is in order.
    pub status: Option<(String, bool)>,
}

impl UploadForm {
    #[must_use]
    pub fn load(settings: &Settings, secrets: &dyn SecretStore) -> Self {
        Self {
            target: settings.upload_target,
            endpoint: settings.s3_endpoint.clone(),
            region: settings.s3_region.clone(),
            bucket: settings.s3_bucket.clone(),
            prefix: settings.s3_prefix.clone(),
            public_url: settings.s3_public_url.clone(),
            access_key: secrets.get(Secret::S3AccessKey).unwrap_or_default(),
            secret_key: secrets.get(Secret::S3SecretKey).unwrap_or_default(),
            client_id: settings.google_client_id.clone(),
            client_secret: secrets.get(Secret::GoogleClientSecret).unwrap_or_default(),
            email: settings.google_email.clone(),
            signed_in: secrets.get(Secret::GoogleRefreshToken).is_some(),
            status: None,
        }
    }

    /// Saves the plain fields to `settings.toml` and the secrets to the store, and sets `status`.
    pub fn save(&mut self, path: &Path, store: &dyn SecretStore) {
        let form = self.clone();
        let written = Settings::update(path, |s| {
            s.upload_target = form.target;
            s.s3_endpoint = form.endpoint.trim().to_owned();
            s.s3_region = form.region.trim().to_owned();
            s.s3_bucket = form.bucket.trim().to_owned();
            s.s3_prefix = form.prefix.trim().to_owned();
            s.s3_public_url = form.public_url.trim().to_owned();
            s.google_client_id = form.client_id.trim().to_owned();
        });
        if let Err(e) = written {
            self.status = Some((e, false));
            return;
        }
        let (stored, fields) = match self.target {
            UploadTarget::S3 => (
                secrets::save(store, Secret::S3AccessKey, &self.access_key)
                    .and_then(|()| secrets::save(store, Secret::S3SecretKey, &self.secret_key)),
                vec![("access key ID", self.access_key.clone()), ("secret access key", self.secret_key.clone())],
            ),
            UploadTarget::Drive => (
                secrets::save(store, Secret::GoogleClientSecret, &self.client_secret),
                vec![("Client ID", self.client_id.clone()), ("Client secret", self.client_secret.clone())],
            ),
        };
        self.status = Some(match stored {
            Err(e) => (format!("Could not save to Credential Manager: {e}"), false),
            Ok(()) => {
                let empty: Vec<&str> = fields.iter().filter(|(_, v)| v.trim().is_empty()).map(|(n, _)| *n).collect();
                if empty.is_empty() { ("Saved".into(), true) } else { (format!("Saved. Still empty: {}.", empty.join(", ")), false) }
            }
        });
    }
}
```

Run the tests → pass.

- [ ] **Step 4: The window**

Add to `upload_settings.rs`:

```rust
use snapline_upload::google::{self, GoogleClient};
use snapline_upload::http::UreqTransport;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const CONSOLE: &str = "https://console.cloud.google.com/auth/clients";
const GUIDE: &str = "https://github.com/barayuda/Snapline#google-drive-one-time-setup-about-10-minutes";

struct SettingsApp {
    form: UploadForm,
    path: std::path::PathBuf,
    store: Keyring,
    /// A Google sign-in or sign-out running on another thread; its result is the new email, or an error.
    pending: Option<Receiver<Result<Option<String>, String>>>,
    google_error: Option<String>,
}

/// Runs the window until it is closed. One window at a time.
pub fn run_window() {
    if !crate::platform::claim("Local\\dev.barayuda.snapline.upload-settings") {
        return; // ponytail: the open window is not brought to the front
    }
    let (Some(path), Ok(store)) = (Settings::path(), Keyring::open()) else {
        return crate::platform::show_error("Snapline cannot reach its settings or Credential Manager.");
    };
    let (settings, problem) = Settings::load(&path);
    let mut form = UploadForm::load(&settings, &store);
    form.status = problem.map(|p| (format!("This file could not be read, so Save is off until it is fixed:\n{p}"), false));
    let icon = eframe::egui::IconData { rgba: crate::icon::tray_rgba(64, crate::icon::TrayState::Normal), width: 64, height: 64 };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Snapline upload settings")
            .with_inner_size([480.0, 620.0])
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };
    let app = SettingsApp { form, path, store, pending: None, google_error: None };
    if let Err(e) = eframe::run_native("Snapline upload settings", options, Box::new(|_| Ok(Box::new(app)))) {
        crate::platform::show_error(&format!("Snapline could not open its upload settings: {e}"));
    }
}

impl SettingsApp {
    fn sign_in(&mut self) {
        self.form.save(&self.path, &self.store);
        let client = GoogleClient { id: self.form.client_id.clone(), secret: self.form.client_secret.clone() };
        if !client.is_complete() {
            self.google_error = Some("Paste the Client ID and Client secret above, then click Save.".into());
            return;
        }
        let (tx, rx) = channel();
        let (path, store) = (self.path.clone(), self.store);
        std::thread::spawn(move || {
            let open = |url: &url::Url| open::that(url.as_str()).is_ok();
            let result = google::sign_in(&UreqTransport::new(), &client, &open, Duration::from_secs(300))
                .map_err(|e| e.to_string())
                .and_then(|signed_in| {
                    secrets::save(&store, Secret::GoogleRefreshToken, &signed_in.refresh_token)?;
                    Settings::update(&path, |s| {
                        s.google_email = signed_in.email.clone().unwrap_or_default();
                        s.drive_folder_id.clear();
                    })?;
                    Ok(signed_in.email)
                });
            let _ = tx.send(result);
        });
        self.pending = Some(rx);
        self.google_error = None;
    }

    fn sign_out(&mut self) {
        let (tx, rx) = channel();
        let (path, store) = (self.path.clone(), self.store);
        std::thread::spawn(move || {
            if let Some(token) = store.get(Secret::GoogleRefreshToken) {
                google::sign_out(&UreqTransport::new(), &token);
            }
            let result = store.remove(Secret::GoogleRefreshToken).and_then(|()| {
                Settings::update(&path, |s| {
                    s.google_email.clear();
                    s.drive_folder_id.clear();
                })
                .map(|_| None)
            });
            let _ = tx.send(result);
        });
        self.pending = Some(rx);
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        use eframe::egui;
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok(email) => {
                            self.form.signed_in = email.is_some() || self.store.get(Secret::GoogleRefreshToken).is_some();
                            self.form.email = email.unwrap_or_default();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                        Err(e) => self.google_error = Some(e),
                    }
                }
                Err(_) => ctx.request_repaint_after(Duration::from_millis(200)),
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Upload settings");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.form.target, UploadTarget::S3, "S3-compatible bucket");
                ui.selectable_value(&mut self.form.target, UploadTarget::Drive, "Google Drive");
            });
            ui.separator();
            let field = |ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str, secret: bool| {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(value).hint_text(hint).password(secret).desired_width(f32::INFINITY));
                ui.end_row();
            };
            match self.form.target {
                UploadTarget::S3 => {
                    egui::Grid::new("s3").num_columns(2).show(ui, |ui| {
                        field(ui, "Endpoint", &mut self.form.endpoint, "https://<account>.r2.cloudflarestorage.com", false);
                        field(ui, "Region", &mut self.form.region, "auto, or e.g. ap-southeast-1", false);
                        field(ui, "Bucket", &mut self.form.bucket, "", false);
                        field(ui, "Key prefix", &mut self.form.prefix, "snapline", false);
                        field(ui, "Public URL", &mut self.form.public_url, "https://shots.example.com", false);
                    });
                    ui.label(egui::RichText::new("Keys (stored in Credential Manager)").strong());
                    egui::Grid::new("keys").num_columns(2).show(ui, |ui| {
                        field(ui, "Access key ID", &mut self.form.access_key, "", false);
                        field(ui, "Secret access key", &mut self.form.secret_key, "", true);
                    });
                }
                UploadTarget::Drive => {
                    ui.label(egui::RichText::new("Google Cloud OAuth client (type: Desktop app)").strong());
                    egui::Grid::new("google").num_columns(2).show(ui, |ui| {
                        field(ui, "Client ID", &mut self.form.client_id, "Paste: …apps.googleusercontent.com", false);
                        field(ui, "Client secret", &mut self.form.client_secret, "Paste: GOCSPX-…", true);
                    });
                    ui.horizontal(|ui| {
                        ui.hyperlink_to("Open Google Cloud Console", CONSOLE);
                        ui.hyperlink_to("Step-by-step guide", GUIDE);
                    });
                }
            }
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.form.save(&self.path, &self.store);
                }
                if let Some((text, ok)) = &self.form.status {
                    let colour = if *ok { egui::Color32::from_rgb(0x34, 0xC7, 0x59) } else { egui::Color32::from_rgb(0xFF, 0x95, 0x00) };
                    ui.colored_label(colour, text);
                }
            });
            ui.separator();
            match self.form.target {
                UploadTarget::S3 => {
                    ui.small("Uploads are public to anyone with the link. Do not upload guest or customer data.");
                }
                UploadTarget::Drive => {
                    ui.label(egui::RichText::new("Account").strong());
                    let busy = self.pending.is_some();
                    if self.form.signed_in {
                        ui.label(format!("Signed in as {}", if self.form.email.is_empty() { "Google account" } else { &self.form.email }));
                        if ui.add_enabled(!busy, egui::Button::new("Sign out")).clicked() {
                            self.sign_out();
                        }
                    } else if ui
                        .add_enabled(!busy && !self.form.client_id.trim().is_empty(), egui::Button::new("Sign in with Google…"))
                        .clicked()
                    {
                        self.sign_in();
                    }
                    if busy {
                        ui.spinner();
                    }
                    if let Some(e) = &self.google_error {
                        ui.colored_label(egui::Color32::from_rgb(0xFF, 0x3B, 0x30), e);
                    }
                    ui.small(
                        "Screenshots go to a \"Snapline\" folder in My Drive and are shared as \"anyone with the link can view\". \
                         Snapline can only see files it created.",
                    );
                }
            }
        });
    }
}
```

`platform.rs`: rename `claim_single_instance()` to `claim(name: &str) -> bool`, taking the mutex name. Keep `claim_single_instance()` as `claim("Local\\dev.barayuda.snapline")`.

`main.rs`, first thing in `main()`:

```rust
    if std::env::args().any(|a| a == "--upload-settings") {
        return upload_settings::run_window();
    }
```

Also add `mod upload_settings;`, then `cargo add -p snapline url`.

- [ ] **Step 5: Build, lint, and try it on this Mac**

Run: `cd windows-linux && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo clippy -p snapline --all-targets --target x86_64-pc-windows-msvc -- -D warnings`
Expected: pass. Then `cargo run -p snapline -- --upload-settings` and check:
- The window opens with both choices and every field.
- Save shows the status.
- The macOS Keychain gets `dev.barayuda.snapline` items.
- `~/Library/Application Support/Snapline/settings.toml` holds no secrets.

Delete those test Keychain items afterwards (Keychain Access, search `dev.barayuda.snapline`).

- [ ] **Step 6: Commit**

```bash
git add windows-linux
git commit -m "Add the upload settings window with Google sign-in"
```

---

### Task 10: Docs, and the Windows check

**Files:**
- Modify: `docs/windows-linux.md`, `README.md`

- [ ] **Step 1: Design doc**

In `docs/windows-linux.md`:
- **Decisions:** add "Upload settings window in Phase 3 (Upload tab first, as a second process) — Barayuda, 30 Oct 2026". Add "Search similar and confirmations in Phase 3 — Barayuda, 30 Oct 2026".
- **Platform table:**
  - URLSession → "`ureq` (rustls with ring), on a worker thread", replacing `reqwest`.
  - Keychain → "`keyring-core` with `windows-native-keyring-store` (Credential Manager)".
  - Preferences → "an `egui` window, Upload tab in Phase 3".
  - Menu bar popovers → "a borderless popup next to the tray, drawn with tiny-skia".
- **Repo layout:** add `crates/snapline-upload/`.
- **Phase 3 line:** link this plan.
- **Risks:** add "Networks that inspect TLS with their own root certificate: ureq trusts the bundled web roots only. Add the Windows certificate store if users hit it."

- [ ] **Step 2: README**

In the "Windows (preview)" section:
- Change "uploads … come later" to "auto-hide and Copy Text come later".
- Add:

```markdown
- Upload: Ctrl+D or the Upload button. Set the bucket or Google Drive in the tray menu → **Upload Settings…** (the same
  fields as on macOS; keys are kept in Windows Credential Manager). The link is copied when the upload finishes.
- Search similar images: the magnifier button, then Google Lens or TinEye.
```

Also add `crates/snapline-upload/` to the project layout.

- [ ] **Step 3: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo clippy -p snapline --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cd ../macOS && swift test
cd .. && git add docs README.md && git commit -m "Document uploads on Windows"
git log --format='%h %G?' origin/feat/windows-linux-rust..HEAD   # every line ends in G
git push
```

Expected: CI passes on Windows and Linux. The Windows job uploads `snapline-windows`.

- [ ] **Step 4: Barayuda checks it on Windows**

Pass or fail for each line, with a screenshot for any fail:

1. Tray menu → **Upload Settings…** opens the window; choosing it again does not open a second one.
2. S3 (R2, AWS or MinIO):
   - Fill in the fields and press Save: it says "Saved".
   - `%APPDATA%\Snapline\settings.toml` (open it in Notepad) holds no keys.
   - Control Panel → Credential Manager → Windows Credentials lists the Snapline entries.
3. Capture, then Ctrl+D:
   - A popup next to the clock shows progress, then "Uploaded. Link copied."
   - The pasted link opens the image.
   - The tray icon shows the ring, then ✓, then returns to normal.
4. Change the secret key to a wrong one and upload:
   - "S3 upload failed (HTTP 403) …" appears, with a red badge on the tray icon.
   - **Upload settings…** and **Try again** both work.
5. Google Drive:
   - Paste the client ID and secret, then **Sign in with Google…**: the browser page says you are signed in, and the window shows your email.
   - Upload: the link opens the Drive page, and it is viewable in a private browser window.
6. Remove Snapline at myaccount.google.com/permissions:
   - The next upload says the sign-in expired.
   - Signing in again works.
7. Search similar → Google Lens, and → TinEye: each opens with the image.
8. Ctrl+C and Ctrl+S show "Screenshot copied" and "Screenshot saved", which fade.
9. Move the taskbar to the top, then the left, then the right (Settings → Personalization → Taskbar → Taskbar behaviors): the popup stays fully on screen, next to the tray icon. Repeat on a second monitor.
10. Turn Wi-Fi off and upload: "Snapline could not reach …". Turn it on again: **Try again** works.
11. While Snapline is running, switch the settings window from S3 to Drive and Save: the next upload goes to Drive without restarting.

- [ ] **Step 5: Fix what the Windows check finds**

For each fail:
- If the bug is in `snapline-upload` or `snapline-overlay`: write a failing test there first, then fix it.
- If it is in the app's window or OS code: fix it, and ask Barayuda to re-check that line.
