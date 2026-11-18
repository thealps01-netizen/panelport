# Changelog

## 1.1.1 — 2026-11-18

### Fixed
- Find panels offered addresses whose start page was an error page (for example HTTP 400 from a
  short-lived local program). Only addresses that answer with a working web page (HTTP 2xx and
  HTML, after same-site redirects such as `/` to `/login`) are offered now; the others are listed
  as skipped with their status.
- Adding panels checks each ticked address again first. An address that stopped answering, or now
  answers with an error, is not added, and Find panels names it in one line.
- A panel that answers with an HTTP error such as 400, 405 or 429 showed "Running". It now shows
  "Error · HTTP 400" (Turkish: "Hata · HTTP 400") with an amber dot, sends no notification of its
  own, and a later outage from that state is not announced. Existing panels are not changed.

## 1.1.0 — 2026-11-18

### Changed
- New look (NORA): a night-blue ground with white as the interface colour, panel names in tall
  condensed capitals, and a matching light version for Windows light mode. The fonts ship inside
  the app; nothing is loaded from the internet.
- The launcher has three views: Panels, Find panels and Settings. In Panels, the selected panel
  fills the centre with a stored preview of its window and its name set large; a list on the left
  shows every panel's state in words and colour; Open, Refresh, Logs and Edit sit on the right.
  A stopped Docker panel offers "Start containers" right in the list.
- Every window has Panelport's own title bar. Minimize, maximize and close behave like Windows'
  own buttons, including the Snap Layouts menu when you hover over maximize, double-click to
  maximize, dragging to the screen edge, Alt+F4 and Win+Up/Win+Down.
- A panel window's title bar shows the panel's name, address and state, with Refresh and
  "Back to panels". The title bar is a separate part of the window; the panel page still has no
  access to Panelport's functions.
- Find panels also lists the panels you already added, and says which addresses it skipped
  because they are not web pages.
- Settings has sections for panels (drag to reorder, or Alt+Up/Alt+Down), general options,
  notifications, saved passwords and About.

### Added
- Panel previews: a still image of each panel, taken on this PC while its window is open, shown
  in the launcher. Can be switched off and deleted in Settings.
- Container logs: the last lines of a Docker panel's container, in the launcher and on the
  "not reachable" page.
- Settings switches for notifications and for saving passwords, and a button that deletes the
  saved passwords.
- `--self-test` also checks that a panel window has its two parts, that the panel page is refused
  every command, and that the title-bar buttons answer Windows' hit test.

## 1.0.1 — 2026-11-18

### Fixed
- *Find panels* offers a port only when its start page is an HTML document. Program
  interfaces that answer with JSON, plain text, or a `401`/`403` without a page are left out,
  for example a local service that answers "missing or wrong daemon token". A redirect to
  another page on the same site (`/` to `/login`) is followed; a redirect to another site is not.
- A panel that answers without a web page is no longer shown as running. It now shows
  *Not a web page / no access* (Turkish: *Web sayfası değil / erişim yok*) with an amber dot,
  and its window shows Panelport's status page with what to check.
- Panel windows offer to save the passwords you type into them and fill them in next time.
  WebView2 has its password manager and form autofill switched off by default; panel windows
  now switch both on. Panelport's own windows are unchanged. The README section
  "Saved passwords" says where they are stored and how to delete them.

### Added
- `--self-test` opens a hidden panel window and reports whether password saving and form
  autofill read back as on. The release workflow fails unless both are on.

### Note on v1.0.0
- The `v1.0.0` tag was moved once before the release reached its owner: the first v1.0.0 build
  was withdrawn and rebuilt from a fixed commit.

## 1.0.0 — 2026-11-18

First release.
