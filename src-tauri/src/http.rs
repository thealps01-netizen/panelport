//! Small HTTP client used for health checks, discovery probes and favicons.
//! It only ever talks to panel URLs the user added or to ports on this computer.

use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// Answered with a status below 500 (redirects and login pages count as up).
    Up,
    /// Answered, but with a server error (5xx).
    Error,
    /// Answered, but not with a web page (for example an API answering 401 in plain text).
    NotPage,
    /// Answered with a client error (4xx other than a login wall), for example HTTP 400.
    ClientError,
    /// No HTTP answer at all.
    Down,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub health: Health,
    pub status: Option<u16>,
    /// Short technical reason for the log ("connection refused", "timeout", ...).
    pub detail: String,
    /// Time until the response headers arrived (only when there was an answer).
    pub latency_ms: Option<u32>,
}

pub struct Page {
    pub status: u16,
    pub content_type: String,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

/// True when the answer is an HTML document: an HTML content type, or no usable content type
/// and a body that starts like an HTML document.
pub fn is_html_document(content_type: &str, body: &[u8]) -> bool {
    let ct = content_type.trim().to_ascii_lowercase();
    if ct.starts_with("text/html") || ct.starts_with("application/xhtml+xml") {
        return true;
    }
    if !(ct.is_empty() || ct.starts_with("application/octet-stream")) {
        return false;
    }
    let body = body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(body);
    let start = body
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(body.len());
    let head: Vec<u8> = body[start..body.len().min(start + 15)].to_ascii_lowercase();
    head.starts_with(b"<!doctype html") || head.starts_with(b"<html")
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .max_redirects(0)
        .user_agent(concat!("Panelport/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn describe(e: &ureq::Error) -> String {
    match e {
        ureq::Error::Timeout(_) => "timeout".into(),
        ureq::Error::HostNotFound => "host not found".into(),
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::ConnectionRefused => {
            "connection refused".into()
        }
        other => other.to_string(),
    }
}

/// Health of one check. `expect_page` is true when the checked URL is the page the panel window
/// shows (health path `/`); then only an HTML answer counts as up. A configured health path may
/// answer anything below 400 (Arcane's `/api/health` answers JSON). A 4xx answer counts as up
/// only when it is an HTML page (a login or error page of a web UI); a non-HTML 4xx is an API
/// that wants a key, so the panel is "not a web page".
pub fn check(url: &Url, timeout: Duration, expect_page: bool) -> Check {
    let started = std::time::Instant::now();
    match agent(timeout).get(url.as_str()).call() {
        Ok(mut resp) => {
            let latency_ms = Some(started.elapsed().as_millis().min(u32::MAX as u128) as u32);
            let status = resp.status().as_u16();
            let content_type = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let needs_body = (400..500).contains(&status) || (expect_page && status < 300);
            let html = needs_body && {
                let body = resp
                    .body_mut()
                    .with_config()
                    .limit(64 * 1024)
                    .read_to_vec()
                    .unwrap_or_default();
                is_html_document(&content_type, &body)
            };
            // DECISIONS.md 28 and 47: only 2xx/3xx are up; an HTML 401/403 is a login wall (up);
            // a non-HTML 401/403 is not a web page; any other 4xx is a client error.
            let health = match status {
                500.. => Health::Error,
                300..=399 => Health::Up,
                401 | 403 if html => Health::Up,
                401 | 403 => Health::NotPage,
                400..=499 => Health::ClientError,
                200..=299 if expect_page && !html => Health::NotPage,
                _ => Health::Up,
            };
            let detail = if health == Health::NotPage {
                let ct = content_type.split(';').next().unwrap_or("").trim();
                let ct = if ct.is_empty() { "no content type" } else { ct };
                format!("HTTP {status}, {ct}")
            } else {
                format!("HTTP {status}")
            };
            Check {
                health,
                status: Some(status),
                detail,
                latency_ms,
            }
        }
        Err(e) => Check {
            health: Health::Down,
            status: None,
            detail: describe(&e),
            latency_ms: None,
        },
    }
}

/// GET with a body size cap. `Err` holds a short reason for the log.
pub fn get(url: &Url, timeout: Duration, limit: u64) -> Result<Page, String> {
    let mut resp = agent(timeout)
        .get(url.as_str())
        .call()
        .map_err(|e| describe(&e))?;
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let body = resp
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .unwrap_or_default();
    Ok(Page {
        status,
        content_type,
        location,
        body,
    })
}

/// Text of the first `<title>` element, whitespace collapsed, at most 60 characters.
pub fn html_title(body: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(body);
    let lower = text.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = start + lower[start..].find('>')? + 1;
    let close = open_end + lower[open_end..].find("</title")?;
    let raw = &text[open_end..close];
    let title = decode_entities(&raw.split_whitespace().collect::<Vec<_>>().join(" "));
    if title.is_empty() {
        return None;
    }
    Some(title.chars().take(60).collect())
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// `href` values of `<link rel="...icon...">` tags, in document order.
pub fn icon_links(body: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(body);
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<link") {
        let start = pos + i;
        let Some(len) = lower[start..].find('>') else {
            break;
        };
        let tag_lower = &lower[start..start + len];
        let tag = &text[start..start + len];
        if let Some(rel) = attr(tag_lower, tag_lower, "rel") {
            if rel
                .split_whitespace()
                .any(|r| r == "icon" || r == "apple-touch-icon")
            {
                if let Some(href) = attr(tag_lower, tag, "href") {
                    out.push(href);
                }
            }
        }
        pos = start + len;
    }
    out
}

fn attr(tag_lower: &str, tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let mut search = 0;
    while let Some(i) = tag_lower[search..].find(&needle) {
        let at = search + i;
        let before_ok = at == 0 || tag_lower.as_bytes()[at - 1].is_ascii_whitespace();
        let vstart = at + needle.len();
        if before_ok {
            let rest = &tag[vstart..];
            let value = match rest.chars().next() {
                Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or(""),
                _ => rest
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or(""),
            };
            return Some(value.to_string());
        }
        search = vstart;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_extraction() {
        assert_eq!(
            html_title(b"<html><head><TITLE>\n  Arcane &amp; Co </TITLE>"),
            Some("Arcane & Co".into())
        );
        assert_eq!(html_title(b"<title data-x=1>Hi</title>"), Some("Hi".into()));
        assert_eq!(html_title(b"<title></title>"), None);
        assert_eq!(html_title(b"no title"), None);
    }

    #[test]
    fn icon_link_extraction() {
        let html = br#"<link rel="stylesheet" href="/a.css"><link rel="icon" type="image/png" href="/favicon.png"/>
            <LINK REL='shortcut icon' HREF='/f.ico'><link href="/t.png" rel="apple-touch-icon">"#;
        assert_eq!(icon_links(html), vec!["/favicon.png", "/f.ico", "/t.png"]);
    }

    #[test]
    fn refused_port_is_down() {
        // Bind and drop a listener to get a port that is very likely closed.
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let c = check(
            &Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap(),
            Duration::from_secs(2),
            true,
        );
        assert_eq!(c.health, Health::Down);
    }

    #[test]
    fn local_server_is_up_and_5xx_is_error() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for (i, s) in l.incoming().take(2).enumerate() {
                let mut s = s.unwrap();
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let status = if i == 0 {
                    "302 Found\r\nLocation: /login"
                } else {
                    "503 Service Unavailable"
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        });
        let url = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let a = check(&url, Duration::from_secs(3), true);
        assert_eq!((a.health, a.status), (Health::Up, Some(302)));
        let b = check(&url, Duration::from_secs(3), true);
        assert_eq!((b.health, b.status), (Health::Error, Some(503)));
    }

    fn check_once(resp: String, expect_page: bool) -> Check {
        let port = crate::testutil::serve(vec![resp]);
        let url = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        check(&url, Duration::from_secs(3), expect_page)
    }

    #[test]
    fn api_401_in_plain_text_is_not_a_page() {
        use crate::testutil::response;
        // What the cua daemon answers on its API port.
        let r = response(
            "401 Unauthorized",
            "text/plain; charset=utf-8",
            "",
            "missing or wrong daemon token",
        );
        let c = check_once(r, true);
        assert_eq!((c.health, c.status), (Health::NotPage, Some(401)));
    }

    #[test]
    fn json_403_is_not_a_page_even_on_a_custom_health_path() {
        use crate::testutil::response;
        let r = response(
            "403 Forbidden",
            "application/json",
            "",
            r#"{"error":"forbidden"}"#,
        );
        assert_eq!(check_once(r, false).health, Health::NotPage);
    }

    #[test]
    fn html_login_page_with_401_is_up() {
        use crate::testutil::response;
        let r = response(
            "401 Unauthorized",
            "text/html",
            "",
            "<!doctype html><title>Sign in</title>",
        );
        assert_eq!(check_once(r, true).health, Health::Up);
    }

    #[test]
    fn json_200_is_up_on_a_health_path_but_not_as_the_page() {
        use crate::testutil::response;
        let r = || response("200 OK", "application/json", "", r#"{"status":"UP"}"#);
        // Arcane's /api/health answers JSON: fine for a configured health path...
        assert_eq!(check_once(r(), false).health, Health::Up);
        // ...but a panel whose start page is JSON is not a web page.
        assert_eq!(check_once(r(), true).health, Health::NotPage);
    }

    #[test]
    fn html_4xx_errors_are_client_errors_not_up() {
        use crate::testutil::response;
        for status in [
            "400 Bad Request",
            "405 Method Not Allowed",
            "429 Too Many Requests",
        ] {
            let code: u16 = status[..3].parse().unwrap();
            let page = || {
                response(
                    status,
                    "text/html",
                    "",
                    "<!doctype html><title>Error</title><h1>Error</h1>",
                )
            };
            for expect_page in [true, false] {
                let c = check_once(page(), expect_page);
                assert_eq!(
                    (c.health.clone(), c.status, c.detail.as_str()),
                    (
                        Health::ClientError,
                        Some(code),
                        format!("HTTP {code}").as_str()
                    ),
                    "{status}, expect_page={expect_page}"
                );
            }
        }
    }

    #[test]
    fn non_html_4xx_other_than_401_403_is_a_client_error() {
        use crate::testutil::response;
        let r = response(
            "400 Bad Request",
            "application/json",
            "",
            r#"{"error":"bad"}"#,
        );
        assert_eq!(check_once(r, true).health, Health::ClientError);
    }

    #[test]
    fn html_login_walls_401_403_stay_up() {
        use crate::testutil::response;
        for status in ["401 Unauthorized", "403 Forbidden"] {
            let r = response(
                status,
                "text/html",
                "",
                "<!doctype html><title>Sign in</title>",
            );
            assert_eq!(check_once(r, true).health, Health::Up, "{status}");
        }
    }

    #[test]
    fn html_document_detection() {
        assert!(is_html_document("text/html; charset=utf-8", b""));
        assert!(is_html_document("application/xhtml+xml", b""));
        assert!(is_html_document("", b"\xEF\xBB\xBF  <!DOCTYPE html><html>"));
        assert!(is_html_document("", b"\n<html lang=en>"));
        assert!(!is_html_document("", b"missing or wrong daemon token"));
        assert!(!is_html_document(
            "text/plain",
            b"<html>looks like html</html>"
        ));
        assert!(!is_html_document("application/json", b"{}"));
    }
}
