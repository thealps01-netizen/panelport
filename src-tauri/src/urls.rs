//! URL rules: which panel URLs are accepted and where a window may navigate.

use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlError {
    Empty,
    Invalid,
    Scheme,
    Credentials,
    ReservedHost,
}

/// Hosts the Tauri runtime itself serves on Windows (`http://<name>.localhost`). A panel on one
/// of them would be mistaken for a bundled page, so they are refused.
const RESERVED_HOSTS: &[&str] = &["tauri.localhost", "ipc.localhost", "asset.localhost"];

/// Parses what the user typed (or discovery found) into a panel URL. `localhost:3552` is read
/// as `http://localhost:3552/`.
pub fn parse_panel_url(input: &str) -> Result<Url, UrlError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(UrlError::Empty);
    }
    let with_scheme = if s.contains("://") {
        s.to_string()
    } else {
        format!("http://{s}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| UrlError::Invalid)?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(UrlError::Scheme);
    }
    let host = url
        .host_str()
        .ok_or(UrlError::Invalid)?
        .to_ascii_lowercase();
    if host.is_empty() {
        return Err(UrlError::Invalid);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlError::Credentials);
    }
    if RESERVED_HOSTS.contains(&host.as_str()) {
        return Err(UrlError::ReservedHost);
    }
    Ok(url)
}

/// Builds the health-check URL: the panel's origin plus `health_path` (which starts with `/`).
pub fn health_url(panel: &Url, health_path: &str) -> Url {
    let mut u = panel.clone();
    let (path, query) = match health_path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (health_path, None),
    };
    u.set_path(path);
    u.set_query(query);
    u.set_fragment(None);
    u
}

/// Bundled pages: `tauri://localhost/...` (Linux/macOS) or `http(s)://tauri.localhost/...` (Windows).
pub fn is_app_local(url: &Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => url.host_str() == Some("tauri.localhost"),
        _ => false,
    }
}

/// Absolute URL of a bundled page, as the platform's webview sees it.
pub fn app_url(path_and_query: &str) -> Url {
    let base = if cfg!(windows) {
        "http://tauri.localhost/"
    } else {
        "tauri://localhost/"
    };
    Url::parse(base)
        .and_then(|b| b.join(path_and_query))
        .expect("static base URL")
}

pub fn same_origin(a: &Url, b: &Url) -> bool {
    a.origin() == b.origin()
}

/// True when the URL points at this computer (only then Docker on this PC can be behind it).
pub fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(ip)) => ip.is_loopback() || ip.is_unspecified(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback() || ip.is_unspecified(),
        None => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nav {
    Allow,
    /// Leave the window alone and open this http(s) URL in the default browser.
    External(Url),
    Deny,
}

/// Navigation rule for a window. `panel` is the panel's URL for panel windows, `None` for the
/// launcher and settings windows (bundled pages only).
pub fn decide(target: &Url, panel: Option<&Url>) -> Nav {
    if is_app_local(target) {
        // A panel window may only show the bundled "not reachable" page, so a panel page cannot
        // load (or frame) the launcher or settings pages inside its own window.
        return match panel {
            Some(_) if target.path() != "/down.html" => Nav::Deny,
            _ => Nav::Allow,
        };
    }
    if target.scheme() == "about" && (target.path() == "blank" || target.path() == "srcdoc") {
        return Nav::Allow;
    }
    if let Some(p) = panel {
        if same_origin(target, p) {
            return Nav::Allow;
        }
    }
    match target.scheme() {
        "http" | "https" if target.username().is_empty() && target.password().is_none() => {
            Nav::External(target.clone())
        }
        _ => Nav::Deny,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn parses_user_input() {
        assert_eq!(
            parse_panel_url("localhost:3552").unwrap().as_str(),
            "http://localhost:3552/"
        );
        assert_eq!(
            parse_panel_url(" https://nas.lan:5001/ui ")
                .unwrap()
                .as_str(),
            "https://nas.lan:5001/ui"
        );
        assert_eq!(parse_panel_url(""), Err(UrlError::Empty));
        assert_eq!(parse_panel_url("file:///c:/x"), Err(UrlError::Scheme));
        assert_eq!(
            parse_panel_url("javascript://alert(1)"),
            Err(UrlError::Scheme)
        );
        assert_eq!(
            parse_panel_url("http://a:b@localhost/"),
            Err(UrlError::Credentials)
        );
        assert_eq!(
            parse_panel_url("http://tauri.localhost/"),
            Err(UrlError::ReservedHost)
        );
        assert_eq!(
            parse_panel_url("http://IPC.localhost/"),
            Err(UrlError::ReservedHost)
        );
    }

    #[test]
    fn health_url_uses_origin_and_path() {
        let p = u("http://localhost:3552/some/page?x=1#frag");
        assert_eq!(
            health_url(&p, "/api/health").as_str(),
            "http://localhost:3552/api/health"
        );
        assert_eq!(health_url(&p, "/").as_str(), "http://localhost:3552/");
        assert_eq!(
            health_url(&p, "/h?full=1").as_str(),
            "http://localhost:3552/h?full=1"
        );
    }

    #[test]
    fn navigation_rules() {
        let arcane = u("http://localhost:3552/");
        assert_eq!(
            decide(&u("http://localhost:3552/containers"), Some(&arcane)),
            Nav::Allow
        );
        assert_eq!(
            decide(
                &u("http://tauri.localhost/down.html?panel=a"),
                Some(&arcane)
            ),
            Nav::Allow
        );
        assert_eq!(
            decide(&u("tauri://localhost/down.html"), Some(&arcane)),
            Nav::Allow
        );
        // A panel page cannot pull the launcher or settings pages into its window.
        assert_eq!(
            decide(&u("http://tauri.localhost/settings.html"), Some(&arcane)),
            Nav::Deny
        );
        assert_eq!(
            decide(&u("tauri://localhost/launcher.html#find"), Some(&arcane)),
            Nav::Deny
        );
        // Another local port is another origin: browser, not this window.
        assert_eq!(
            decide(&u("http://localhost:8080/"), Some(&arcane)),
            Nav::External(u("http://localhost:8080/"))
        );
        assert_eq!(
            decide(&u("https://getarcane.app/docs"), Some(&arcane)),
            Nav::External(u("https://getarcane.app/docs"))
        );
        assert_eq!(decide(&u("file:///etc/passwd"), Some(&arcane)), Nav::Deny);
        assert_eq!(decide(&u("data:text/html,hi"), Some(&arcane)), Nav::Deny);
        // Launcher/settings windows: bundled pages only.
        assert_eq!(
            decide(&u("tauri://localhost/settings.html"), None),
            Nav::Allow
        );
        assert_eq!(
            decide(&u("http://localhost:3552/"), None),
            Nav::External(u("http://localhost:3552/"))
        );
        assert_eq!(
            decide(&u("http://evil.example/"), None),
            Nav::External(u("http://evil.example/"))
        );
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback(&u("http://localhost:1/")));
        assert!(is_loopback(&u("http://127.0.0.1:1/")));
        assert!(is_loopback(&u("http://[::1]:1/")));
        assert!(!is_loopback(&u("http://192.168.1.5:1/")));
        assert!(!is_loopback(&u("https://example.com/")));
    }
}
