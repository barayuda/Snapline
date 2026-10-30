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
        Self {
            method: method.to_owned(),
            url,
            headers: Vec::new(),
            body: Vec::new(),
        }
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
        self.header("Content-Type", "application/json")
            .body(serde_json::to_vec(value).expect("JSON value"))
    }

    /// A request signed by `SigV4Signer`, with the body it was signed for.
    #[must_use]
    pub fn from_signed(signed: HttpRequest, body: Vec<u8>) -> Self {
        Self {
            method: signed.method,
            url: signed.url,
            headers: signed.headers,
            body,
        }
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
    ///
    /// # Errors
    /// When the server cannot be reached or the answer cannot be read.
    fn send(&self, request: &Request, progress: &dyn Fn(f64)) -> Result<Response, UploadError>;
}

/// Sends and returns the body, turning statuses outside 200–299 into `UploadError::Http`.
///
/// # Errors
/// When sending fails or the status is not 2xx.
pub fn send_ok(
    t: &dyn Transport,
    request: &Request,
    action: &str,
    progress: &dyn Fn(f64),
) -> Result<Vec<u8>, UploadError> {
    let response = t.send(request, progress)?;
    if (200..300).contains(&response.status) {
        Ok(response.body)
    } else {
        Err(UploadError::Http {
            action: action.to_owned(),
            status: response.status,
            body: response.body,
        })
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
        let unreachable =
            |e: ureq::Error| UploadError::message(format!("Snapline could not reach {host}: {e}"));
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(request.url.as_str());
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let mut response = if request.body.is_empty() {
            let r = builder
                .body(())
                .map_err(|e| UploadError::message(e.to_string()))?;
            self.0.run(r).map_err(unreachable)?
        } else {
            // An explicit length, so S3 gets Content-Length rather than chunked encoding.
            builder = builder.header("Content-Length", request.body.len().to_string());
            let mut reader = Progress {
                body: &request.body,
                sent: 0,
                report: progress,
            };
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
            let replies = replies.into_iter().map(|(status, body)| Response {
                status,
                body: body.as_bytes().to_vec(),
            });
            Self {
                sent: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.collect()),
            }
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
            Ok(self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Response {
                    status: 599,
                    body: b"no reply scripted".to_vec(),
                }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Answers one request with `reply` and hands back what it received.
    fn one_shot_server(reply: &'static str) -> (url::Url, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url =
            url::Url::parse(&format!("http://{}/upload", listener.local_addr().unwrap())).unwrap();
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
        let (url, server) = one_shot_server(
            "HTTP/1.1 201 Created\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let request = Request::new("PUT", url)
            .header("Content-Type", "image/png")
            .body(b"hello".to_vec());
        let response = UreqTransport::new()
            .send(&request, &move |p| log.lock().unwrap().push(p))
            .unwrap();
        assert_eq!(
            (response.status, response.body.as_slice()),
            (201, b"ok".as_slice())
        );
        let received = server.join().unwrap().to_ascii_lowercase();
        assert!(
            received.contains("content-length: 5") && received.ends_with("hello"),
            "{received}"
        );
        assert!((seen.lock().unwrap().last().copied().unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn error_statuses_are_returned_not_raised() {
        let (url, _server) = one_shot_server(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnope",
        );
        let response = UreqTransport::new()
            .send(&Request::new("GET", url), &|_| {})
            .unwrap();
        assert_eq!(response.status, 404);
    }

    // Review Focus 1.
    #[test]
    fn unreachable_server_gives_a_sentence() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port(); // closed once dropped
        let url = url::Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let error = UreqTransport::new()
            .send(&Request::new("GET", url), &|_| {})
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Snapline could not reach 127.0.0.1"),
            "{error}"
        );
    }

    #[test]
    fn send_ok_turns_error_statuses_into_errors() {
        let fake = fake::FakeTransport::new(vec![(200, "fine"), (500, "boom")]);
        let request = Request::new("GET", url::Url::parse("https://example.com/").unwrap());
        assert_eq!(send_ok(&fake, &request, "Test", &|_| {}).unwrap(), b"fine");
        assert_eq!(
            send_ok(&fake, &request, "Test", &|_| {})
                .unwrap_err()
                .to_string(),
            "Test failed (HTTP 500). boom"
        );
        assert_eq!(fake.requests().len(), 2);
    }
}
