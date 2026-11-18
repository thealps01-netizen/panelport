# Design decisions

Each entry: the choice, and why. Made without asking the owner, as briefed; revisit freely.

## Product

1. **Name: Panelport.** The scope changed from an Arcane-only wrapper ("Arcane Desktop") to a
   manager for any local web panel. "Panelport" (a port for your panels; panels live on ports) is
   short, original, has no notable software using it, and avoids using a third party's trademark
   (Arcane) in the product name.
2. **Arcane is the first panel, not a built-in one.** No host, port or path is hard-coded;
   panels come from discovery or the user. Arcane's health endpoint (`/api/health`) is just that
   panel's configurable health path.
3. **Discovery = Docker-published ports + local listening ports that answer HTTP.** Docker gives
   good names (Compose service / container), the port scan catches non-Docker panels. Listening
   sockets are read from `netstat -ano` by their foreign address (`0.0.0.0:0` / `[::]:0`), not
   the translated "LISTENING" word, so it works on Turkish and other Windows languages. Each
   candidate port gets one GET; obvious non-web ports (SMB, RDP, databases) are skipped.
   Container-backed HTML pages are pre-ticked; others are offered unticked.
4. **Health rule:** any HTTP answer below 500 is "up" (login redirects and 401/403 mean the
   panel is alive); 5xx is "server error" (amber); no answer is "down". Redirects are not
   followed for checks. Changed in 1.0.1: see 28.
5. **Debounce:** a panel that was up must fail 2 checks in a row (≈10 s) before it counts as
   down; notifications per panel are at most one per 60 s, and a "back" notification is only sent
   after a "down" one.
6. **Docker fixes are generic:** the containers behind a panel are found by the published host
   port; if that container belongs to a Compose project, the whole project's stopped containers
   are started, dependencies first (from Compose's `depends_on` label), with plain
   `docker start <id>` calls. `docker compose` is not used because it needs the project's
   compose file, which may not be on this computer.
7. **Panel icons:** the panel's own favicon, fetched from the panel's origin only and cached in
   the app data folder; otherwise a coloured monogram. Only PNG/ICO become window icons.
8. **Close behaviour:** closing the launcher hides it (Panelport keeps watching from the tray;
   the first time a notification explains this). Closing a panel window closes just that window.
9. **Same-origin popups** (`target=_blank` inside a panel) open in the same panel window instead
   of a new unguarded window; other origins open in the default browser.
10. **No auto-updater in v1.** The build is unsigned; an updater that installs unsigned code
    would let whoever controls the release page run code on the user's PC. Instead,
    *Settings → About → Check for new versions* opens the releases page. When a signed build
    exists, add the Tauri updater with a `pubkey`.
11. **Languages:** English and Turkish, chosen from the OS UI language (`sys-locale`); English is
    the fallback. `PANELPORT_LANG=en|tr` overrides it (used by tests).

## Security

12. **Zero IPC for panel origins.** All app commands are registered in an app ACL manifest
    (build.rs), so each needs an explicit permission; the only capability
    (`capabilities/local-pages.json`) grants them to the windows `launcher`, `settings` and
    `panel-*` **without a `remote` section**, i.e. for bundled pages only. No core or plugin
    permissions are granted at all (bundled pages poll state instead of using events). This is
    the class of bug in Pake's GHSA-vq4w-55rh-9ff3. A second gate in the invoke handler
    (`top_level_guard`) runs a command only when the window's top-level page is bundled, so a
    bundled page loaded inside a panel page's frame cannot be used either (the ACL looks at the
    calling frame only); bundled pages also refuse to call commands when framed, and their CSP
    has `frame-ancestors 'none'`. A mock-runtime test invokes every app, core and plugin command
    from six panel origins in three windows plus the framed case; a Linux e2e run does the same
    from inside a live Arcane window. Commands that act take only a panel id; the app resolves
    everything else.
13. **Navigation guard per window** (`on_navigation`, `on_new_window`): the panel's own origin,
    `about:blank` and, in panel windows, only the bundled `down.html` page are allowed (launcher
    and settings windows: bundled pages only); other http(s) URLs go to the default browser; all
    other schemes are blocked. Panel URLs on Tauri's own hosts (`tauri.localhost`,
    `ipc.localhost`, `asset.localhost`) and URLs with credentials are refused.
14. **Strict CSP for bundled pages** (`script-src 'self'`, no inline script or style,
    `object-src 'none'`), `withGlobalTauri: false`, `freezePrototype: true`. Bundled pages use no
    third-party JavaScript.
15. **Docker commands from a fixed list** (`version`, `ps`, `inspect`, `start`); container ids
    must be 12–64 lowercase hex characters and come from Docker's own output.
16. **Opening things:** only http(s) URLs that passed the navigation rule and the fixed log
    folder are handed to the OS; no command opens arbitrary strings from a page.

## Data and logs

17. **Settings** live in Tauri's per-user app data folder (`%LOCALAPPDATA%\<identifier>`), as
    `settings.json` with `"schema": 1`. Older files are backed up (`settings.schema<N>.<time>.bak.json`)
    and upgraded; schema 0 is the pre-release development format. A file from a newer version is
    used read-only and never overwritten; an unreadable file is moved aside and reported.
    The folder name is the bundle identifier (`io.github.thealps01-netizen.panelport`) because
    that is what Tauri, WebView2 data and the uninstaller use consistently.
18. **Logs:** `tauri-plugin-log`, UTF-8, `logs\panelport.log`, rotated at 1 MB keeping 3 old
    files. User-facing errors are one sentence with the next step and the log folder; raw errors
    go to the log only.
19. **No telemetry, no crash reporting,** so no consent prompt or privacy notice is needed beyond
    the README/About statement.

## Build and release

20. **Windows build only in GitHub Actions** (public repo, free standard runners); development
    and tests in an isolated Linux sandbox. The host only downloads, verifies and installs.
21. **Frontend without npm runtime packages:** plain HTML/CSS/JS bundled from `ui/`. npm is used
    only for the pinned Tauri CLI (prebuilt binary, `package-lock.json`, `npm ci`).
22. **Locked everything:** `Cargo.lock` + `--locked`, `package-lock.json` + `npm ci`,
    `rust-toolchain.toml` pins Rust 1.100.0, every action pinned to a commit SHA, gitleaks
    download checked against its SHA-256.
23. **NSIS per-user installer** (`installMode: currentUser`): no UAC prompt, installs to
    `%LOCALAPPDATA%\Panelport`. MSI is not built (per-machine only).
24. **Release gate:** gitleaks (history + tree), portable-paths audit, `cargo audit` +
    `npm audit` (`scripts/release-check.sh`), unit tests on Windows, then a clean-runner job
    installs the built installer silently, runs `--self-test`, starts the app, checks its window,
    single instance and log, and uninstalls. Only then is the release published with
    `SHA256SUMS.txt`, a CycloneDX SBOM (cargo-cyclonedx, Windows target) and a build-provenance
    attestation (`actions/attest`).
25. **THIRD_PARTY_LICENSES.txt** is generated by cargo-about for the Windows target (license
    texts with each crate's own copyright lines) plus any NOTICE files, installed next to the exe.
26. **License:** MIT, author = the GitHub account owner. Arcane is credited as a separate project.
27. **Test profile (`PANELPORT_PROFILE_DIR`, absolute path):** settings, logs and WebView2 data go
    to that folder and the single-instance lock is skipped. It exists so the installed build can
    be checked (for example over WebView2's debug port) without reading or changing the user's
    own Panelport data or their running instance. "Start with Windows" still writes the real
    Run key, so tests leave it alone.

## 1.0.1

28. **"Not a web page / no access" state.** A panel is something to look at, so an answer that is
    not an HTML page is not "up". Rule per check:
    - no answer: down (debounced as before); 5xx: server error (amber);
    - 3xx: up (redirects are not followed; a web UI sending `/` to `/login` is alive);
    - 4xx: up only when the body is an HTML document (a web UI's login or error page);
      a non-HTML 4xx (an API wanting a token, JSON 401/403) is **not a web page**;
    - 2xx: when the health path is `/` (the page the window shows) the answer must be HTML,
      otherwise it is **not a web page**; a custom health path (Arcane's `/api/health` answers
      JSON) only needs a status below 400.
    "Not a web page" shows an amber dot, counts as degraded (amber tray icon, never red), is set
    on the first such answer (it is a definite answer, not an outage, so no debounce), sends no
    notification of its own, and puts the panel window on the status page. A "back" notification
    that is owed after a "down" one is still sent when the panel becomes up, even if it answered
    "not a web page" in between.
29. **Discovery offers HTML pages only.** A port is offered only when its start page is an HTML
    document: content type `text/html`/`application/xhtml+xml`, or no content type (or
    `application/octet-stream`) and a body starting with `<!doctype html` or `<html`. Same-origin
    redirects are followed up to 3 times; other redirects end the probe. Every offered page
    served by a Docker container is pre-ticked (all candidates are web pages now).
30. **Saved passwords in panel windows.** WebView2 defaults `IsPasswordAutosaveEnabled` and
    `IsGeneralAutofillEnabled` to false. Panel windows (`panel-*` only) set both to true through
    `ICoreWebView2Settings4` right after the window is created; the launcher and settings windows
    keep the defaults and their inputs say `autocomplete="off"`. Saved data lives in Panelport's
    own WebView2 data folder (`%LOCALAPPDATA%\io.github.thealps01-netizen.panelport\EBWebView`),
    protected by WebView2 for the Windows user, not synced and not shared with Edge. This is
    chosen over a Panelport-made password vault: WebView2's manager matches logins per site and
    already handles encryption; Panelport never sees the passwords.
31. **Runtime proof instead of a mock.** The setting can only be read from a real WebView2, so
    `--self-test` creates a hidden `panel-selftest` window with a throwaway data folder, calls the
    same `passwords::enable` as real panel windows, and reports the read-back; the clean-runner
    job fails unless both values are `true`. The same job runs a best-effort probe of the save
    prompt against a throwaway login page with throwaway credentials, in an isolated profile.
