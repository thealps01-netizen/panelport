# Product

<!-- impeccable:product-schema 1 -->

> Inferred record. The owner skipped the product interview and left the calls to the designer, so
> these facts come from README.md, DECISIONS.md, the code and the owner's stated requests. Lines
> marked *(inferred)* are assumptions for the owner to confirm or correct.

## Platform

web

The design surfaces are the bundled pages (launcher, settings, find panels, panel-down page),
rendered by WebView2 inside a Tauri 2 desktop shell on Windows 11. The panels themselves are
third-party web UIs; Panelport does not design them.

## Users

The owner runs self-hosted tools on a Windows PC: Docker containers managed with Arcane, plus other
local web panels. The owner is not a developer and works only through graphical interfaces. The
job: open the web panels running on this computer, and keep an eye on them, without hunting
through browser tabs and `localhost:<port>` addresses. *(inferred)* People with a similar home-lab
setup may install it from the public GitHub release.

## Product Purpose

Panelport turns every web panel running on this computer into a desktop app. It finds the panels,
gives each its own window, and says when one stops answering. Success: the owner never types a
`localhost` address again and sees at a glance which panels are up.

## Positioning

A browser "install as app" shortcut or a single-site wrapper (Pake, Nativefier) opens one known
address. Panelport finds the panels itself (ports published by Docker and local web servers that
serve HTML), watches all of them at once, and brings a stopped Docker panel back with one click.

## Operating Context

- Panelport lives in the Windows tray. The launcher is opened a few times a day; panel windows
  stay open for long sessions. *(inferred)*
- It runs next to Docker Desktop, and most panels are Docker containers on `127.0.0.1`.
- States the screens must show: panel up, down, starting, "not a web page / no access"; Docker
  engine running or stopped; first run with no panels added.

## Capabilities and Constraints

- Discovery of Docker-published ports and local HTML servers; one window per panel with
  remembered size and position; launcher cards with live status; tray menu with status dots;
  notifications when a panel goes down or comes back; "Start Docker Desktop" and "Start
  containers" buttons; settings to add, edit, remove and reorder panels; saved passwords inside
  panel windows.
- Follows Windows light and dark mode. The UI is Turkish when Windows is Turkish, English
  otherwise.
- No telemetry and no update checks. Builds are unsigned, so Windows SmartScreen warns on install.
- Panel pages can never call Panelport's own commands; only bundled pages can.

## Brand Commitments

- The name is Panelport, with its own original icon (never Arcane's logo). Wherever Arcane is
  named, Panelport is called an unofficial companion app.
- The redesign replaces the v1.0 look. The owner called it far from professional, so polishing it
  is not enough.

## Evidence on Hand

- Real data for designs: the owner's panels (Arcane at `http://localhost:3552`) and screenshots of
  the real states, kept in the owner's research archive outside this repository.
- No users, testimonials, metrics or press exist. Do not invent any.

## Product Principles

1. Status is read first and is never wrong or vague.
2. Opening a panel is the shortest path on every screen.
3. When something is down, the screen says why and offers the safe fix in place.
4. While everything works, Panelport stays quiet.
5. Nothing starts, stops or is shared until the owner clicks.

## Accessibility & Inclusion

The launcher and settings work from the keyboard, contrast holds in both Windows themes, and
status never relies on colour alone. *(inferred: the owner named no standard)*
