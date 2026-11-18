# Changelog

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
