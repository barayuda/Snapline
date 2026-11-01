//! HTTP for uploads: plain request and response values, a `Transport` to send them, and the ureq transport.

use crate::error::UploadError;
use crate::proxy::SystemProxy;
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

/// How long one request may take: a minute, plus time to send the body at 32 KB/s (a slow hotel or phone link).
/// A dead network still ends with an error; a big screenshot on a slow link still gets through.
#[must_use]
pub fn time_limit(body_len: usize) -> Duration {
    Duration::from_secs(60 + (body_len / (32 * 1024)) as u64)
}

/// The real transport: ureq with rustls, checking certificates against the system's store (so a company's own
/// root, added to Windows, is trusted). Each request has a time limit (see `time_limit`), so a dead network never
/// hangs the app.
pub struct UreqTransport {
    direct: ureq::Agent,
    /// Windows' proxy setting, with one agent per proxy address in it.
    system: Option<(SystemProxy, Vec<(String, ureq::Agent)>)>,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    /// Connects directly, or through `HTTPS_PROXY` / `ALL_PROXY` when one is set.
    #[must_use]
    pub fn new() -> Self {
        Self::with_system_proxy(None)
    }

    /// As `new`, plus `system` (Windows' proxy setting) for the hosts it covers. A proxy variable
    /// (`HTTPS_PROXY`, `ALL_PROXY`) wins over the Windows setting, as it does for other tools.
    #[must_use]
    pub fn with_system_proxy(system: Option<SystemProxy>) -> Self {
        let from_env = ureq::Proxy::try_from_env();
        let system = system.filter(|_| from_env.is_none()).map(|s| {
            let mut agents: Vec<(String, ureq::Agent)> = Vec::new();
            for address in [&s.http, &s.https].into_iter().flatten() {
                let uri = if address.contains("://") {
                    address.clone()
                } else {
                    format!("http://{address}")
                };
                if agents.iter().all(|(a, _)| a != address)
                    && let Ok(proxy) = ureq::Proxy::new(&uri)
                {
                    agents.push((address.clone(), agent(Some(proxy))));
                }
            }
            (s, agents)
        });
        Self {
            direct: agent(from_env),
            system,
        }
    }

    /// The agent for `url` and the Windows proxy it goes through, if any.
    fn route(&self, url: &url::Url) -> (&ureq::Agent, Option<&str>) {
        self.system
            .as_ref()
            .and_then(|(s, agents)| {
                let address = s.for_url(url)?;
                agents
                    .iter()
                    .find(|(a, _)| a == address)
                    .map(|(a, agent)| (agent, Some(a.as_str())))
            })
            .unwrap_or((&self.direct, None))
    }
}

fn agent(proxy: Option<ureq::Proxy>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .user_agent("Snapline")
        .proxy(proxy)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
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
        let (agent, via) = self.route(&request.url);
        let unreachable = |e: ureq::Error| match e {
            ureq::Error::Timeout(_) => UploadError::message(format!(
                "The upload to {host} timed out. Check the connection and try again."
            )),
            ureq::Error::ConnectProxyFailed(why) if why.contains("responded 407") => {
                UploadError::message(format!(
                    "The proxy {} set in Windows asks Snapline to sign in, which Snapline cannot do yet. \
                     Ask IT to let Snapline through, or use a network without a proxy.",
                    via.unwrap_or("server")
                ))
            }
            e => match via {
                Some(proxy) => UploadError::message(format!(
                    "Snapline could not reach {host} through the proxy {proxy} set in Windows: {e}"
                )),
                None => UploadError::message(format!("Snapline could not reach {host}: {e}")),
            },
        };
        let limit = Some(time_limit(request.body.len()));
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
            agent
                .run(agent.configure_request(r).timeout_global(limit).build())
                .map_err(unreachable)?
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
            agent
                .run(agent.configure_request(r).timeout_global(limit).build())
                .map_err(unreachable)?
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

    // Review finding: a fixed 120 s limit failed big screenshots on slow links.
    #[test]
    fn time_limit_grows_with_the_upload() {
        assert_eq!(time_limit(0), Duration::from_secs(60));
        let twenty_mb = time_limit(20 * 1024 * 1024);
        assert!(twenty_mb >= Duration::from_mins(11), "{twenty_mb:?}");
    }

    // Phase 5c: networks that inspect TLS add their own root to Windows, so the system store must be used.
    #[test]
    fn certificates_come_from_the_system_store() {
        let transport = UreqTransport::new();
        assert!(matches!(
            transport.direct.config().tls_config().root_certs(),
            ureq::tls::RootCerts::PlatformVerifier
        ));
    }

    // Phase 5c: Windows' proxy setting is used for hosts it covers.
    #[test]
    fn requests_go_through_the_windows_proxy() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let seen = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            line
        });
        let system = crate::proxy::SystemProxy::from_registry(true, &address, "");
        let request = Request::new(
            "GET",
            url::Url::parse("http://uploads.example.invalid/x").unwrap(),
        );
        let _ = UreqTransport::with_system_proxy(system).send(&request, &|_| {});
        let line = seen.join().unwrap();
        assert!(
            line.starts_with("CONNECT uploads.example.invalid:80"),
            "{line}"
        );
    }

    // Review Focus 2: a local server on the bypass list is reached directly, even with a dead proxy.
    #[test]
    fn bypassed_hosts_are_reached_directly() {
        let (url, _server) =
            one_shot_server("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        let system = crate::proxy::SystemProxy::from_registry(true, "127.0.0.1:1", "127.0.0.1");
        let response = UreqTransport::with_system_proxy(system)
            .send(&Request::new("GET", url), &|_| {})
            .unwrap();
        assert_eq!(response.status, 200);
    }

    // Review Focus 1. `one_shot_server` plays the proxy: its URL's host:port is the proxy address.
    #[test]
    fn a_proxy_that_wants_a_sign_in_is_named() {
        let (proxy_url, _proxy) = one_shot_server(
            "HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        let address = format!(
            "{}:{}",
            proxy_url.host_str().unwrap(),
            proxy_url.port().unwrap()
        );
        let system = crate::proxy::SystemProxy::from_registry(true, &address, "");
        let request = Request::new(
            "GET",
            url::Url::parse("http://uploads.example.invalid/x").unwrap(),
        );
        let error = UreqTransport::with_system_proxy(system)
            .send(&request, &|_| {})
            .unwrap_err()
            .to_string();
        assert!(error.contains("asks Snapline to sign in"), "{error}");
    }
}
