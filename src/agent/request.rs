//! HTTP request-head parsing and caller authorisation for the local gateway.
//!
//! Deliberately free of external crates so it can be exercised in isolation:
//! this is the only thing standing between a web page in the operator's browser
//! and full control of every saved peer.
//!
//! Binding to `127.0.0.1` and checking the peer socket address is *not* enough.
//! A page on any site can `fetch('http://127.0.0.1:21120/exec', ...)`; the
//! browser connects from loopback, so the source-IP check passes and the
//! command runs before the browser blocks the attacker from reading the reply.
//! Three guards close that, cheapest first:
//!
//! 1. `Host` must name loopback on the port we bound — a DNS-rebinding page
//!    reaches us over loopback but still sends its own name here.
//! 2. No `Origin` header at all: every legitimate caller is a local process,
//!    never a browser, and browsers attach `Origin` to exactly the requests
//!    that would be dangerous.
//! 3. `Authorization: Bearer <token>` must match the per-user gateway token,
//!    which a web page cannot read and which stops any *other* local process
//!    from inheriting the operator's saved peer credentials.

/// Request heads larger than this are rejected before any allocation growth.
pub const MAX_HEADER: usize = 64 * 1024;

/// A parsed request head: the request line plus every header, in order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Head {
    pub method: String,
    pub path: String,
    headers: Vec<(String, String)>,
}

impl Head {
    /// First value for `name`, matched case-insensitively per RFC 9110.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn header_count(&self, name: &str) -> usize {
        self.headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .count()
    }

    /// Declared body length. Absent means no body; anything ambiguous is an
    /// error rather than a guess, so a body can never be split across what the
    /// guard inspected and what the router acts on.
    pub fn content_length(&self) -> Result<usize, String> {
        if self.header_count("content-length") > 1 {
            return Err("conflicting Content-Length headers".to_owned());
        }
        match self.header("content-length") {
            None => Ok(0),
            Some(v) => v
                .parse::<usize>()
                .map_err(|_| format!("invalid Content-Length: {v}")),
        }
    }
}

/// Parse the bytes before the blank line that ends a request head.
pub fn parse_head(bytes: &[u8]) -> Result<Head, String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");

    let mut parts = lines.next().unwrap_or_default().split_whitespace();
    let method = parts.next().unwrap_or_default().to_ascii_uppercase();
    let target = parts.next().unwrap_or_default();
    if method.is_empty() || target.is_empty() {
        return Err("malformed request line".to_owned());
    }
    let path = target.split('?').next().unwrap_or_default().to_owned();

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "malformed header line".to_owned())?;
        let name = name.trim();
        if name.is_empty() || name.len() != line[..line.find(':').unwrap_or(0)].len() {
            // A leading space means obsolete line folding, which would let a
            // header be read differently by us and by anything in front of us.
            return Err("malformed header name".to_owned());
        }
        headers.push((name.to_owned(), value.trim().to_owned()));
    }

    let head = Head {
        method,
        path,
        headers,
    };
    if head.header("transfer-encoding").is_some() {
        return Err("Transfer-Encoding is not supported".to_owned());
    }
    Ok(head)
}

/// Why a request was refused, and with which status.
#[derive(Debug, PartialEq, Eq)]
pub enum Denied {
    /// 403 — the request cannot have come from a trusted local caller.
    Forbidden(String),
    /// 401 — missing or wrong gateway token.
    Unauthorized(String),
}

impl Denied {
    pub fn status(&self) -> u16 {
        match self {
            Denied::Forbidden(_) => 403,
            Denied::Unauthorized(_) => 401,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Denied::Forbidden(m) | Denied::Unauthorized(m) => m,
        }
    }
}

/// Authorises callers of the local HTTP gateway.
pub struct Guard {
    port: u16,
    token: String,
}

impl Guard {
    pub fn new(port: u16, token: String) -> Self {
        Self { port, token }
    }

    pub fn check(&self, head: &Head) -> Result<(), Denied> {
        let host = head.header("host").unwrap_or_default();
        if !is_loopback_authority(host, self.port) {
            return Err(Denied::Forbidden(format!(
                "Host '{host}' is not 127.0.0.1:{} (possible DNS rebinding)",
                self.port
            )));
        }
        if let Some(origin) = head.header("origin") {
            return Err(Denied::Forbidden(format!(
                "requests from a web origin ({origin}) are never accepted"
            )));
        }
        // Fail closed: an unset token must deny everything, not everyone.
        if self.token.is_empty() {
            return Err(Denied::Unauthorized(
                "gateway token is not configured".to_owned(),
            ));
        }
        let presented = head
            .header("authorization")
            .and_then(bearer_value)
            .unwrap_or_default();
        if !ct_eq(presented, &self.token) {
            return Err(Denied::Unauthorized(
                "missing or invalid gateway token (send 'Authorization: Bearer <token>')"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

/// The credentials of an `Authorization: Bearer <token>` header. The scheme is
/// case-insensitive per RFC 9110.
fn bearer_value(value: &str) -> Option<&str> {
    let (scheme, token) = value.trim().split_once(char::is_whitespace)?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim_start())
}

/// Does this `Host` say the client believes it is talking to our loopback
/// socket? Rebinding pages send the attacker's own name, which is the point.
fn is_loopback_authority(host: &str, port: u16) -> bool {
    let host = host.trim();
    let (name, declared_port) = match host.strip_prefix('[') {
        // Bracketed IPv6 literal: [::1] or [::1]:21120.
        Some(rest) => match rest.split_once(']') {
            Some((name, "")) => (name, None),
            Some((name, tail)) => match tail.strip_prefix(':') {
                Some(p) => (name, Some(p)),
                None => return false,
            },
            None => return false,
        },
        None if host.contains(':') => match host.rsplit_once(':') {
            // An unbracketed IPv6 literal is malformed here, and splitting one
            // on the last colon would silently accept part of it as a name.
            Some((name, _)) if name.contains(':') => return false,
            Some((name, p)) => (name, Some(p)),
            None => return false,
        },
        None => (host, None),
    };

    let name_ok = name.eq_ignore_ascii_case("localhost") || name == "127.0.0.1" || name == "::1";
    // No port means the default 80, which only matches if that is where we bound.
    let port_ok = match declared_port {
        Some(p) => p.parse::<u16>() == Ok(port),
        None => port == 80,
    };
    name_ok && port_ok
}

/// Compare two secrets without leaking their contents through timing. Length
/// is not secret here — the token is a fixed-width hex string.
fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn head(raw: &str) -> Head {
        parse_head(raw.as_bytes()).expect("parse failed")
    }

    fn guard() -> Guard {
        Guard::new(21120, TOKEN.to_owned())
    }

    fn legit() -> String {
        format!(
            "POST /exec HTTP/1.1\r\nHost: 127.0.0.1:21120\r\nAuthorization: Bearer {TOKEN}\r\nContent-Length: 2"
        )
    }

    #[test]
    fn parses_request_line_and_headers() {
        let h = head("get /peers?x=1 HTTP/1.1\r\nHost: 127.0.0.1:21120\r\nX-A: 1");
        assert_eq!(h.method, "GET");
        assert_eq!(h.path, "/peers");
        assert_eq!(h.header("HOST"), Some("127.0.0.1:21120"));
        assert_eq!(h.header("x-a"), Some("1"));
        assert_eq!(h.header("missing"), None);
    }

    #[test]
    fn content_length_absent_conflicting_and_invalid() {
        assert_eq!(head("POST / HTTP/1.1\r\nHost: x").content_length(), Ok(0));
        assert_eq!(
            head("POST / HTTP/1.1\r\nContent-Length: 7").content_length(),
            Ok(7)
        );
        assert!(
            head("POST / HTTP/1.1\r\nContent-Length: 3\r\nContent-Length: 9")
                .content_length()
                .is_err()
        );
        assert!(head("POST / HTTP/1.1\r\nContent-Length: -1")
            .content_length()
            .is_err());
        // The old parser used `.unwrap_or(0)` here and silently treated a
        // malformed length as "no body".
        assert!(head("POST / HTTP/1.1\r\nContent-Length: 1e3")
            .content_length()
            .is_err());
    }

    #[test]
    fn rejects_malformed_heads() {
        assert!(parse_head(b"").is_err());
        assert!(parse_head(b"POST\r\nHost: x").is_err());
        assert!(parse_head(b"POST / HTTP/1.1\r\nnot-a-header").is_err());
        assert!(parse_head(b"POST / HTTP/1.1\r\n Host: x").is_err());
        assert!(parse_head(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked").is_err());
    }

    #[test]
    fn accepts_a_legitimate_local_caller() {
        assert_eq!(guard().check(&head(&legit())), Ok(()));
        // localhost and a bracketed IPv6 literal name the same socket.
        let h = head(&legit().replace("127.0.0.1:21120", "localhost:21120"));
        assert_eq!(guard().check(&h), Ok(()));
        let h = head(&legit().replace("127.0.0.1:21120", "[::1]:21120"));
        assert_eq!(guard().check(&h), Ok(()));
    }

    /// The regression this module exists for: a page on any site can reach the
    /// gateway from loopback, so the source-IP check passes.
    #[test]
    fn rejects_browser_csrf_from_loopback() {
        let h = head(&format!(
            "POST /exec HTTP/1.1\r\nHost: 127.0.0.1:21120\r\nOrigin: https://evil.example\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: text/plain"
        ));
        let denied = guard().check(&h).expect_err("CSRF request was accepted");
        assert_eq!(denied.status(), 403);
        assert!(denied.message().contains("evil.example"));
    }

    /// DNS rebinding: same-origin to the browser, loopback to us, attacker's
    /// name in Host.
    #[test]
    fn rejects_dns_rebinding_host() {
        for host in [
            "evil.example:21120",
            "evil.example",
            "127.0.0.1.evil.example:21120",
            "localhost.evil.example:21120",
            // An unbracketed IPv6 literal must not be split on its last colon.
            "::1:21120",
        ] {
            let h = head(&legit().replace("127.0.0.1:21120", host));
            let denied = guard()
                .check(&h)
                .expect_err(&format!("{host} was accepted"));
            assert_eq!(denied.status(), 403, "{host}");
        }
    }

    #[test]
    fn rejects_wrong_or_missing_port_in_host() {
        for host in ["127.0.0.1:80", "127.0.0.1", "localhost:1", "127.0.0.1:"] {
            let h = head(&legit().replace("127.0.0.1:21120", host));
            assert!(guard().check(&h).is_err(), "{host} was accepted");
        }
    }

    #[test]
    fn rejects_missing_host() {
        let h = head(&format!(
            "POST /exec HTTP/1.1\r\nAuthorization: Bearer {TOKEN}"
        ));
        assert_eq!(guard().check(&h).unwrap_err().status(), 403);
    }

    #[test]
    fn rejects_missing_or_wrong_token() {
        let no_auth = head("POST /exec HTTP/1.1\r\nHost: 127.0.0.1:21120");
        assert_eq!(guard().check(&no_auth).unwrap_err().status(), 401);

        for auth in [
            "Bearer ",
            "Bearer wrong",
            // A near miss must not pass: the compare is over the whole string.
            "Bearer 0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef",
            "Basic 0123456789abcdef0123456789abcdef",
        ] {
            let h = head(&format!(
                "POST /exec HTTP/1.1\r\nHost: 127.0.0.1:21120\r\nAuthorization: {auth}"
            ));
            assert_eq!(
                guard().check(&h).unwrap_err().status(),
                401,
                "'{auth}' was accepted"
            );
        }
    }

    #[test]
    fn accepts_case_insensitive_bearer_scheme() {
        let h = head(&format!(
            "POST /exec HTTP/1.1\r\nhost: 127.0.0.1:21120\r\nauthorization: bearer {TOKEN}"
        ));
        assert_eq!(guard().check(&h), Ok(()));
    }

    /// An empty configured token must lock the gateway, not open it.
    #[test]
    fn empty_token_denies_everything() {
        let g = Guard::new(21120, String::new());
        let h = head("POST /exec HTTP/1.1\r\nHost: 127.0.0.1:21120\r\nAuthorization: Bearer ");
        assert_eq!(g.check(&h).unwrap_err().status(), 401);
    }

    #[test]
    fn host_is_checked_before_the_token() {
        // Otherwise a rebinding page learns whether a token guess was right.
        let h = head("GET /peers HTTP/1.1\r\nHost: evil.example:21120");
        assert_eq!(guard().check(&h).unwrap_err().status(), 403);
    }
}
