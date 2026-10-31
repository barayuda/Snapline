//! The proxy Windows is set to use (Settings → Network & internet → Proxy → "Use a proxy server"). The app reads
//! the registry; this decides which proxy a URL goes through. Automatic setup (PAC scripts, WPAD) is not supported.

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

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    #[test]
    fn one_server_for_every_scheme() {
        let p = SystemProxy::from_registry(true, "proxy.corp:8080", "").unwrap();
        assert_eq!(
            p.for_url(&url("https://www.googleapis.com/upload")),
            Some("proxy.corp:8080")
        );
        assert_eq!(
            p.for_url(&url("http://minio.example:9000/b")),
            Some("proxy.corp:8080")
        );
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
        let p = SystemProxy::from_registry(
            true,
            "a:80",
            "<local>;*.corp.example;10.*;Storage.Internal",
        )
        .unwrap();
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
