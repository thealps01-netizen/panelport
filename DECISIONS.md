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
    job fails unless both values are `true`.
32. **No automated proof of the save-password prompt itself.** Driving a panel window from a
    test needs WebView2's DevTools port. WebView2 ignores `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`
    for Panelport because wry always sets the browser arguments itself, so the port would have to
    be a switch built into the release binary. That exposes a local debugging interface in the
    shipped app, so it was not added; the prompt is left to a manual check.

## 1.1.0

33. **NORA world for every bundled page.** The owner chose the NORA direction (catalog world
    "pop-culture shelf / vertical video feed") over the other comps; `.impeccable/surfaces/` holds
    the contract. The launcher gives the selected panel the stage: its stored preview under a dark
    scrim, its name set huge over the bottom edge, URL, state, response time and container below.
    Ground `#070d16 → #1d4363` radial, white as the only interface colour, status green
    `#5ee39a`, amber `#ffc44d`, red `#ff6b6b` only for state. Antonio (200/300) for panel names
    and display, Manrope for interface text, JetBrains Mono for ports and milliseconds.
34. **Status reads at a glance.** The owner found the mock's statuses too small. Every state is a
    dot with a soft ring plus the word in the state's colour (14 px semibold in lists, 17 px on
    the stage and the down page), never colour alone.
35. **Fonts are files inside the app.** Subsets (Latin, Latin-1, Latin Extended-A for Turkish)
    of the three variable fonts are bundled as woff2 (`ui/fonts`, about 97 KB),
    loaded with `@font-face` from the app itself; the CSP stays `font-src 'self'` and nothing is
    fetched from the network. `scripts/fetch-fonts.sh` rebuilds them from a pinned google/fonts
    commit; their OFL texts are appended to `THIRD_PARTY_LICENSES.txt`.
36. **Turkish dotted İ.** Uppercased panel names render with `lang="en"` unless the name contains
    Turkish letters, so "Home Assistant" becomes "HOME ASSISTANT", not "HOME ASSİSTANT".
37. **Light counterpart, same world.** Windows light mode switches to a pale mist-blue ground
    (`#f6f9fc → #ccdaea`) with ink `#0b1622` as the only interface colour, the same type and
    controls; the stage and the log stay dark because they frame a picture and terminal output.
    State words use darker tones in light mode (`#127a45`, `#8a5800`, `#b3242c`) for contrast;
    the dots keep the dark-mode hues. The owner uses dark.
38. **No eyebrows.** The mocks put "Panel kapalı · 4 dakikadır" and "Bu bilgisayarda bulunanlar ·
    6 sn önce tarandı" above the big headings. The design rules this build follows ban a label
    above a heading, so both lines sit below their heading as the status and scan lines.
39. **One launcher window, three views.** Panels, Find panels and Settings are tabs of the
    launcher, as in the mocks; the separate Settings window of 1.0 is gone. The tray's
    "Find panels…" and "Settings…" open the launcher on that view.
40. **Container logs, safely.** "Günlük" (launcher rail) and the down page's log box show the last
    40 lines of the container that publishes the panel's port: `docker logs --tail 40
    --timestamps <id>`, added to the fixed command list; the id comes from Docker's own output,
    colour codes are stripped, and the text is shown with `textContent`. The button is not shown
    for panels that are not Docker containers on this PC. The mock's "Günlüğü aç" button on the
    down page was dropped: the log is already on that page.
41. **Settings toggles.** "Panel kapanınca bildir", "Şifreleri kaydet" and "Önizlemeleri sakla"
    are stored as `prefs` in `settings.json`, defaults on. The fields are optional with defaults,
    so 1.0.x files load unchanged and the schema stays 1; a 1.0.x Panelport reading a 1.1 file
    ignores them. Turning previews off deletes the stored previews. "Kayıtlı şifreleri sil" uses
    WebView2's `ClearBrowsingData(PASSWORD_AUTOSAVE)` on Panelport's own profile.

### Panel previews

42. **Stored snapshot, never a live embed.** When a panel window has loaded its panel, Panelport
    takes a PNG with WebView2's `CapturePreview` (2.5 s after the load, then at most every 3
    minutes while the window is visible) into `<app data>/previews/<id>.png`. The launcher shows
    only that file. A panel never opened shows its icon or letter. Linux (development only) takes
    no previews.

### Custom title bar

43. **Frameless windows with a bundled title bar.** Every window uses `decorations(false)`. The
    launcher's top row is its title bar; a panel window's title bar is its own webview (below).
    Dragging empty bar space calls `start_dragging` (the system move loop, so Aero Snap by drag
    works), double-click toggles maximize, Tauri keeps the DWM shadow, the Windows 11 rounded
    corners and edge resizing for undecorated windows.
44. **Snap Layouts through the system hit test.** The minimize, maximize and close glyphs are
    drawn by the page. Over them sits a native child window (`PANELPORT_CAPTION_BUTTONS`,
    `WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP`, so it draws nothing) that answers
    `WM_NCHITTEST` with `HTMINBUTTON`, `HTMAXBUTTON` and `HTCLOSE`, handles the button clicks with
    `WM_SYSCOMMAND`, and reports hover and press to the page. Windows 11 shows the Snap Layouts
    flyout for a window that reports `HTMAXBUTTON`. Sources: Microsoft Learn, "Support snap
    layouts for desktop apps on Windows 11"
    (https://learn.microsoft.com/windows/apps/desktop/modernize/ui/apply-snap-layout-menu), the
    same input-sink pattern in Windows Terminal (`NonClientIslandWindow`), and Tauri's own
    undecorated-resize border window (`TAURI_DRAG_RESIZE_WINDOW` in tauri-runtime-wry), which
    returns hit-test codes from a same-thread child window in the same way. The window keeps
    `WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX`, so Alt+F4 and Win+Up / Win+Down work.
45. **Two webviews per panel window (Tauri `unstable`).** A panel window (`pwin-<id>`) holds a
    title-bar webview `bar-<id>` (bundled `titlebar.html`) above the panel webview `panel-<id>`.
    - `bar-*` has its own capability (`title-bar.json`) with only the window commands and
      `bar_info` / `bar_reload` / `bar_back`, which act on the caller's own window and panel and
      take no arguments. Its navigation guard allows `titlebar.html` and nothing else.
    - `panel-*` keeps the 1.0 rules: no `remote` section anywhere, so its panel origin has no IPC;
      only the bundled down page inside it may call app commands; per-origin navigation guard,
      password autosave and the same data folder.
    - Capabilities match webviews by label and the launcher by window; panel windows themselves
      are in no capability, so a webview there is matched only by its own label.
    - Risk: the multi-webview API is marked unstable and may change in any Tauri release, so
      `tauri`, `tauri-build`, `tauri-runtime-wry`, `wry`, `webview2-com` and `windows` are pinned
      to exact versions and upgraded deliberately. On Linux (development only) Tauri packs child
      webviews into a GTK box that ignores bounds; Panelport pins the bar's height there.
46. **Evidence.** Unit tests on the mock runtime build real two-webview windows and check every
    app, title-bar, core and plugin command from six panel origins in four webviews; mutation runs
    (`scripts/mutation-check.sh`) show each rule's test failing when the rule is broken.
    `--self-test` builds a real panel window and checks two webviews, IPC refused to the panel
    origin, password read-back and the caption hit test; the clean-runner job requires all four.
    `scripts/screens-windows.ps1` drives the real WebView2 build on the Windows runner.
