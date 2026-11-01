# Phase 5c: Windows fixes before the first release — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix what would hurt the first Windows release:
- uploads work on office and hotel networks that use a proxy or inspect TLS;
- the problems users notice from earlier reviews are fixed;
- the release workflow is safer;
- the README and its banner describe a Mac and Windows app.

**Architecture:**
- **Certificates:** ureq's `platform-verifier` feature checks certificates against the Windows certificate store, including roots a company adds. It replaces the bundled web roots.
- **Proxy:** `snapline-upload` gets a pure `SystemProxy`. It parses Windows' manual proxy setting and bypass list, and picks a proxy per URL. The app reads the setting from the registry and hands it to `UreqTransport`, which keeps one agent per proxy. `HTTPS_PROXY` / `ALL_PROXY` still win when set.
- **Minors:**
  - The shortcut recorder decides through a pure `shortcuts::record`.
  - ShellExecute codes become sentences, and temp files never collide.
  - The tray menu is updated in place instead of the tray icon being rebuilt.
  - A crashed scan gets its own badge.
- **Release workflow:** signing moves into its own job. That job runs no `cargo build`, so dependency build scripts never see an OIDC token.
- **Branding:** the brand script writes the banner and the website's social image from the same drawing.

**Tech Stack:** Rust 2024; ureq 3.5 with `platform-verifier` (pulls `rustls-platform-verifier` 0.7); windows-sys registry calls (feature already on); tray-icon 0.26 `set_menu` / `set_show_menu_on_left_click`; GitHub Actions; Inno Setup 6; Swift + CoreGraphics brand script.

**Spec:** [docs/windows-linux.md](../windows-linux.md) (Phase 5). This plan adds "5c" to it.

## Decisions made for this phase

Barayuda's choices on 31 October 2026:

| Question | Choice |
|---|---|
| Proxy and certificates | **Manual proxy + certificate store.** Use the proxy from Settings → Network & internet → Proxy → "Use a proxy server" (registry `ProxyEnable`, `ProxyServer`, `ProxyOverride`) and `HTTPS_PROXY`, and the Windows certificate store. PAC scripts and WPAD ("Automatically detect settings", "Use setup script") are not supported. |
| Banner label | **"macOS · WINDOWS · ON-DEVICE PRIVACY".** Linux is added when Phase 6 ships a Linux build. |
| README | **Platform-neutral.** The intro and Privacy cover Mac and Windows, then there is a macOS section and a Windows section. The website's `og.png` comes from the same script. |
| Minors | **All four groups:** <ul><li>shortcut recorder;</li><li>Print/Edit/Share;</li><li>tray and scan;</li><li>release and docs.</li></ul> |

Choices made in this plan:
- **Dependencies:** every direct dependency is already at its latest stable release (checked 31 Oct 2026).
  - Only the lockfile moves (`cargo update`: 12 patch releases).
  - winit stays on 0.30 because 0.31 is a beta.
  - windows-collections stays on 0.3, because its types must match `windows` 0.62 (the 0.100 line pairs with a different `windows`).
  - The Rust toolchain is `stable` (1.99.1), and the actions are at their latest majors.
- **Proxy that wants a sign-in (HTTP 407):** not supported. Snapline says so in plain words instead of "CONNECT proxy failed".
- **Reserved shortcuts:** Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+Z, Ctrl+A, Ctrl+S and Alt+F4.

Left for later: PAC/WPAD, proxy sign-in (NTLM/Kerberos), Linux certificates without `ca-certificates` (Phase 6). Also left are the other deferred minors listed in PR #1:
- the uninstaller isn't signed;
- `releases/latest` could point to a Mac release;
- small icon sizes are blurred;
- the exe has no icon when cross-compiled;
- the home page needs spacing;
- `package.ps1 -Out` is relative to the caller's folder;
- the phase 3 and phase 4 minors.

## Global Constraints

- Every command chain is gated with `&&`. Never run `git commit` after a check that can fail, unless the chain is gated.
- This Mac cannot build the Windows target. Before using a Windows API in code, check its name and signature in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. Workflow and Windows-only changes can only be proven on CI: push, then read the run.
- `cargo clippy --workspace --all-targets -- -D warnings` (pedantic) and `cargo fmt --check` must stay clean, on the Mac and on CI.
- Text shown to users: plain English, sentence case, and it says what to do next.
- Git rules:
  - one commit per task, GPG-signed, in plain sentence case;
  - no AI co-author or "generated with" lines;
  - check that `git log --format='%h %G?'` shows `G` before pushing.
- Publishing is Barayuda's step. The agent never pushes a tag, publishes a release, or pushes to `website`.

## Review Focus

1. **A proxy that asks for a sign-in (407).** Snapline must say the proxy wants a sign-in it cannot give, not "CONNECT proxy failed: proxy server responded 407/407". Task 2 has the test.
2. **The bypass list is honoured.** These must connect directly:
   - `<local>` (names without a dot);
   - `*.corp.example`;
   - `10.*`;
   - exact names, ignoring case.

   Everything else goes through the proxy. A local MinIO at `http://127.0.0.1:9000` must still work with a proxy on. Task 2 has both tests.
3. **The certificate store on every OS.** The platform verifier must build on Windows, macOS and Linux CI, and existing plain-HTTP tests must still pass. Task 1 pins the setting with a test; the Windows check uploads to Google Drive and S3 for real.
4. **The recorder never gets stuck.** Esc and the Cancel button always stop recording. A refused key keeps recording and shows why, and a good key saves and stops. Task 3 has the tests.
5. **The release workflow without signing.** With no `AZURE_SIGNING_ACCOUNT`, the sign job is skipped and the release job still runs with the unsigned files. With signing on, the release gets the signed files. This can't be run without a tag. Task 6 reviews the job conditions line by line, and the release guide says what to check on the first tag.

---

## File structure

```
windows-linux/crates/snapline-upload/
├── Cargo.toml              ureq + platform-verifier
└── src/proxy.rs            new: SystemProxy (parse Windows' setting, pick a proxy per URL)
    src/http.rs             UreqTransport: certificate store, one agent per proxy, 407 message
    src/lib.rs              + pub mod proxy
windows-linux/crates/snapline/src/
├── platform.rs             + system_proxy() (registry), shell_error(), select_arg()
├── app.rs                  transport with system proxy; tray menu in place; scan crash badge
├── upload_settings.rs      transport with system proxy; recorder keeps recording
├── shortcuts.rs            + record(), Recorded, RESERVED_KEY
├── actions.rs              temp files never collide; clearer Print/Edit errors
└── recognize.rs            + FAILED, CRASHED, badge_for()
windows-linux/installer/    package.ps1 + FileVersion; snapline.iss uses it
.github/workflows/          windows-linux.yml: proxy-setting and pre-release checks; windows-release.yml: sign job
scripts/brand/make-brand.swift   new label; also writes site/assets/og.png
Assets/banner.png, site/assets/og.png   regenerated
README.md, docs/releasing-windows.md, docs/windows-linux.md
```

---

### Task 1: Certificates from the system store, dependencies current

**Files:**
- Modify: `windows-linux/crates/snapline-upload/Cargo.toml`
- Modify: `windows-linux/crates/snapline-upload/src/http.rs` (`UreqTransport::new`, tests)
- Modify: `windows-linux/Cargo.lock` (through `cargo update`)

**Interfaces:**
- Consumes: nothing new.
- Produces: `UreqTransport` whose agents use `RootCerts::PlatformVerifier`. Task 2 builds its agents with the same settings through a helper `agent(proxy: Option<ureq::Proxy>) -> ureq::Agent`, introduced here.

- [ ] **Step 1: Write the failing test** at the end of `mod tests` in `http.rs`:

```rust
    // Phase 5c: networks that inspect TLS add their own root to Windows, so the system store must be used.
    #[test]
    fn certificates_come_from_the_system_store() {
        let transport = UreqTransport::new();
        assert!(matches!(
            transport.direct.config().tls_config().root_certs(),
            ureq::tls::RootCerts::PlatformVerifier
        ));
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd windows-linux && cargo test -p snapline-upload certificates_come_from_the_system_store`
Expected: does not compile (`no field direct`). That is the RED: the transport has no system-store agent yet.

- [ ] **Step 3: Implement.** In `snapline-upload/Cargo.toml`:

```toml
ureq = { version = "3.5.0", features = ["platform-verifier"] }
```

In `http.rs`, replace the `UreqTransport` struct, `Default` and `impl UreqTransport` with:

```rust
/// The real transport: ureq with rustls, checking certificates against the system's store (so a company's own
/// root, added to Windows, is trusted). Each request has a time limit (see `time_limit`), so a dead network never
/// hangs the app.
pub struct UreqTransport {
    direct: ureq::Agent,
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
        Self {
            direct: agent(ureq::Proxy::try_from_env()),
        }
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
```

In `send`, use `self.direct` where it says `self.0` (four places).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p snapline-upload`
Expected: PASS, including `certificates_come_from_the_system_store` and the three existing ureq tests.

- [ ] **Step 5: Lockfile.** Run `cargo update`. Expected: about 12 patch updates (`log`, `js-sys`, `wasm-bindgen`…), with no change to a direct dependency's major or minor version.

- [ ] **Step 6: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cd .. && git add windows-linux && git commit -m "Trust the system certificate store for uploads, update the lockfile" && git log --format='%h %G?' -1 && git push
```

Expected: the commit shows `G`. CI must pass on windows-latest and ubuntu-latest (the platform verifier compiles on both).

---

### Task 2: Windows' proxy setting for uploads

**Files:**
- Create: `windows-linux/crates/snapline-upload/src/proxy.rs`
- Modify: `windows-linux/crates/snapline-upload/src/lib.rs` (`pub mod proxy;` between `http` and `s3`)
- Modify: `windows-linux/crates/snapline-upload/src/http.rs`
- Modify: `windows-linux/crates/snapline/src/platform.rs` (add `system_proxy`, `mod registry`)
- Modify: `windows-linux/crates/snapline/src/app.rs:596`, `upload_settings.rs:214`, `upload_settings.rs:239`
- Modify: `.github/workflows/windows-linux.yml` (a step that checks the registry reader)

**Interfaces:**
- Consumes: `agent(proxy)` and `UreqTransport { direct }` from Task 1.
- Produces:
  - `snapline_upload::proxy::SystemProxy { http: Option<String>, https: Option<String>, bypass: Vec<String> }`;
  - `SystemProxy::from_registry(enabled: bool, server: &str, overrides: &str) -> Option<SystemProxy>`;
  - `SystemProxy::for_url(&self, url: &url::Url) -> Option<&str>`;
  - `UreqTransport::with_system_proxy(system: Option<SystemProxy>) -> UreqTransport`;
  - `snapline::platform::system_proxy() -> Option<SystemProxy>`.

- [ ] **Step 1: Write the failing tests.** Create `proxy.rs` with only the tests, so nothing else compiles yet:

```rust
//! The proxy Windows is set to use (Settings → Network & internet → Proxy → "Use a proxy server"). The app reads
//! the registry; this decides which proxy a URL goes through. Automatic setup (PAC scripts, WPAD) is not supported.

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    #[test]
    fn one_server_for_every_scheme() {
        let p = SystemProxy::from_registry(true, "proxy.corp:8080", "").unwrap();
        assert_eq!(p.for_url(&url("https://www.googleapis.com/upload")), Some("proxy.corp:8080"));
        assert_eq!(p.for_url(&url("http://minio.example:9000/b")), Some("proxy.corp:8080"));
    }

    #[test]
    fn a_server_per_scheme() {
        let p = SystemProxy::from_registry(true, "http=a:80;https=b:443;socks=c:1080", "").unwrap();
        assert_eq!(p.for_url(&url("https://x.example/")), Some("b:443"));
        assert_eq!(p.for_url(&url("http://x.example/")), Some("a:80"));
    }

    #[test]
    fn off_or_empty_means_no_proxy() {
        assert_eq!(SystemProxy::from_registry(false, "a:80", ""), None);
        assert_eq!(SystemProxy::from_registry(true, "  ", ""), None);
        assert_eq!(SystemProxy::from_registry(true, "socks=c:1080", ""), None);
    }

    // Review Focus 2.
    #[test]
    fn the_bypass_list_follows_windows_rules() {
        let p = SystemProxy::from_registry(true, "a:80", "<local>;*.corp.example;10.*;Storage.Internal").unwrap();
        for direct in [
            "https://intranet/",
            "https://files.corp.example/",
            "https://10.1.2.3/",
            "https://storage.internal/",
        ] {
            assert_eq!(p.for_url(&url(direct)), None, "{direct}");
        }
        for proxied in [
            "https://www.googleapis.com/",
            "https://corp.example.com/",
            "https://110.1.2.3/",
        ] {
            assert_eq!(p.for_url(&url(proxied)), Some("a:80"), "{proxied}");
        }
    }
}
```

Add to `http.rs`'s `mod tests` (they use `one_shot_server`, which is already there):

```rust
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
        let request = Request::new("GET", url::Url::parse("http://uploads.example.invalid/x").unwrap());
        let _ = UreqTransport::with_system_proxy(system).send(&request, &|_| {});
        let line = seen.join().unwrap();
        assert!(line.starts_with("CONNECT uploads.example.invalid:80"), "{line}");
    }

    // Review Focus 2: a local server on the bypass list is reached directly, even with a dead proxy.
    #[test]
    fn bypassed_hosts_are_reached_directly() {
        let (url, _server) = one_shot_server(
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        );
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
        let address = format!("{}:{}", proxy_url.host_str().unwrap(), proxy_url.port().unwrap());
        let system = crate::proxy::SystemProxy::from_registry(true, &address, "");
        let request = Request::new("GET", url::Url::parse("http://uploads.example.invalid/x").unwrap());
        let error = UreqTransport::with_system_proxy(system)
            .send(&request, &|_| {})
            .unwrap_err()
            .to_string();
        assert!(error.contains("asks Snapline to sign in"), "{error}");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd windows-linux && cargo test -p snapline-upload proxy`
Expected: does not compile: `SystemProxy` and `with_system_proxy` don't exist yet.

- [ ] **Step 3: Implement `SystemProxy`.** In `proxy.rs`, above `mod tests`:

```rust
/// Windows' manual proxy: one server for every scheme or one per scheme, and the hosts that skip it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemProxy {
    pub http: Option<String>,
    pub https: Option<String>,
    /// `ProxyOverride` entries, lower-case: `<local>`, `*.corp.example`, `10.*`, exact names.
    pub bypass: Vec<String>,
}

impl SystemProxy {
    /// From the registry values `ProxyEnable`, `ProxyServer` and `ProxyOverride`; `None` when the proxy is off or has
    /// no HTTP or HTTPS server. `ProxyServer` is "host:port" for every scheme, or "http=host:port;https=host:port;…".
    #[must_use]
    pub fn from_registry(enabled: bool, server: &str, overrides: &str) -> Option<Self> {
        if !enabled {
            return None;
        }
        let mut proxy = Self::default();
        for part in server.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            match part.split_once('=') {
                Some((scheme, address)) if scheme.eq_ignore_ascii_case("http") => {
                    proxy.http = Some(address.trim().to_owned());
                }
                Some((scheme, address)) if scheme.eq_ignore_ascii_case("https") => {
                    proxy.https = Some(address.trim().to_owned());
                }
                Some(_) => {} // ftp=, socks=: uploads never use them
                None => {
                    proxy.http = Some(part.to_owned());
                    proxy.https = Some(part.to_owned());
                }
            }
        }
        proxy.bypass = overrides
            .split(';')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
        (proxy.http.is_some() || proxy.https.is_some()).then_some(proxy)
    }

    /// The proxy ("host:port") for `url`, or `None` to connect directly.
    #[must_use]
    pub fn for_url(&self, url: &url::Url) -> Option<&str> {
        let host = url.host_str()?.to_ascii_lowercase();
        if self.bypass.iter().any(|p| bypassed(&host, p)) {
            return None;
        }
        match url.scheme() {
            "https" => self.https.as_deref(),
            "http" => self.http.as_deref(),
            _ => None,
        }
    }
}

/// One `ProxyOverride` entry: `<local>` is any name without a dot; `*` matches any run of characters.
fn bypassed(host: &str, pattern: &str) -> bool {
    if pattern == "<local>" {
        return !host.contains('.');
    }
    glob(host.as_bytes(), pattern.as_bytes())
}

fn glob(text: &[u8], pattern: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|i| glob(&text[i..], rest)),
        Some((c, rest)) => text.first() == Some(c) && glob(&text[1..], rest),
    }
}
```

Add `pub mod proxy;` to `lib.rs`.

- [ ] **Step 4: Implement the transport.** In `http.rs`, extend the struct and add the constructor:

```rust
pub struct UreqTransport {
    direct: ureq::Agent,
    /// Windows' proxy setting, with one agent per proxy address in it.
    system: Option<(crate::proxy::SystemProxy, Vec<(String, ureq::Agent)>)>,
}
```

`new()` becomes `Self::with_system_proxy(None)`, and:

```rust
    /// As `new`, plus `system` (Windows' proxy setting) for the hosts it covers. A proxy variable
    /// (`HTTPS_PROXY`, `ALL_PROXY`) wins over the Windows setting, as it does for other tools.
    #[must_use]
    pub fn with_system_proxy(system: Option<crate::proxy::SystemProxy>) -> Self {
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
```

In `send`, start with `let (agent, via) = self.route(&request.url);`, use `agent` where it says `self.direct`, and replace the `unreachable` closure with:

```rust
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
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p snapline-upload`
Expected: PASS, including the four `proxy` tests, the three new transport tests, and `unreachable_server_gives_a_sentence`.

If `requests_go_through_the_windows_proxy` fails because this machine has `HTTPS_PROXY` set, that is the env-wins rule. Unset it for the run; CI has none.

- [ ] **Step 6: Read the registry in the app.** In `platform.rs`, after `reveal`:

```rust
/// Windows' manual proxy for this user (Settings → Network & internet → Proxy → "Use a proxy server"), when it is
/// on. Elsewhere `None`; ureq still reads `HTTPS_PROXY`.
#[must_use]
pub fn system_proxy() -> Option<snapline_upload::proxy::SystemProxy> {
    #[cfg(windows)]
    {
        const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        let enabled = registry::dword(KEY, "ProxyEnable").is_some_and(|v| v != 0);
        let server = registry::string(KEY, "ProxyServer").unwrap_or_default();
        let overrides = registry::string(KEY, "ProxyOverride").unwrap_or_default();
        snapline_upload::proxy::SystemProxy::from_registry(enabled, &server, &overrides)
    }
    #[cfg(not(windows))]
    None
}

/// Reading values under `HKEY_CURRENT_USER`.
#[cfg(windows)]
mod registry {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// A text value, if it exists.
    pub fn string(key: &str, name: &str) -> Option<String> {
        let (key, name) = (wide(key), wide(name));
        let mut buf = vec![0u16; 4096];
        let mut bytes = u32::try_from(buf.len() * 2).unwrap_or(u32::MAX);
        // SAFETY: the strings are NUL-terminated; `buf` holds `bytes` bytes; all live across the call.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr().cast(),
                &raw mut bytes,
            )
        };
        // `bytes` counts the closing NUL too.
        (status == ERROR_SUCCESS)
            .then(|| String::from_utf16_lossy(&buf[..(bytes as usize / 2).saturating_sub(1)]))
    }

    /// A number value, if it exists.
    pub fn dword(key: &str, name: &str) -> Option<u32> {
        let (key, name) = (wide(key), wide(name));
        let mut value = 0u32;
        let mut bytes = 4u32;
        // SAFETY: as above; `value` is the 4 bytes `bytes` says.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&raw mut value).cast(),
                &raw mut bytes,
            )
        };
        (status == ERROR_SUCCESS).then_some(value)
    }

    #[cfg(test)]
    mod tests {
        // CI sets this user's proxy first (see windows-linux.yml); never run it on your own PC with a proxy you need.
        #[test]
        #[ignore = "expects the proxy setting CI writes"]
        fn reads_the_windows_proxy_setting() {
            let p = super::super::system_proxy().expect("CI turned the proxy on");
            assert_eq!(p.https.as_deref(), Some("127.0.0.1:3128"));
            assert_eq!(p.bypass, vec!["<local>".to_owned(), "*.corp.example".to_owned()]);
        }
    }
}
```

Before pushing, check in the registry sources that `RRF_RT_REG_DWORD` exists in windows-sys 0.61 (`grep -rn "RRF_RT_REG_DWORD" ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/windows-sys-0.61.2/src/Windows/Win32/System/Registry/mod.rs`).

Use the setting in the three places that make a transport:
- `app.rs:596`: `&UreqTransport::with_system_proxy(platform::system_proxy())`
- `upload_settings.rs:214` and `:239`: `&UreqTransport::with_system_proxy(crate::platform::system_proxy())`

- [ ] **Step 7: CI check for the registry reader.** In `.github/workflows/windows-linux.yml`, after "Lint and test (Windows)":

```yaml
      - name: Read Windows' proxy setting
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
          Set-ItemProperty $key -Name ProxyEnable -Value 1 -Type DWord
          Set-ItemProperty $key -Name ProxyServer -Value '127.0.0.1:3128'
          Set-ItemProperty $key -Name ProxyOverride -Value '<local>;*.corp.example'
          try { cargo test -p snapline -- --ignored reads_the_windows_proxy_setting }
          finally { Set-ItemProperty $key -Name ProxyEnable -Value 0 -Type DWord }
          if ($LASTEXITCODE) { throw "reading the proxy setting failed" }
```

- [ ] **Step 8: Full check, commit, push, read CI**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cd .. && git add windows-linux .github/workflows/windows-linux.yml && git commit -m "Send uploads through the proxy set in Windows" && git log --format='%h %G?' -1 && git push
```

Expected: CI passes. The Windows log of "Read Windows' proxy setting" shows `test platform::registry::tests::reads_the_windows_proxy_setting ... ok`.

---

### Task 3: The shortcut recorder refuses common shortcuts and keeps recording

**Files:**
- Modify: `windows-linux/crates/snapline/src/shortcuts.rs`
- Modify: `windows-linux/crates/snapline/src/upload_settings.rs:450-484` (`record_shortcut`)

**Interfaces:**
- Consumes: `from_event`, `NEEDS_MODIFIER`, `UNKNOWN_KEY`, `is_free` (existing).
- Produces: `pub enum Recorded { Shortcut(String), Cancelled, TryAgain(&'static str) }`, `pub fn record(key: Key, physical_key: Option<Key>, mods: Modifiers) -> Recorded`, `pub const RESERVED_KEY: &str`.

- [ ] **Step 1: Write the failing tests** in `shortcuts.rs`'s `mod tests`:

```rust
    // Phase 5c: taking Ctrl+C or Alt+F4 would break them in every other app.
    #[test]
    fn common_shortcuts_are_refused() {
        let ctrl = Modifiers { ctrl: true, ..Modifiers::default() };
        let alt = Modifiers { alt: true, ..Modifiers::default() };
        for key in [Key::C, Key::V, Key::X, Key::Z, Key::A, Key::S] {
            assert_eq!(record(key, Some(key), ctrl), Recorded::TryAgain(RESERVED_KEY), "{key:?}");
        }
        assert_eq!(record(Key::F4, Some(Key::F4), alt), Recorded::TryAgain(RESERVED_KEY));
        assert_eq!(record(Key::D, Some(Key::D), ctrl), Recorded::Shortcut("Ctrl+D".into()));
    }

    // Phase 5c (Review Focus 4): as on macOS, a key Snapline cannot use keeps recording; Esc stops it.
    #[test]
    fn a_key_snapline_cannot_use_keeps_recording() {
        assert_eq!(
            record(Key::A, Some(Key::A), Modifiers::default()),
            Recorded::TryAgain(NEEDS_MODIFIER)
        );
        assert_eq!(
            record(Key::Escape, Some(Key::Escape), Modifiers::default()),
            Recorded::Cancelled
        );
    }
```

`Key` is already imported at the top of the file (`use eframe::egui::{Key, Modifiers};`).

- [ ] **Step 2: Run them to see them fail**

Run: `cd windows-linux && cargo test -p snapline shortcuts`
Expected: does not compile: `record`, `Recorded` and `RESERVED_KEY` don't exist yet.

- [ ] **Step 3: Implement** in `shortcuts.rs`, after `from_event`:

```rust
pub const RESERVED_KEY: &str =
    "Other apps need that shortcut (copy, paste, save, close…). Choose another one.";

/// Shortcuts every Windows app relies on; taking one as a global shortcut breaks it everywhere.
const RESERVED: [&str; 7] = ["Ctrl+C", "Ctrl+V", "Ctrl+X", "Ctrl+Z", "Ctrl+A", "Ctrl+S", "Alt+F4"];

/// What one key press does while recording.
#[derive(Debug, PartialEq, Eq)]
pub enum Recorded {
    /// Use this shortcut and stop recording.
    Shortcut(String),
    /// Esc: stop recording, keep the old shortcut.
    Cancelled,
    /// Keep recording and show why that key cannot be used.
    TryAgain(&'static str),
}

#[must_use]
pub fn record(key: Key, physical_key: Option<Key>, mods: Modifiers) -> Recorded {
    if key == Key::Escape {
        return Recorded::Cancelled;
    }
    match from_event(key, physical_key, mods) {
        Ok(name) if is_reserved(&name) => Recorded::TryAgain(RESERVED_KEY),
        Ok(name) => Recorded::Shortcut(name),
        Err(why) => Recorded::TryAgain(why),
    }
}

fn is_reserved(name: &str) -> bool {
    let key = name.parse::<HotKey>().ok();
    RESERVED.iter().any(|r| r.parse::<HotKey>().ok() == key)
}
```

In `upload_settings.rs`, replace `record_shortcut`'s body after `let Some((key, physical_key, m)) = pressed else { return; };` with:

```rust
        match crate::shortcuts::record(key, physical_key, m) {
            Recorded::Cancelled => self.recording = false,
            Recorded::TryAgain(why) => self.general_error = Some(why.into()),
            Recorded::Shortcut(name) if !crate::shortcuts::is_free(&name, &self.settings.hotkeys) => {
                self.general_error =
                    Some("That shortcut is used by another app. Try another one.".into());
            }
            Recorded::Shortcut(name) => {
                self.recording = false;
                let print_screen = self.settings.uses_print_screen();
                self.save_general(|s| s.set_shortcuts(Some(name), print_screen));
            }
        }
```

Add `use crate::shortcuts::Recorded;` with the file's other `use` lines, and update the doc comment: "While recording: the next usable key becomes the capture shortcut; a key Snapline cannot use keeps recording and says why; Esc cancels."

- [ ] **Step 4: Run the tests**

Run: `cargo test -p snapline shortcuts`
Expected: PASS (4 tests).

- [ ] **Step 5: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cd .. && git add windows-linux && git commit -m "Refuse shortcuts other apps need and keep recording after a bad key" && git log --format='%h %G?' -1 && git push
```

---

### Task 4: Clear Print and Edit errors, temp files that never collide, Explorer paths with spaces

**Files:**
- Modify: `windows-linux/crates/snapline/src/platform.rs` (`shell_verb`, `reveal`, new `shell_error`, `select_arg`)
- Modify: `windows-linux/crates/snapline/src/actions.rs` (`temp_png`, the `Edit` and `Print` arms)

**Interfaces:**
- Consumes: `snapline_core::naming::unique_file_path(dir: &Path, name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf`.
- Produces: `pub fn shell_error(verb: &str, code: usize) -> String`, `pub fn select_arg(file: &Path) -> String` in `platform`.

- [ ] **Step 1: Write the failing tests.** In `actions.rs` `mod tests`:

```rust
    // Phase 5c: names change once a second, so Print then Edit in the same second wrote over the same file.
    #[test]
    fn temp_files_never_collide() {
        let name = format!("Snapline collide {}.png", std::process::id());
        let first = temp_png(b"one", &name).unwrap();
        let second = temp_png(b"two", &name).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
    }
```

At the end of `platform.rs` (add `#[cfg(test)] mod tests { use super::*; … }` if the file has none):

```rust
    // Phase 5c: "code 31" told the user nothing.
    #[test]
    fn shell_errors_say_what_to_do() {
        let none = shell_error("print", 31);
        assert!(none.contains("Default apps") && !none.contains("31"), "{none}");
        assert!(shell_error("open", 2).contains("could not find"));
        assert!(shell_error("open", 99).contains("error 99"));
    }

    // Phase 5c: Explorer did not select a file whose path has spaces.
    #[test]
    fn explorer_gets_the_path_in_quotes() {
        let path = std::path::Path::new(r"C:\Users\A B\AppData\Local\Temp\Snapline\Shot 1.png");
        assert_eq!(
            select_arg(path),
            r#"/select,"C:\Users\A B\AppData\Local\Temp\Snapline\Shot 1.png""#
        );
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd windows-linux && cargo test -p snapline -- temp_files_never_collide shell_errors_say explorer_gets`
Expected: does not compile (no `shell_error`, `select_arg`). Add the two functions as stubs that return `String::new()`, and run it again:
- `temp_files_never_collide` fails on `assert_ne!` (the same path twice);
- the two `platform` tests fail on their asserts.

- [ ] **Step 3: Implement.** In `platform.rs`, not behind `cfg`:

```rust
/// What to tell the user when ShellExecute fails with `code` (its result, 32 or less).
#[must_use]
#[cfg_attr(not(windows), allow(dead_code, reason = "used by shell_verb on Windows"))]
pub fn shell_error(verb: &str, code: usize) -> String {
    match code {
        // SE_ERR_NOASSOC, SE_ERR_ASSOCINCOMPLETE
        27 | 31 => format!(
            "No app on this PC is set up to {verb} PNG images. Pick one in Settings → Apps → Default apps (for .png)."
        ),
        2 | 3 => format!("Windows could not find the screenshot file to {verb}."),
        5 => format!("Windows did not allow Snapline to {verb} the file."),
        _ => format!("Windows could not {verb} the file (error {code})."),
    }
}

/// Explorer's argument to show `file` selected. Explorer wants the path in quotes after the comma, so the
/// argument is passed as is (`raw_arg`), not quoted again as a whole.
#[must_use]
#[cfg_attr(not(windows), allow(dead_code, reason = "used by reveal on Windows"))]
pub fn select_arg(file: &std::path::Path) -> String {
    format!("/select,\"{}\"", file.display())
}
```

In the Windows `shell_verb`, replace the `Err(format!("Windows could not {verb} the file (code {code})"))` line with `Err(shell_error(verb, code))`.

In `reveal`, the Windows branch becomes:

```rust
    #[cfg(windows)]
    let shown = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(select_arg(file))
            .spawn()
            .map(|_| ())
    };
```

In `actions.rs`, in `temp_png`, replace `let path = dir.join(name);` with:

```rust
    // Names change once a second, so Print then Edit in the same second must not share a file.
    let path = naming::unique_file_path(&dir, name, Path::exists);
```

The `Print` and `Edit` arms use a full stop instead of a colon, so the sentence from `shell_error` reads on its own:

```rust
            platform::shell_verb("print", &file)
                .map_err(|e| format!("Snapline could not print. {e}"))?;
```

```rust
            edit(&file, settings).map_err(|e| {
                format!("Snapline could not open an editor. {e} You can also choose one in Snapline settings → General.")
            })?;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p snapline`
Expected: PASS, including the three new tests and `temp_png_writes_into_the_snapline_temp_folder`.

- [ ] **Step 5: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cd .. && git add windows-linux && git commit -m "Explain Print and Edit errors, keep temp files apart, select paths with spaces" && git log --format='%h %G?' -1 && git push
```

Expected: CI passes on Windows, which proves `raw_arg` and `shell_error` compile there.

---

### Task 5: Update the tray menu in place, a separate badge for a crashed scan

**Files:**
- Modify: `windows-linux/crates/snapline/src/app.rs` (`reload_settings`, `build_tray`, `start_scan`, `ScanDone` handling)
- Modify: `windows-linux/crates/snapline/src/recognize.rs`

**Interfaces:**
- Consumes: tray-icon 0.26 `TrayIcon::set_menu(Option<Box<dyn ContextMenu>>)`, `TrayIcon::set_show_menu_on_left_click(bool)`.
- Produces: `fn build_menu(&self, labels: &[String]) -> Result<Menu, String>` (private to `app.rs`); `recognize::FAILED`, `recognize::CRASHED`, `recognize::badge_for(why: &str) -> &'static str`.

- [ ] **Step 1: Write the failing test** in `recognize.rs` `mod tests`:

```rust
    // Phase 5c: a crashed scan said "auto-hide unavailable", as if OCR were missing.
    #[test]
    fn a_crashed_scan_has_its_own_badge() {
        assert_eq!(badge_for(CRASHED), FAILED);
        assert_eq!(badge_for(ocr_unavailable(true)), UNAVAILABLE);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd windows-linux && cargo test -p snapline a_crashed_scan_has_its_own_badge`
Expected: does not compile (no `badge_for`, `CRASHED`, `FAILED`).

- [ ] **Step 3: Implement.** In `recognize.rs`, after `INCOMPLETE`:

```rust
/// Shown in the size badge when the scan itself broke (a bug in Snapline, not missing OCR).
pub const FAILED: &str = "auto-hide failed";
/// The scan's error when its thread panicked.
pub const CRASHED: &str = "the scan stopped unexpectedly";

/// The badge for a scan that ended with the error `why`.
#[must_use]
pub fn badge_for(why: &str) -> &'static str {
    if why == CRASHED { FAILED } else { UNAVAILABLE }
}
```

In `app.rs`:
- in `start_scan`'s `OnPanic` closure, `result: Err("internal error".into())` becomes `result: Err(recognize::CRASHED.into())`;
- in the `ScanDone` arm, `Err(why) => (Err(recognize::UNAVAILABLE.to_owned()), Some(why)),` becomes `Err(why) => (Err(recognize::badge_for(&why).to_owned()), Some(why)),`.

Split `build_tray`: move everything from `let menu = Menu::new();` up to the `menu.append_items(...)` call (inclusive) into a new method, and have `build_tray` call it:

```rust
    fn build_menu(&self, labels: &[String]) -> Result<Menu, String> {
        // … the existing menu-building lines, unchanged …
        Ok(menu)
    }

    fn build_tray(&self, labels: &[String]) -> Result<TrayIcon, String> {
        let menu = self.build_menu(labels)?;
        let icon = Icon::from_rgba(icon::tray_rgba(32, icon::TrayState::Normal), 32, 32)
            .map_err(|e| e.to_string())?;
        TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Snapline")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(self.settings.tray_click == TrayClick::Menu)
            .build()
            .map_err(|e| e.to_string())
    }
```

In `reload_settings`, replace `self.tray = None;` and the `match self.build_tray(&labels) { … }` that follows with:

```rust
        // Change the existing icon: a new one may land in the overflow, away from where the user pinned it.
        if let Some(tray) = &self.tray {
            match self.build_menu(&labels) {
                Ok(menu) => {
                    tray.set_menu(Some(Box::new(menu)));
                    tray.set_show_menu_on_left_click(self.settings.tray_click == TrayClick::Menu);
                }
                Err(e) => show_error(&format!("Snapline could not update its tray menu: {e}")),
            }
        } else {
            match self.build_tray(&labels) {
                Ok(tray) => self.tray = Some(tray),
                Err(e) => show_error(&format!("Snapline could not add its tray icon: {e}")),
            }
        }
```

Update `reload_settings`' doc comment: "…register the shortcuts again and update the tray menu, so the menu labels and the tray click follow…".

- [ ] **Step 4: Run the tests**

Run: `cargo test -p snapline`
Expected: PASS. The tray change has no unit test, because only Windows' tray shows it. Ledger it as a ruling; the Windows check below covers it.

- [ ] **Step 5: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cd .. && git add windows-linux && git commit -m "Keep the tray icon in place on settings changes, say when a scan crashed" && git log --format='%h %G?' -1 && git push
```

---

### Task 6: Safer release workflow, pre-release versions, release guide

**Files:**
- Modify: `.github/workflows/windows-release.yml` (whole file below)
- Modify: `.github/workflows/windows-linux.yml` (pre-release packaging check)
- Modify: `windows-linux/installer/package.ps1`, `windows-linux/installer/snapline.iss`
- Modify: `docs/releasing-windows.md`, `docs/windows-linux.md`

**Interfaces:**
- Consumes: `package.ps1 -Version -Out -SumsOnly` (existing).
- Produces: `package.ps1` passes `/DFileVersion=<x.y.z>` to ISCC; `snapline.iss` uses `{#FileVersion}` for `VersionInfoVersion`.

- [ ] **Step 1: Write the failing CI check.** In `windows-linux.yml`, after the step "Install, upgrade, check and uninstall":

```yaml
      - name: Package a pre-release version
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          ./installer/package.ps1 -Version 0.1.0-beta.1 -Out target\dist-pre
          if (-not (Test-Path target\dist-pre\Snapline-Setup-0.1.0-beta.1.exe)) { throw "no pre-release installer" }
          "pre-release packaging: ok"
```

- [ ] **Step 2: Push it alone and watch it fail**

```bash
git add .github/workflows/windows-linux.yml && git commit -m "Test packaging a pre-release version on CI" && git log --format='%h %G?' -1 && git push
```

Expected: the Windows job fails in "Package a pre-release version", with ISCC rejecting `VersionInfoVersion` "0.1.0-beta.1" (or a similar version error). Read the log and make sure that is the reason.

- [ ] **Step 3: Implement the pre-release version.** In `package.ps1`, before the `& $iscc` line:

```powershell
    # Windows' version info takes numbers only: "0.2.0-beta.1" → "0.2.0".
    $fileVersion = $Version -replace '[-+].*$', ''
```

and add `"/DFileVersion=$fileVersion"` to the `& $iscc` arguments, after `"/DVersion=$Version"`.

In `snapline.iss`, after the `#ifndef Source … #endif` block:

```
#ifndef FileVersion
  #define FileVersion Version
#endif
```

and change `VersionInfoVersion={#Version}` to `VersionInfoVersion={#FileVersion}`.

- [ ] **Step 4: Replace `windows-release.yml`** with:

```yaml
name: windows-release

# A `windows-v<version>` tag builds Snapline for Windows and creates a DRAFT release; Barayuda checks it and
# publishes it. Signing runs only once the AZURE_SIGNING_* repository variables exist (see docs/releasing-windows.md).
on:
  push:
    tags: ["windows-v*"]

permissions:
  contents: read

jobs:
  build:
    runs-on: windows-latest
    defaults:
      run:
        working-directory: windows-linux
        shell: pwsh
    outputs:
      version: ${{ steps.version.outputs.version }}
      signed: ${{ steps.version.outputs.signed }}
    steps:
      - uses: actions/checkout@v7
      - run: rustup toolchain install
      - id: version
        name: Check the tag against the crate version
        env:
          TAG: ${{ github.ref_name }}
          SIGNING_ACCOUNT: ${{ vars.AZURE_SIGNING_ACCOUNT }}
        run: |
          $tag = $env:TAG -replace '^windows-v', ''
          $crate = (cargo pkgid -p snapline) -replace '.*[#@]', ''
          if ($tag -ne $crate) { throw "Tag says $tag but snapline's Cargo.toml says $crate" }
          "version=$crate" >> $env:GITHUB_OUTPUT
          "signed=$(-not [string]::IsNullOrEmpty($env:SIGNING_ACCOUNT))".ToLower() >> $env:GITHUB_OUTPUT
      - run: cargo fmt --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - run: cargo build --release -p snapline
      - name: Build the installer, zip and checksums
        run: ./installer/package.ps1 -Version ${{ steps.version.outputs.version }}
      - uses: actions/upload-artifact@v7
        with:
          name: snapline-windows-release
          path: windows-linux/target/dist/*
          if-no-files-found: error
      - uses: actions/upload-artifact@v7
        with:
          name: snapline-exe
          path: windows-linux/target/release/snapline.exe
          if-no-files-found: error

  # Only this job can ask for an Azure token, and it builds nothing with cargo, so no dependency's build script
  # ever runs while a token is available.
  sign:
    needs: build
    if: needs.build.outputs.signed == 'true'
    runs-on: windows-latest
    # Azure trusts this environment (an exact federated-credential subject), not the tag name, which it cannot match.
    environment: release
    permissions:
      contents: read
      id-token: write
    defaults:
      run:
        working-directory: windows-linux
        shell: pwsh
    steps:
      - uses: actions/checkout@v7
      - uses: actions/download-artifact@v8
        with:
          name: snapline-exe
          path: windows-linux/target/release
      - name: Sign in to Azure
        uses: azure/login@v3
        with:
          client-id: ${{ secrets.AZURE_CLIENT_ID }}
          tenant-id: ${{ secrets.AZURE_TENANT_ID }}
          subscription-id: ${{ secrets.AZURE_SUBSCRIPTION_ID }}
      - name: Sign snapline.exe
        uses: azure/artifact-signing-action@v2
        with:
          endpoint: ${{ vars.AZURE_SIGNING_ENDPOINT }}
          signing-account-name: ${{ vars.AZURE_SIGNING_ACCOUNT }}
          certificate-profile-name: ${{ vars.AZURE_SIGNING_PROFILE }}
          files: ${{ github.workspace }}\windows-linux\target\release\snapline.exe
          file-digest: SHA256
          timestamp-digest: SHA256
      - name: Build the installer and zip around the signed exe
        run: ./installer/package.ps1 -Version ${{ needs.build.outputs.version }}
      - name: Sign the installer
        uses: azure/artifact-signing-action@v2
        with:
          endpoint: ${{ vars.AZURE_SIGNING_ENDPOINT }}
          signing-account-name: ${{ vars.AZURE_SIGNING_ACCOUNT }}
          certificate-profile-name: ${{ vars.AZURE_SIGNING_PROFILE }}
          files: ${{ github.workspace }}\windows-linux\target\dist\Snapline-Setup-${{ needs.build.outputs.version }}.exe
          file-digest: SHA256
          timestamp-digest: SHA256
      - name: Checksums of the signed files
        run: ./installer/package.ps1 -Version ${{ needs.build.outputs.version }} -SumsOnly
      - uses: actions/upload-artifact@v7
        with:
          name: snapline-windows-release
          path: windows-linux/target/dist/*
          if-no-files-found: error
          overwrite: true

  release:
    needs: [build, sign]
    # Runs after signing, or when signing was skipped (not set up); never after a failed or cancelled run.
    if: ${{ !cancelled() && needs.build.result == 'success' && needs.sign.result != 'failure' }}
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v8
        with:
          name: snapline-windows-release
          path: dist
      - name: Create the draft release
        env:
          GH_TOKEN: ${{ github.token }}
          VERSION: ${{ needs.build.outputs.version }}
          SIGNED: ${{ needs.build.outputs.signed }}
        run: |
          {
            echo "Download **Snapline-Setup-$VERSION.exe** and run it (no admin rights needed), or use the portable zip."
            echo
            if [ "$SIGNED" != "true" ]; then
              echo "This build is not signed yet, so Windows SmartScreen asks first: choose **More info**, then **Run anyway**."
              echo
            fi
            echo "SHA-256:"
            echo
            echo '```'
            cat dist/SHA256SUMS.txt
            echo '```'
          } > notes.md
          gh release create "$GITHUB_REF_NAME" --repo "$GITHUB_REPOSITORY" --draft --verify-tag \
            --title "Snapline for Windows $VERSION" --notes-file notes.md dist/*
```

Review it line by line against Review Focus 5:
- `sign` is skipped when `signed` is `false`;
- `release` then sees `needs.sign.result == 'skipped'` and runs;
- a failed `sign` stops `release`;
- `id-token` is only on `sign`;
- no `${{ }}` that comes from a tag or a variable is pasted inside a `run:` script (`github.ref_name` and `vars.*` go through `env:`);
- `steps.version.outputs.version` was checked against `Cargo.toml` before use.

Check that the YAML parses: `ruby -ryaml -e 'YAML.load_file(".github/workflows/windows-release.yml"); puts "ok"'`.

- [ ] **Step 5: Release guide.** In `docs/releasing-windows.md`, "Cut a release":
- step 2 becomes "Pull `main` (`git switch main && git pull`), then tag the merged commit and push the tag. The tag must match the version, or the workflow stops before building." Keep the code block.
- add after step 4:

  "If the run fails, or you need to build the same version again: delete the draft release if one was made, then delete the tag (`git push --delete origin windows-v0.2.0` and `git tag -d windows-v0.2.0`). Fix the problem on `main`, then tag again. `gh release create` stops if a release for that tag already exists."
- In "Turn on code signing later", step 2: "…open **release** (the first signed release run creates it)…". Add one sentence after the list: "Signing runs in its own job, which builds nothing with cargo, so only Azure's own steps can ask for the token."

In `docs/windows-linux.md`, add these decision rows:

```
| Phase 5c | Proxy from Windows' manual setting and `HTTPS_PROXY`, certificates from the Windows store (no PAC/WPAD); banner says macOS and Windows; README for both; the recorder, Print/Edit/Share, tray and scan, and release minors | Fix what would hurt the first release. Barayuda, 31 Oct 2026 |
```

Under Phases, item 5, add "   - 5c: uploads through a proxy and the Windows certificate store, the minors users notice, safer release workflow, README and banner for both platforms. Plan: [docs/plans/2026-10-31-windows-polish.md](plans/2026-10-31-windows-polish.md)."

- [ ] **Step 6: Commit, push, read CI**

```bash
ruby -ryaml -e 'YAML.load_file(".github/workflows/windows-release.yml"); YAML.load_file(".github/workflows/windows-linux.yml"); puts "yaml ok"' && git add .github windows-linux/installer docs && git commit -m "Sign releases in their own job, package pre-release versions, fill gaps in the release guide" && git log --format='%h %G?' -1 && git push
```

Expected: CI passes, and "Package a pre-release version" prints `pre-release packaging: ok`.

---

### Task 7: README and banner for macOS and Windows

**Files:**
- Modify: `scripts/brand/make-brand.swift` (label; also write `site/assets/og.png`)
- Modify (generated): `Assets/banner.png`, `site/assets/og.png`
- Modify: `README.md`

**Interfaces:**
- Consumes: `drawBanner(_ c: CGContext, w: CGFloat, h: CGFloat)`, `context(_:_:)`, `writePNG(_:_:)` in `make-brand.swift`.
- Produces: nothing used by code.

- [ ] **Step 1: The failing check**

Run: `grep -q "macOS  ·  WINDOWS" scripts/brand/make-brand.swift && grep -q "site/assets/og.png" scripts/brand/make-brand.swift && ! grep -q "come next" README.md && echo "test result: ok. branding"`
Expected: no output (the check fails).

- [ ] **Step 2: The script.** In `drawBanner`, the label line becomes:

```swift
    let label = "macOS  ·  WINDOWS  ·  ON-DEVICE PRIVACY"
```

At the end of the main section, before the `print`:

```swift
// The website's social preview (og:image): the same banner at half size.
let og = context(bw / 2, bh / 2)
og.scaleBy(x: 0.5, y: 0.5)
drawBanner(og, w: CGFloat(bw), h: CGFloat(bh))
writePNG(og, "\(root)/site/assets/og.png")
```

and the `print` becomes `print("Wrote Assets/AppIcon.icon, Assets/icon-1024.png, Assets/banner.png, site/assets/og.png")`.

- [ ] **Step 3: Generate.** From the repo root: `swift scripts/brand/make-brand.swift && git status --short Assets site`

Expected:
- `Assets/banner.png` and `site/assets/og.png` modified.
- If `Assets/AppIcon.icon` or `Assets/icon-1024.png` also show as modified (the icon did not change, only its bytes), restore them with `git checkout -- Assets/AppIcon.icon Assets/icon-1024.png` and ledger it.
- Open both PNGs and look:
  - the label reads "macOS · WINDOWS · ON-DEVICE PRIVACY", inside its corner ticks;
  - nothing is cut off;
  - `og.png` is 1280 × 400 (`sips -g pixelWidth -g pixelHeight site/assets/og.png`).

- [ ] **Step 4: README.** Replace everything from the line after the banner up to (not including) `## Build and run` with:

```markdown
A screenshot tool for the macOS menu bar and the Windows tray, modelled on Lightshot. Select an area, draw on it,
then copy, save, print, edit, share, upload (S3 or Google Drive) or search for similar images. Sensitive data is
hidden automatically on every capture.

- **macOS:** Swift 6.4, AppKit, ScreenCaptureKit, Vision, no third-party dependencies. Apple Silicon, macOS 15 or newer.
- **Windows:** Rust, with Windows' own text recognition and face detection. Windows 10 (1809) or 11, x64.
  [Install it](#windows-preview).
- **Linux:** next, from the same Rust code ([plan](docs/windows-linux.md)).
- Copy the text out of any screenshot, read on your computer.
- Written from scratch: Snapline shares no code or artwork with Lightshot.

## Privacy

Snapline is private by design. It has no servers, no accounts of its own, no analytics and no tracking.

- **Everything runs on your computer.** Capturing, drawing, auto-hide and Copy Text use the system's own frameworks
  on this device: ScreenCaptureKit and Vision on macOS, Windows OCR and face detection on Windows. No screenshot and
  no recognised text is sent to Snapline or to any third party to be read, analysed or stored.
- **An image leaves your computer only when you ask it to:**
  - **Upload** sends it to the destination you set up: your own S3 bucket or your own Google Drive.
  - **Search** uploads it there first, then opens Google Lens or TinEye in your browser with that link.
  - **Email** (macOS) hands the file to your mail app; **Share** (Windows) hands it to the app you pick.
- **Secrets stay in the system's keystore** (the macOS Keychain or Windows Credential Manager): bucket keys and the
  Google sign-in token. Google sign-in runs in your browser; Snapline only receives the result on a local address
  (127.0.0.1).

You can check this in the code: all network calls are in `macOS/Sources/Snapline/Uploaders.swift` and
`GoogleAuth.swift` on macOS, and in `windows-linux/crates/snapline-upload/` on Windows.

## macOS

```

Change the headings that follow:
- `## Build and run` → `### Build and run`;
- `### Keep permissions across rebuilds` → `#### Keep permissions across rebuilds`;
- `## Use` → `### Use`;
- `### Auto-hide sensitive data` → `#### Auto-hide sensitive data`;
- any other `###` inside "Use" → `####`.

Move the whole `## Windows (preview)` section so it sits right after the macOS section and before `## Upload setup`. In it:
- delete the sentence "An installer and code signing come next; keyboard-symbol repair in Copy Text comes later.";
- put "Keyboard-symbol repair in Copy Text comes later." in its place;
- add this bullet after the Upload bullet:

```markdown
- Networks with a proxy: uploads use the proxy set in Settings → Network & internet → Proxy → "Use a proxy server"
  (or `HTTPS_PROXY`), and trust the certificates Windows trusts. Automatic proxy setup (a setup script or "Automatically
  detect settings") and proxies that ask for a sign-in are not supported yet.
```

Under `## Upload setup`, add after its first paragraph: "On Windows the same fields are in the tray menu → **Settings…** → Upload, and keys are kept in Windows Credential Manager."

In "Project layout", the `Assets/` line becomes `Assets/ AppIcon.icon (layered app icon), icon-1024.png, banner.png (generated; also site/assets/og.png)`. Keep the column alignment.

- [ ] **Step 5: Run the check**

Run: the command from Step 1.
Expected: `test result: ok. branding`. Then read the README top to bottom once:
- every `#windows-preview` link still matches its heading;
- no "your Mac" is left in shared text (Mac-only sections may keep it).

- [ ] **Step 6: Commit, push**

```bash
git add scripts/brand/make-brand.swift Assets/banner.png site/assets/og.png README.md && git commit -m "README and banner for macOS and Windows" && git log --format='%h %G?' -1 && git push
```

The website change goes live only when Barayuda pushes `main:website`.

- [ ] **Step 7: Barayuda checks it on Windows** (pass or fail, with a screenshot for any fail). Add these to the PR's Windows checklist:
  1. **Proxy:** set a manual proxy in Settings → Network & internet → Proxy (a real one at work, or none to check the direct path still works). Upload to Google Drive and to S3: both succeed. With a proxy that needs a sign-in, the popup says the proxy asks Snapline to sign in.
  2. **Shortcut recorder:** Settings → General → Record, press Ctrl+C. It says other apps need it and keeps recording. Then Ctrl+Shift+2 saves, and Esc while recording keeps the old shortcut.
  3. **Tray:** pin the tray icon next to the clock, change "Clicking the tray icon" in General, and save. The icon stays where it was, and the new click behaviour works at once.
  4. **Print and Edit:** with no app set for PNG printing (if you can arrange it), Print explains where to set one. Press Print and Edit quickly one after the other: both open their own file.
  5. **Share fallback:** with Share unavailable, Explorer opens with the file selected, also when your user name has a space in it.
