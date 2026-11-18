# Panelport

Panelport opens the web panels that run on your computer in their own desktop windows.

Many self-hosted tools have a web page on `localhost`: Docker managers such as
[Arcane](https://getarcane.app), home-lab dashboards, admin pages. Panelport finds them, gives
each one a native window with its own icon, size and position, and tells you when one stops
answering. It is a small Windows app built with Tauri 2 and Rust.

> **Unofficial companion app.** Panelport is not affiliated with or endorsed by Arcane or the
> makers of any other panel. Arcane is a separate open-source project (BSD-3-Clause) and is not
> part of Panelport.

## What it does

- **Finds panels.** On first start, and whenever you click *Find panels*, Panelport lists the
  web ports published by running Docker containers and the local web servers that answer HTTP.
  It suggests a name from the container or the page title. You tick the ones to add.
- **One window per panel.** Each window shows the panel's own icon and remembers its size,
  position and maximized state. Links to other sites open in your default browser.
- **Launcher and tray.** The launcher shows a card per panel with its live status. The tray icon
  is green when every panel answers, amber when some do not, and red when none do. Its menu lists
  every panel with a status dot, plus *Open Panelport*, *Find panels…*, *Settings…*,
  *Start with Windows* (off until you turn it on) and *Quit*.
- **Health checks.** Every 5 seconds Panelport requests each panel's health path (default `/`).
  A notification tells you when a panel goes down or comes back; a single failed check does not
  count.
- **One-click fixes.** When a panel on this computer is down and Docker is behind it, the panel
  window offers *Start Docker Desktop* if the engine is not running, and *Start containers* for
  the stopped containers (or Compose project) that publish the panel's port. Nothing starts until
  you click.
- Closing the launcher keeps Panelport running in the tray. Starting Panelport again brings the
  existing window back.
- Light and dark mode follow Windows. The interface is in Turkish when Windows is, English
  otherwise.
- No telemetry and no update checks. Panelport only connects to the panel addresses you add and
  to Docker on this computer.

## Install

Download `Panelport_<version>_x64-setup.exe` from the
[releases page](https://github.com/thealps01-netizen/panelport/releases) and run it. It installs
for your Windows user only and needs no administrator rights. [INSTALL.md](INSTALL.md) explains
the "Windows protected your PC" screen, how to check the download, and how to uninstall.

## Security

Panels are web pages made by other programs, so Panelport does not trust them:

- Panel pages cannot call any of Panelport's native commands (Tauri IPC). Only the pages built
  into Panelport can. A unit test calls every command from six panel addresses and expects a
  refusal; an end-to-end run repeats the check from inside a live Arcane window.
- A panel window shows only its own panel's site and Panelport's built-in pages.
- Docker actions use a fixed list of commands. Container ids come from Docker itself, never from
  a web page.

Report security problems privately as described in [SECURITY.md](SECURITY.md).

## Where things are

| What | Where |
| --- | --- |
| Program | `%LOCALAPPDATA%\Panelport` |
| Settings (`settings.json`) and cached panel icons | `%LOCALAPPDATA%\io.github.thealps01-netizen.panelport` |
| Logs (1 MB per file, 3 old files kept) | `%LOCALAPPDATA%\io.github.thealps01-netizen.panelport\logs` |

## Building

Tests and the end-to-end run (`scripts/e2e-linux.sh`) run on Linux. The Windows installer is
built by GitHub Actions (`.github/workflows/build-and-release.yml`) from locked dependencies,
installed and started on a fresh Windows runner, and only then published with `SHA256SUMS.txt`,
a CycloneDX SBOM and a build-provenance attestation.

```
npm ci
cargo test --locked --manifest-path src-tauri/Cargo.toml
npx tauri build --bundles nsis -- --locked
bash scripts/release-check.sh      # cargo audit + npm audit; fails on known vulnerabilities
```

[DECISIONS.md](DECISIONS.md) records the design choices and their reasons.

## License

MIT © 2026 thealps01-netizen, see [LICENSE](LICENSE). The licenses of all bundled third-party
components are in `THIRD_PARTY_LICENSES.txt`, installed next to the program.
