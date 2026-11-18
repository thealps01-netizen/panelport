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
  position and maximized state. Its title bar shows the panel's name, address and state, with
  Refresh and *Back to panels*. Links to other sites open in your default browser.
- **Launcher and tray.** The launcher lists every panel with its state in words and colour and
  shows the selected one large, with a stored preview of its window, its address, response time
  and container; *Open*, *Refresh*, *Logs* and *Edit* sit beside it. The tray icon
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
- Every window has Panelport's own title bar. Its buttons behave like Windows' own: hover over
  maximize for Snap Layouts, double-click the bar to maximize, drag it to a screen edge, Alt+F4,
  Win+Up / Win+Down.
- Closing the launcher keeps Panelport running in the tray. Starting Panelport again brings the
  existing window back.
- Light and dark mode follow Windows. The interface is in Turkish when Windows is, English
  otherwise.
- No telemetry and no update checks. Panelport only connects to the panel addresses you add and
  to Docker on this computer.
- Panel windows can save the passwords you type into them (see "Saved passwords").
- The launcher can keep a preview image of each panel (see "Panel previews").

## Install

Download `Panelport_<version>_x64-setup.exe` from the
[releases page](https://github.com/thealps01-netizen/panelport/releases) and run it. It installs
for your Windows user only and needs no administrator rights. [INSTALL.md](INSTALL.md) explains
the "Windows protected your PC" screen, how to check the download, and how to uninstall.

## Security

Panels are web pages made by other programs, so Panelport does not trust them:

- Panel pages cannot call any of Panelport's native commands (Tauri IPC). Only the pages built
  into Panelport can. A panel window is two parts: the title bar (a built-in page that may only
  control its own window) and the panel, whose page gets no commands at all. A unit test calls
  every command from six panel addresses and expects a refusal; an end-to-end run repeats the
  check from inside a live Arcane window, and `--self-test` repeats it in a real WebView2 window.
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
| Saved passwords and other browsing data of panel windows | `%LOCALAPPDATA%\io.github.thealps01-netizen.panelport\EBWebView` |
| Panel previews | `%LOCALAPPDATA%\io.github.thealps01-netizen.panelport\previews` |

## Saved passwords

When you sign in to a panel, its window offers to save the password, and fills it in the next
time. This is the password manager built into Microsoft's WebView2, the browser engine
Panelport uses; Panelport itself never reads or stores your passwords.

- Passwords are kept only on this PC, in Panelport's own WebView2 folder (see the table above),
  protected by WebView2 for your Windows user account.
- They are not synced to a Microsoft account and not shared with Microsoft Edge or any other
  browser.
- Panelport's own pages (launcher, title bars) do not save or fill anything.
- *Settings → Saved passwords* turns saving off and has a button that deletes every saved
  password. Deleting the `EBWebView` folder while Panelport is not running also removes them,
  and signs you out of every panel. The uninstaller keeps the folder unless you tick its option
  to delete the app data.

## Panel previews

The launcher shows a still image of each panel's window, taken on this PC with WebView2 while
the window is open (after it loads, then at most every 3 minutes). The image can contain whatever
the panel showed at that moment, including names and numbers on its screen.

- Previews are stored only on this PC, in the `previews` folder above, and are never sent
  anywhere. The launcher never embeds the live panel; it only shows the stored image.
- *Settings → General → Keep previews* switches them off (which deletes the stored ones);
  *Delete previews* removes them at any time. A panel you never opened shows its icon instead.

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
