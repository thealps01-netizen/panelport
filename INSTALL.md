# Installing Panelport on Windows

Panelport is an **unofficial companion app** that opens your locally hosted web panels (for
example Arcane) as desktop windows. It is free, open source (MIT) and sends no telemetry.

## 1. Download and check the file

From the release page download:

- `Panelport_<version>_x64-setup.exe` — the installer
- `SHA256SUMS.txt` — the checksums

Check that the download is intact and unchanged. In **PowerShell**, in your Downloads folder:

```powershell
Get-FileHash .\Panelport_1.0.0_x64-setup.exe -Algorithm SHA256
```

The long number it prints must be the same as the one next to the installer's name in
`SHA256SUMS.txt`. If it differs, delete the file and download it again.

If you use the GitHub command line tool, you can also check that the file was built by this
project's GitHub workflow:

```powershell
gh attestation verify .\Panelport_1.0.0_x64-setup.exe -R thealps01-netizen/panelport
```

## 2. Install

Double-click the installer. It installs **for your user only** into
`%LOCALAPPDATA%\Panelport` and needs no administrator rights. A Start menu entry is created.
Silent install for scripts: `Panelport_1.0.0_x64-setup.exe /S`.

### "Windows protected your PC" (SmartScreen)

This first version is **not code-signed**, so Windows SmartScreen may show a blue
"Windows protected your PC" window. To continue, click **More info**, check that the app name is
`Panelport_…_x64-setup.exe`, then click **Run anyway**. The warning goes away as more people
download the release; a signed release would build that reputation faster.

### Smart App Control

On Windows 11 with **Smart App Control** turned on, unsigned programs without a known good
reputation are blocked and there is no "Run anyway" button. In that case Panelport cannot be
installed until a signed release exists; do not turn Smart App Control off just for this app.

### Antivirus warnings

An antivirus program may wrongly flag or quarantine a new, unsigned program (a "false positive").
If that happens:

- **Do not** switch your antivirus off and **do not** add an exception.
- Send the antivirus vendor the program name, the alert text and the SHA-256 from
  `SHA256SUMS.txt`, and report it as a false positive; the vendor checks the file.
- You may also open an issue on the project page so the maintainer can report it too.

### Company computers

On PCs managed with AppLocker or App Control for Business, programs in your user folder
(`%LOCALAPPDATA%`) are often blocked. Your IT department can allow Panelport with a rule for the
installer's file hash (from `SHA256SUMS.txt`).

## 3. First start

Panelport opens its launcher and offers to **find panels** on this computer (Docker containers
that publish web ports, and local web servers). Tick the ones you want and click **Add**. You can
also add a panel by address in **Settings**.

Closing the launcher keeps Panelport running in the system tray (bottom right, you may need to
click the ^ arrow). Right-click the tray icon for the menu, or **Quit** to exit.

## 4. Uninstall

**Settings → Apps → Installed apps → Panelport → Uninstall**. Your settings stay in
`%LOCALAPPDATA%\io.github.thealps01-netizen.panelport`; delete that folder to remove them too.

## Updates

Panelport does not update itself and does not check for updates in the background. Use
**Settings → About → Check for new versions** to open the releases page.
