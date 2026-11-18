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
    /// No HTTP answer at all.
    Down,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub health: Health,
    pub status: Option<u16>,
    /// Short technical reason for the log ("connection refused", "timeout", ...).
    pub detail: String,
}

pub struct Page {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
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

pub fn check(url: &Url, timeout: Duration) -> Check {
    match agent(timeout).get(url.as_str()).call() {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let health = if status >= 500 {
                Health::Error
            } else {
                Health::Up
            };
            Check {
                health,
                status: Some(status),
                detail: format!("HTTP {status}"),
            }
        }
        Err(e) => Check {
            health: Health::Down,
            status: None,
            detail: describe(&e),
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
    let body = resp
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .unwrap_or_default();
    Ok(Page {
        status,
        content_type,
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
        let a = check(&url, Duration::from_secs(3));
        assert_eq!((a.health, a.status), (Health::Up, Some(302)));
        let b = check(&url, Duration::from_secs(3));
        assert_eq!((b.health, b.status), (Health::Error, Some(503)));
    }
}
