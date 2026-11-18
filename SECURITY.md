# Security policy

## Reporting a vulnerability

Please **do not** report security problems in public issues.

Use GitHub's private vulnerability reporting: open the repository's **Security** tab and click
**Report a vulnerability**. You will get an answer as soon as possible.

Useful details: Panelport version, Windows version, what an attacker can do, and steps to
reproduce. The log folder (`%LOCALAPPDATA%\io.github.thealps01-netizen.panelport\logs`) may help;
check it for anything private before sharing.

## Supported versions

Security fixes are made for the **latest release** only. Older versions should be updated by
installing the newest release from the releases page.

## Scope notes

- Panel pages are untrusted web content. Any way for a panel page to reach Panelport's native
  commands, open a window on another origin inside Panelport, or make Panelport run a program is
  a vulnerability.
- Vulnerabilities in the panels themselves (for example Arcane) belong to those projects.
