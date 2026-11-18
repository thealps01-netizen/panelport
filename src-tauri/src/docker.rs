//! Optional Docker integration through the local `docker` CLI.
//!
//! Only these fixed commands are ever run: `docker version`, `docker ps -a -q --no-trunc`,
//! `docker inspect <id>...`, `docker start <id>` and `docker logs --tail <n> --timestamps <id>`.
//! Container ids always come from Docker's own output and are validated as hex; nothing a web
//! page sends ends up on a command line.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    /// No docker CLI found on this computer.
    NotInstalled,
    /// CLI found, engine not answering.
    NotRunning,
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub state: String,
    pub project: Option<String>,
    pub service: Option<String>,
    /// Compose services this one depends on.
    #[serde(skip)]
    pub depends_on: Vec<String>,
    /// Host ports published on an address this computer can reach (loopback or all interfaces).
    pub ports: Vec<u16>,
    /// Exit code of the last run, for a stopped container.
    pub exit_code: Option<i64>,
    /// When the container last stopped (RFC 3339), for a stopped container.
    pub finished_at: Option<String>,
}

pub fn is_container_id(s: &str) -> bool {
    (12..=64).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Finds the docker CLI: PATH first, then the Docker Desktop install folder (Windows).
pub fn find_cli() -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "docker.exe"
    } else {
        "docker"
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let p = dir.join(exe);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    docker_desktop_dir()
        .map(|d| d.join("resources").join("bin").join(exe))
        .filter(|p| p.is_file())
}

/// Docker Desktop's install folder, from its uninstall registry entry or Program Files.
#[cfg(windows)]
pub fn docker_desktop_dir() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    const KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Docker Desktop";
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if let Ok(k) = RegKey::predef(root).open_subkey(KEY) {
            if let Ok(loc) = k.get_value::<String, _>("InstallLocation") {
                let p = PathBuf::from(loc);
                if p.join("Docker Desktop.exe").is_file() {
                    return Some(p);
                }
            }
        }
    }
    let pf = std::env::var_os("ProgramFiles").map(PathBuf::from)?;
    let p = pf.join("Docker").join("Docker");
    p.join("Docker Desktop.exe").is_file().then_some(p)
}

#[cfg(not(windows))]
pub fn docker_desktop_dir() -> Option<PathBuf> {
    None
}

pub fn can_start_desktop() -> bool {
    docker_desktop_dir().is_some()
}

/// Starts Docker Desktop (Windows). Returns a short reason on failure (for the log).
pub fn start_desktop() -> Result<(), String> {
    let dir = docker_desktop_dir().ok_or("Docker Desktop install folder not found")?;
    let mut cmd = Command::new(dir.join("Docker Desktop.exe"));
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("starting Docker Desktop failed: {e}"))
}

fn run(cli: &PathBuf, args: &[&str], timeout: Duration) -> Result<Output, String> {
    let mut cmd = Command::new(cli);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("docker {}: {e}", args[0]))?;
    // Drain both pipes on threads so a large output cannot block the child.
    let drain = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
    );
    let err = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
    );
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "docker {}: timed out after {}s",
                    args[0],
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("docker {}: {e}", args[0])),
        }
    };
    Ok(Output {
        status,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

pub fn engine(cli: Option<&PathBuf>) -> Engine {
    engine_info(cli).0
}

/// Engine state and, when it answers, its version.
pub fn engine_info(cli: Option<&PathBuf>) -> (Engine, Option<String>) {
    let Some(cli) = cli else {
        return (Engine::NotInstalled, None);
    };
    let engine = match run(
        cli,
        &["version", "--format", "{{.Server.Version}}"],
        Duration::from_secs(8),
    ) {
        Ok(o) if o.status.success() && !o.stdout.trim_ascii().is_empty() => {
            let v = String::from_utf8_lossy(o.stdout.trim_ascii())
                .chars()
                .take(40)
                .collect();
            return (Engine::Running, Some(v));
        }
        Ok(o) => {
            log::info!(
                "docker engine not answering: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            );
            Engine::NotRunning
        }
        Err(e) => {
            log::warn!("{e}");
            Engine::NotRunning
        }
    };
    (engine, None)
}

/// The container that publishes `port` (the one a panel's address points at).
pub fn owner(all: &[Container], port: u16) -> Option<&Container> {
    all.iter().find(|c| c.ports.contains(&port))
}

/// Last `tail` log lines of a container, oldest first, as `HH:MM:SS message`. Docker writes the
/// container's stdout and stderr to the CLI's stdout and stderr; both are merged by timestamp.
pub fn logs(cli: &PathBuf, id: &str, tail: u32) -> Result<Vec<String>, String> {
    if !is_container_id(id) {
        return Err(format!("refusing logs for invalid container id {id:?}"));
    }
    let tail = tail.clamp(1, 500).to_string();
    let o = run(
        cli,
        &["logs", "--tail", &tail, "--timestamps", id],
        Duration::from_secs(15),
    )?;
    if !o.status.success() {
        return Err(format!(
            "docker logs failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(merge_logs(&o.stdout, &o.stderr, tail.parse().unwrap_or(40)))
}

/// Removes terminal colour and cursor sequences (`ESC [ ... letter`) that containers often log.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

pub fn merge_logs(out: &[u8], err: &[u8], keep: usize) -> Vec<String> {
    let out = strip_ansi(&String::from_utf8_lossy(out));
    let err = strip_ansi(&String::from_utf8_lossy(err));
    let mut lines: Vec<(String, String)> = out
        .lines()
        .chain(err.lines())
        .filter(|l| !l.trim().is_empty())
        .map(|l| match l.split_once(' ') {
            Some((ts, rest)) if ts.len() >= 19 && ts.as_bytes()[10] == b'T' => {
                (ts.to_string(), format!("{} {}", &ts[11..19], rest))
            }
            _ => (String::new(), l.to_string()),
        })
        .collect();
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    let skip = lines.len().saturating_sub(keep);
    lines
        .into_iter()
        .skip(skip)
        .map(|(_, l)| l.chars().take(400).collect())
        .collect()
}

/// All containers (running or not) with their published ports.
pub fn containers(cli: &PathBuf) -> Result<Vec<Container>, String> {
    let o = run(
        cli,
        &["ps", "-a", "-q", "--no-trunc"],
        Duration::from_secs(10),
    )?;
    if !o.status.success() {
        return Err(format!(
            "docker ps failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    let ids: Vec<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| is_container_id(l))
        .map(String::from)
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut args: Vec<&str> = vec!["inspect"];
    args.extend(ids.iter().map(String::as_str));
    let o = run(cli, &args, Duration::from_secs(15))?;
    if !o.status.success() {
        return Err(format!(
            "docker inspect failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    parse_inspect(&o.stdout)
}

pub fn parse_inspect(json: &[u8]) -> Result<Vec<Container>, String> {
    let v: Value =
        serde_json::from_slice(json).map_err(|e| format!("docker inspect output: {e}"))?;
    let arr = v.as_array().ok_or("docker inspect output is not a list")?;
    let mut out = Vec::new();
    for c in arr {
        let id = c["Id"].as_str().unwrap_or_default().to_string();
        if !is_container_id(&id) {
            continue;
        }
        let name = c["Name"]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string();
        let labels = &c["Config"]["Labels"];
        let label = |k: &str| {
            labels[k]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(String::from)
        };
        // "svc:condition:restart,svc2:..." (compose v2 label format)
        let depends_on = label("com.docker.compose.depends_on")
            .map(|s| {
                s.split(',')
                    .filter_map(|d| d.split(':').next())
                    .filter(|d| !d.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let mut ports = Vec::new();
        if let Some(bindings) = c["HostConfig"]["PortBindings"].as_object() {
            for list in bindings.values() {
                for b in list.as_array().into_iter().flatten() {
                    let ip = b["HostIp"].as_str().unwrap_or("");
                    let reachable = matches!(
                        ip,
                        "" | "0.0.0.0" | "::" | "127.0.0.1" | "::1" | "localhost"
                    );
                    if let Some(port) = b["HostPort"].as_str().and_then(|p| p.parse::<u16>().ok()) {
                        if reachable && port != 0 && !ports.contains(&port) {
                            ports.push(port);
                        }
                    }
                }
            }
        }
        // Ports Docker assigned at run time (`-p 8080` style) only show up here.
        if let Some(net) = c["NetworkSettings"]["Ports"].as_object() {
            for list in net.values() {
                for b in list.as_array().into_iter().flatten() {
                    let ip = b["HostIp"].as_str().unwrap_or("");
                    let reachable = matches!(ip, "" | "0.0.0.0" | "::" | "127.0.0.1" | "::1");
                    if let Some(port) = b["HostPort"].as_str().and_then(|p| p.parse::<u16>().ok()) {
                        if reachable && port != 0 && !ports.contains(&port) {
                            ports.push(port);
                        }
                    }
                }
            }
        }
        ports.sort_unstable();
        let running = c["State"]["Running"].as_bool().unwrap_or(false);
        out.push(Container {
            id,
            name,
            running,
            state: c["State"]["Status"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            project: label("com.docker.compose.project"),
            service: label("com.docker.compose.service"),
            depends_on,
            ports,
            exit_code: if running {
                None
            } else {
                c["State"]["ExitCode"].as_i64()
            },
            finished_at: c["State"]["FinishedAt"]
                .as_str()
                .filter(|t| !running && !t.starts_with("0001-"))
                .map(String::from),
        });
    }
    Ok(out)
}

/// Containers behind `port`: the container publishing it, plus — when it belongs to a Compose
/// project — every container of that project, ordered so dependencies start first.
pub fn backing(all: &[Container], port: u16) -> Vec<Container> {
    let Some(owner) = all.iter().find(|c| c.ports.contains(&port)) else {
        return Vec::new();
    };
    let group: Vec<&Container> = match &owner.project {
        Some(p) => all
            .iter()
            .filter(|c| c.project.as_ref() == Some(p))
            .collect(),
        None => vec![owner],
    };
    // Topological order by compose service dependencies (cycles fall back to name order).
    let by_service: HashMap<&str, &Container> = group
        .iter()
        .filter_map(|c| c.service.as_deref().map(|s| (s, *c)))
        .collect();
    let mut ordered: Vec<Container> = Vec::new();
    let mut done: HashSet<String> = HashSet::new();
    fn visit<'a>(
        c: &'a Container,
        by_service: &HashMap<&str, &'a Container>,
        done: &mut HashSet<String>,
        stack: &mut Vec<String>,
        out: &mut Vec<Container>,
    ) {
        if done.contains(&c.id) || stack.contains(&c.id) {
            return;
        }
        stack.push(c.id.clone());
        for d in &c.depends_on {
            if let Some(dep) = by_service.get(d.as_str()) {
                visit(dep, by_service, done, stack, out);
            }
        }
        stack.pop();
        if done.insert(c.id.clone()) {
            out.push(c.clone());
        }
    }
    let mut sorted = group.clone();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for c in sorted {
        visit(c, &by_service, &mut done, &mut Vec::new(), &mut ordered);
    }
    ordered
}

/// Starts the given containers one by one, in order. Ids must come from `containers()`.
pub fn start(cli: &PathBuf, ids: &[String]) -> Result<(), String> {
    for id in ids {
        if !is_container_id(id) {
            return Err(format!("refusing to start invalid container id {id:?}"));
        }
        let o = run(cli, &["start", id], Duration::from_secs(60))?;
        if !o.status.success() {
            return Err(format!(
                "docker start {id} failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSPECT: &str = r#"[
      {"Id":"aaaaaaaaaaaa1111111111111111111111111111111111111111111111111111","Name":"/arcane",
       "State":{"Running":false,"Status":"exited"},
       "Config":{"Labels":{"com.docker.compose.project":"arcane","com.docker.compose.service":"arcane",
         "com.docker.compose.depends_on":"docker-socket-proxy:service_healthy:false"}},
       "HostConfig":{"PortBindings":{"3552/tcp":[{"HostIp":"127.0.0.1","HostPort":"3552"}]}},
       "NetworkSettings":{"Ports":{}}},
      {"Id":"bbbbbbbbbbbb2222222222222222222222222222222222222222222222222222","Name":"/arcane-docker-proxy",
       "State":{"Running":false,"Status":"exited"},
       "Config":{"Labels":{"com.docker.compose.project":"arcane","com.docker.compose.service":"docker-socket-proxy"}},
       "HostConfig":{"PortBindings":{}},"NetworkSettings":{"Ports":{}}},
      {"Id":"cccccccccccc3333333333333333333333333333333333333333333333333333","Name":"/db",
       "State":{"Running":true,"Status":"running"},
       "Config":{"Labels":{}},
       "HostConfig":{"PortBindings":{"5432/tcp":[{"HostIp":"192.168.1.9","HostPort":"5432"}]}},
       "NetworkSettings":{"Ports":{"80/tcp":[{"HostIp":"0.0.0.0","HostPort":"49153"}]}}},
      {"Id":"not-hex; rm -rf /","Name":"/evil","State":{},"Config":{"Labels":{}},"HostConfig":{}}
    ]"#;

    #[test]
    fn parses_inspect_and_skips_bad_ids() {
        let cs = parse_inspect(INSPECT.as_bytes()).unwrap();
        assert_eq!(cs.len(), 3);
        assert_eq!(cs[0].name, "arcane");
        assert_eq!(cs[0].ports, vec![3552]);
        assert_eq!(cs[0].depends_on, vec!["docker-socket-proxy"]);
        // A port bound to another interface is not reachable as localhost; the auto-assigned one is.
        assert_eq!(cs[2].ports, vec![49153]);
    }

    #[test]
    fn backing_orders_dependencies_first() {
        let cs = parse_inspect(INSPECT.as_bytes()).unwrap();
        let b = backing(&cs, 3552);
        let names: Vec<_> = b.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["arcane-docker-proxy", "arcane"]);
        assert!(backing(&cs, 9999).is_empty());
    }

    #[test]
    fn logs_are_merged_by_time_and_trimmed() {
        let out = b"2026-11-18T14:02:11.100Z SIGTERM received\n2026-11-18T14:02:12.300Z Closed database\n";
        let err = b"2026-11-18T14:02:11.900Z Shutting down\n\n";
        assert_eq!(
            merge_logs(out, err, 10),
            vec![
                "14:02:11 SIGTERM received",
                "14:02:11 Shutting down",
                "14:02:12 Closed database"
            ]
        );
        assert_eq!(merge_logs(out, err, 1), vec!["14:02:12 Closed database"]);
        // Lines without a timestamp are kept as they are.
        assert_eq!(merge_logs(b"plain line\n", b"", 5), vec!["plain line"]);
        // Terminal colour codes are removed.
        let coloured =
            b"2026-11-18T17:10:26.630Z \x1b[2mNov 18\x1b[0m \x1b[92mINF\x1b[0m Incoming request\n";
        assert_eq!(
            merge_logs(coloured, b"", 5),
            vec!["17:10:26 Nov 18 INF Incoming request"]
        );
    }

    #[test]
    fn stopped_container_reports_exit_code_and_stop_time() {
        let cs = parse_inspect(INSPECT.as_bytes()).unwrap();
        // The fixture's containers carry no ExitCode/FinishedAt: both stay unknown.
        assert_eq!(cs[0].exit_code, None);
        let json = r#"[{"Id":"dddddddddddd4444444444444444444444444444444444444444444444444444","Name":"/kuma",
            "State":{"Running":false,"Status":"exited","ExitCode":137,"FinishedAt":"2026-11-18T14:02:12.5Z"},
            "Config":{"Labels":{}},"HostConfig":{"PortBindings":{"3001/tcp":[{"HostIp":"","HostPort":"3001"}]}}}]"#;
        let c = &parse_inspect(json.as_bytes()).unwrap()[0];
        assert_eq!(c.exit_code, Some(137));
        assert_eq!(c.finished_at.as_deref(), Some("2026-11-18T14:02:12.5Z"));
        assert_eq!(
            owner(std::slice::from_ref(c), 3001).map(|c| c.name.as_str()),
            Some("kuma")
        );
        assert!(
            logs(&PathBuf::from("docker"), "--follow", 5).is_err(),
            "ids are validated"
        );
    }

    #[test]
    fn id_validation() {
        assert!(is_container_id("0123456789ab"));
        assert!(!is_container_id("0123456789AB"));
        assert!(!is_container_id("--help"));
        assert!(!is_container_id("arcane"));
        assert!(!is_container_id("0123456789ab;x"));
    }
}
