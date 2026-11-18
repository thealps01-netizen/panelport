//! Finds web panels on this computer: ports published by Docker containers plus local
//! listening TCP ports that answer HTTP. Read-only: it lists, inspects and sends one GET per port.

use crate::docker::{self, Container};
use crate::http;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Candidate {
    pub url: String,
    pub name: String,
    pub port: u16,
    /// Container name when Docker publishes this port.
    pub container: Option<String>,
    pub title: Option<String>,
    /// Pre-ticked in the list (web pages served by a Docker container).
    pub recommended: bool,
    /// Already one of the user's panels (listed, not offered again).
    pub known: bool,
}

/// A port that answered HTTP, but not with a web page (not offered as a panel).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Skipped {
    pub port: u16,
    /// Short reason: the HTTP status ("401"), "JSON", or the content type.
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Found {
    pub candidates: Vec<Candidate>,
    pub skipped: Vec<Skipped>,
}

/// Ports where nothing web-like lives (or only Windows system HTTP endpoints such as WinRM);
/// probing them only wastes time.
const SKIP_PORTS: &[u16] = &[
    22, 25, 53, 110, 135, 139, 143, 445, 1433, 1521, 3306, 3389, 5040, 5353, 5432, 5672, 5985,
    5986, 6379, 9200, 11211, 27017, 47001,
];
const MAX_PROBES: usize = 400;

/// Local TCP ports in LISTEN state on loopback or all interfaces.
pub fn listening_ports() -> Vec<u16> {
    let mut ports = platform_listening_ports();
    ports.retain(|p| *p != 0 && !SKIP_PORTS.contains(p));
    ports.sort_unstable();
    ports.dedup();
    ports
}

#[cfg(windows)]
fn platform_listening_ports() -> Vec<u16> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Some(sysroot) = std::env::var_os("SystemRoot") else {
        log::warn!("SystemRoot not set; skipping port scan");
        return Vec::new();
    };
    let netstat = std::path::PathBuf::from(sysroot)
        .join("System32")
        .join("NETSTAT.EXE");
    let mut ports = Vec::new();
    for proto in ["TCP", "TCPv6"] {
        match std::process::Command::new(&netstat)
            .args(["-a", "-n", "-p", proto])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            Ok(o) => ports.extend(parse_netstat(&String::from_utf8_lossy(&o.stdout))),
            Err(e) => log::warn!("netstat {proto} failed: {e}"),
        }
    }
    ports
}

/// netstat state names are translated on non-English Windows, so a listening socket is
/// recognised by its foreign address (`0.0.0.0:0` / `[::]:0`) instead.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_netstat(text: &str) -> Vec<u16> {
    let mut out = Vec::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 3 || !cols[0].eq_ignore_ascii_case("tcp") {
            continue;
        }
        let (local, foreign) = (cols[1], cols[2]);
        if foreign != "0.0.0.0:0" && foreign != "[::]:0" {
            continue;
        }
        let Some((host, port)) = local.rsplit_once(':') else {
            continue;
        };
        if !matches!(host, "0.0.0.0" | "127.0.0.1" | "[::]" | "[::1]") {
            continue;
        }
        if let Ok(p) = port.parse::<u16>() {
            out.push(p);
        }
    }
    out
}

#[cfg(not(windows))]
fn platform_listening_ports() -> Vec<u16> {
    let mut out = Vec::new();
    for f in ["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(text) = std::fs::read_to_string(f) {
            out.extend(parse_proc_net_tcp(&text));
        }
    }
    out
}

#[cfg(not(windows))]
pub fn parse_proc_net_tcp(text: &str) -> Vec<u16> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 || cols[3] != "0A" {
            continue;
        }
        let Some((addr, port)) = cols[1].split_once(':') else {
            continue;
        };
        let any_or_loop = matches!(
            addr,
            "00000000"
                | "0100007F"
                | "00000000000000000000000000000000"
                | "00000000000000000000000001000000"
        );
        if !any_or_loop {
            continue;
        }
        if let Ok(p) = u16::from_str_radix(port, 16) {
            out.push(p);
        }
    }
    out
}

struct Probe {
    title: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Page(Option<String>),
    /// Answered HTTP, but not with a web page.
    NotPage(String),
    /// No HTTP answer, a broken server, a 404 start page, or a redirect to another site.
    Nothing,
}

#[cfg(test)]
fn probe(port: u16) -> Option<Probe> {
    match classify(port) {
        Outcome::Page(title) => Some(Probe { title }),
        _ => None,
    }
}

/// Offers a port only when its start page is an HTML document. Same-origin redirects (a web UI
/// sending `/` to `/login`) are followed up to 3 times; redirects elsewhere are not.
fn classify(port: u16) -> Outcome {
    let Ok(mut url) = Url::parse(&format!("http://localhost:{port}/")) else {
        return Outcome::Nothing;
    };
    for _ in 0..4 {
        let Ok(page) = http::get(&url, Duration::from_millis(1500), 128 * 1024) else {
            return Outcome::Nothing;
        };
        if (300..400).contains(&page.status) {
            let next = page.location.as_deref().and_then(|l| url.join(l).ok());
            match next {
                Some(next) if crate::urls::same_origin(&next, &url) => {
                    url = next;
                    continue;
                }
                _ => return Outcome::Nothing,
            }
        }
        // 404 at the start page is an API or system endpoint, 5xx is broken; anything else
        // must be an actual web page (APIs answering 401/403 or JSON are not panels).
        if page.status >= 500 || page.status == 404 {
            return Outcome::Nothing;
        }
        if http::is_html_document(&page.content_type, &page.body) {
            return Outcome::Page(http::html_title(&page.body));
        }
        let ct = page
            .content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let reason = if page.status >= 400 {
            page.status.to_string()
        } else if ct.contains("json") {
            "JSON".to_string()
        } else if ct.is_empty() {
            "?".to_string()
        } else {
            ct
        };
        return Outcome::NotPage(reason);
    }
    Outcome::Nothing
}

fn pretty(name: &str) -> String {
    let mut s: String = name.replace(['-', '_'], " ");
    if let Some(first) = s.get(0..1) {
        s = first.to_uppercase() + &s[1..];
    }
    s
}

/// Runs the full discovery. `known` are URLs already configured (listed with `known: true`).
pub fn discover(known: &[Url]) -> Found {
    let cli = docker::find_cli();
    let containers: Vec<Container> = match &cli {
        Some(cli) if docker::engine(Some(cli)) == docker::Engine::Running => {
            docker::containers(cli).unwrap_or_else(|e| {
                log::warn!("discovery: {e}");
                Vec::new()
            })
        }
        _ => Vec::new(),
    };
    let mut by_port: BTreeMap<u16, Option<&Container>> = BTreeMap::new();
    for c in containers.iter().filter(|c| c.running) {
        for p in &c.ports {
            by_port.insert(*p, Some(c));
        }
    }
    for p in listening_ports() {
        by_port.entry(p).or_insert(None);
    }
    let known_ports: Vec<u16> = known
        .iter()
        .filter(|u| crate::urls::is_loopback(u))
        .filter_map(|u| u.port_or_known_default())
        .collect();
    let ports: Vec<(u16, Option<&Container>)> = by_port.into_iter().take(MAX_PROBES).collect();

    let mut found = Vec::new();
    let mut skipped = Vec::new();
    for chunk in ports.chunks(32) {
        let results: Vec<(u16, Option<&Container>, Outcome)> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(p, c)| {
                    let p = *p;
                    (p, *c, s.spawn(move || classify(p)))
                })
                .collect();
            handles
                .into_iter()
                .map(|(p, c, h)| (p, c, h.join().unwrap_or(Outcome::Nothing)))
                .collect()
        });
        for (port, container, outcome) in results {
            let title = match outcome {
                Outcome::Page(title) => title,
                Outcome::NotPage(reason) => {
                    skipped.push(Skipped { port, reason });
                    continue;
                }
                Outcome::Nothing => continue,
            };
            let probe = Probe { title };
            let known = known_ports.contains(&port);
            let cname = container.map(|c| c.service.clone().unwrap_or_else(|| c.name.clone()));
            let name = match (&probe.title, &cname) {
                (Some(t), _) if t.chars().count() <= 40 => t.clone(),
                (_, Some(c)) => pretty(c),
                _ => format!("localhost:{port}"),
            };
            found.push(Candidate {
                url: format!("http://localhost:{port}/"),
                name,
                port,
                container: container.map(|c| c.name.clone()),
                title: probe.title,
                recommended: container.is_some() && !known,
                known,
            });
        }
    }
    found.sort_by(|a, b| {
        a.known
            .cmp(&b.known)
            .then(b.recommended.cmp(&a.recommended))
            .then(a.port.cmp(&b.port))
    });
    skipped.sort_by_key(|s| s.port);
    Found {
        candidates: found,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netstat_parsing_is_language_independent() {
        let text = "
Etkin Bağlantılar

  Proto  Yerel Adres            Yabancı Adres          Durum
  TCP    0.0.0.0:135            0.0.0.0:0              DİNLEME
  TCP    127.0.0.1:3552         0.0.0.0:0              DİNLEME
  TCP    127.0.0.1:3552         127.0.0.1:50123        KURULDU
  TCP    192.168.1.20:139       0.0.0.0:0              DİNLEME
  TCP    [::]:8081              [::]:0                 LISTENING
  TCP    [::1]:9000             [::]:0                 LISTENING
";
        assert_eq!(parse_netstat(text), vec![135, 3552, 8081, 9000]);
    }

    #[cfg(not(windows))]
    #[test]
    fn proc_net_tcp_parsing() {
        let text = "  sl  local_address rem_address   st tx_queue rx_queue
   0: 0100007F:0DE0 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000
   1: 00000000:1F91 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000
   2: 0100007F:0DE0 0100007F:C350 01 00000000:00000000 00:00000000 00000000  1000
   3: 0501A8C0:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000
";
        assert_eq!(parse_proc_net_tcp(text), vec![3552, 8081]);
    }

    #[test]
    fn finds_a_local_http_server() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in l.incoming().take(4) {
                let mut s = s.unwrap();
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let body = "<html><head><title>Test Panel</title></head></html>";
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let p = probe(port).expect("probe");
        assert_eq!(p.title.as_deref(), Some("Test Panel"));
    }

    #[test]
    fn root_404_is_not_a_panel() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in l.incoming().take(4) {
                let mut s = s.unwrap();
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                let body = "<html><head><title>Not Found</title></head></html>";
                let _ = write!(
                    s,
                    "HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        assert!(probe(port).is_none());
    }

    #[test]
    fn skipped_ports_say_why() {
        use crate::testutil::{response, serve};
        let p401 = serve(vec![response(
            "401 Unauthorized",
            "text/plain",
            "",
            "missing or wrong daemon token",
        )]);
        let pjson = serve(vec![response(
            "200 OK",
            "application/json; charset=utf-8",
            "",
            "{}",
        )]);
        let ptext = serve(vec![response("200 OK", "text/plain", "", "hello")]);
        let phtml = serve(vec![response(
            "200 OK",
            "text/html",
            "",
            "<!doctype html><title>T</title>",
        )]);
        assert_eq!(classify(p401), Outcome::NotPage("401".into()));
        assert_eq!(classify(pjson), Outcome::NotPage("JSON".into()));
        assert_eq!(classify(ptext), Outcome::NotPage("text/plain".into()));
        assert_eq!(classify(phtml), Outcome::Page(Some("T".into())));
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert_eq!(classify(closed), Outcome::Nothing);
    }

    #[test]
    fn api_401_plain_text_endpoint_is_not_offered() {
        use crate::testutil::{response, serve};
        // The cua daemon's API port: 401, text/plain, no page.
        let port = serve(vec![response(
            "401 Unauthorized",
            "text/plain; charset=utf-8",
            "",
            "missing or wrong daemon token",
        )]);
        assert!(probe(port).is_none());
    }

    #[test]
    fn json_endpoint_is_not_offered() {
        use crate::testutil::{response, serve};
        let port = serve(vec![response(
            "200 OK",
            "application/json",
            "",
            r#"{"version":"1.2.3"}"#,
        )]);
        assert!(probe(port).is_none());
    }

    #[test]
    fn redirect_to_a_login_page_is_offered() {
        use crate::testutil::{response, serve};
        let port = serve(vec![
            response("302 Found", "", "Location: /login\r\n", ""),
            response(
                "200 OK",
                "text/html; charset=utf-8",
                "",
                "<!doctype html><title>Sign in</title>",
            ),
        ]);
        let p = probe(port).expect("redirect to an HTML login page is a panel");
        assert_eq!(p.title.as_deref(), Some("Sign in"));
    }

    #[test]
    fn redirect_to_another_site_is_not_followed() {
        use crate::testutil::{response, serve};
        let port = serve(vec![response(
            "302 Found",
            "",
            "Location: https://example.com/\r\n",
            "",
        )]);
        assert!(probe(port).is_none());
    }

    #[test]
    fn html_without_content_type_is_offered() {
        use crate::testutil::{response, serve};
        let port = serve(vec![response(
            "200 OK",
            "",
            "",
            "<!DOCTYPE html><html><title>Bare</title></html>",
        )]);
        assert_eq!(
            probe(port).expect("bare HTML").title.as_deref(),
            Some("Bare")
        );
    }

    #[test]
    fn pretty_names() {
        assert_eq!(pretty("docker-socket-proxy"), "Docker socket proxy");
        assert_eq!(pretty("arcane"), "Arcane");
    }
}
