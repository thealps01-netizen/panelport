# Windows screen run (CI only, on a disposable runner). Drives a build made with the test-only
# `e2e` feature through every screen in real WebView2, takes screenshots at 1440x900 and 1000x700
# (dark, plus one light shot), and checks the custom title bar with real Win32 input: hit test
# over the caption buttons, caption clicks, double-click to maximize, Win+Up / Win+Down, Alt+F4,
# and the Snap Layouts hover. Test panels are local throwaway pages, clearly named as samples.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$OutDir
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path
$Results = Join-Path $OutDir 'results.txt'
Set-Content $Results ''
function Pass([string]$m) { Add-Content $Results "PASS $m"; Write-Host "PASS $m" }
function Fail([string]$m) { Add-Content $Results "FAIL $m"; Write-Host "FAIL $m" }
function Check([string]$name, [bool]$ok) { if ($ok) { Pass $name } else { Fail $name } }
# On any unexpected error: keep the app's log for diagnosis, then fail.
$script:prof = $null
function SaveLogs([string]$suffix) {
    if (-not $script:prof) { return }
    Get-ChildItem (Join-Path $script:prof 'logs') -Filter 'panelport*.log' -ErrorAction SilentlyContinue |
        ForEach-Object { Copy-Item $_.FullName (Join-Path $OutDir "panelport-$suffix.log") -Force }
}
trap { Write-Host "ERROR: $_"; SaveLogs 'error'; exit 1 }

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class W {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr h, int i);
  public static IntPtr Find(int pid, string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      int p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h)) return true;
      var sb = new StringBuilder(256); GetWindowText(h, sb, 256);
      if (sb.ToString() == title) { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  public static string Class(IntPtr h) { var sb = new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
  public static void Click(int x, int y, int times) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(120);
    for (int i = 0; i < times; i++) { mouse_event(2, 0, 0, 0, UIntPtr.Zero); mouse_event(4, 0, 0, 0, UIntPtr.Zero); System.Threading.Thread.Sleep(60); }
  }
  public static void Keys(byte[] down) {
    foreach (var k in down) keybd_event(k, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(80);
    for (int i = down.Length - 1; i >= 0; i--) keybd_event(down[i], 0, 2, UIntPtr.Zero);
  }
}
"@

# Bigger desktop for 1440x900 windows when the runner allows it.
try { Set-DisplayResolution -Width 1920 -Height 1080 -Force -ErrorAction Stop; Write-Host 'display: 1920x1080' } catch { Write-Host "display unchanged: $($_.Exception.Message)" }
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
Write-Host "screen: $($screen.Width)x$($screen.Height)"

# Best effort for the Snap Layouts capture: the flyout setting may be off on server images.
try {
    $adv = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced'
    Set-ItemProperty -Path $adv -Name EnableSnapAssistFlyout -Value 1 -Type DWord -ErrorAction Stop
    Set-ItemProperty -Path $adv -Name EnableSnapBar -Value 1 -Type DWord -ErrorAction SilentlyContinue
} catch { Write-Host "snap layout setting not changed: $($_.Exception.Message)" }

function Set-Theme([bool]$light) {
    $k = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
    if (-not (Test-Path $k)) { New-Item -Path $k -Force | Out-Null }
    $v = if ($light) { 1 } else { 0 }
    Set-ItemProperty -Path $k -Name AppsUseLightTheme -Value $v -Type DWord
    Set-ItemProperty -Path $k -Name SystemUsesLightTheme -Value $v -Type DWord
}

# ---- Throwaway panels ---------------------------------------------------------------------
$work = Join-Path $env:RUNNER_TEMP 'pp-screens'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
$site = Join-Path $work 'site'; $e2e = Join-Path $work 'e2e'; $prof = Join-Path $work 'profile'; $script:prof = $prof
New-Item -ItemType Directory -Force -Path $site, $e2e, (Join-Path $prof 'data') | Out-Null
Set-Content (Join-Path $site 'index.html') -Encoding utf8 -Value @'
<!doctype html><html><head><title>Sample panel</title><style>
body{margin:0;font:15px/1.5 Segoe UI,sans-serif;background:#f4f6f8;color:#1b2430}
header{background:#24364d;color:#fff;padding:14px 24px;font-weight:600}
main{display:grid;grid-template-columns:repeat(3,1fr);gap:16px;padding:24px}
.c{background:#fff;border-radius:10px;padding:18px;box-shadow:0 1px 3px rgba(0,0,0,.1)}
.c b{display:block;font-size:28px}</style></head><body>
<header>Sample panel (CI test page)</header><main>
<div class="c">Containers<b>12</b></div><div class="c">Images<b>31</b></div><div class="c">Volumes<b>7</b></div>
<div class="c">Networks<b>4</b></div><div class="c">Stacks<b>3</b></div><div class="c">Updates<b>2</b></div>
</main></body></html>
'@
$py = (Get-Command python -ErrorAction SilentlyContinue).Source
$srv = Start-Process -FilePath $py -ArgumentList '-m', 'http.server', '8765', '--bind', '127.0.0.1', '--directory', "`"$site`"" -PassThru -WindowStyle Hidden
$api = Start-Process -FilePath $py -ArgumentList "`"$(Join-Path $PSScriptRoot '..\tests\e2e\api401.py')`"", '62310' -PassThru -WindowStyle Hidden
$settings = @{
    schema = 1; discovery_offered = $true; close_hint_shown = $true
    panels = @(
        @{ id = 'psample'; name = 'Sample panel'; url = 'http://localhost:8765/'; health_path = '/'; icon = 'monogram' },
        @{ id = 'poffline'; name = 'Offline panel'; url = 'http://localhost:3999/'; health_path = '/'; icon = 'monogram' },
        @{ id = 'pdaemon'; name = 'Daemon API'; url = 'http://localhost:62310/'; health_path = '/'; icon = 'monogram' }
    )
}
$settings | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $prof 'data\settings.json') -Encoding utf8

# ---- Harness ------------------------------------------------------------------------------
$script:n = 0
function Req($obj) {
    $script:n++
    $name = "r$($script:n)"
    $tmp = Join-Path $e2e "$name.tmp"
    ($obj | ConvertTo-Json -Compress -Depth 5) | Set-Content -NoNewline -Encoding utf8 $tmp
    Move-Item $tmp (Join-Path $e2e "$name.json")
    $out = Join-Path $e2e "$name.out"
    for ($i = 0; $i -lt 150; $i++) { if (Test-Path $out) { $r = Get-Content -Raw $out; Remove-Item $out; return $r }; Start-Sleep -Milliseconds 100 }
    return 'TIMEOUT'
}
function Js([string]$label, [string]$code) { Req @{ op = 'eval'; label = $label; js = $code } | Out-Null }
function UrlOf([string]$label) { Req @{ op = 'url'; label = $label } }
function Probe([string]$label, [string]$expr) {
    Js $label "Promise.resolve().then(() => ($expr)).then(v => { location.hash = 'e2e=' + encodeURIComponent(JSON.stringify(v)); }, e => { location.hash = 'e2e=' + encodeURIComponent(JSON.stringify({error: String(e && e.message || e)})); })"
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 200
        $u = UrlOf $label
        if ($u -match '#e2e=(.*)$') { Js $label "history.replaceState(null, '', location.pathname + location.search)"; return [uri]::UnescapeDataString($Matches[1]) }
    }
    return 'NOHASH'
}
function Size([string]$label, [int]$w, [int]$h) { Req @{ op = 'size'; label = $label; w = $w; h = $h } | Out-Null; Start-Sleep -Milliseconds 900 }

function Shot([IntPtr]$hwnd, [string]$name) {
    Start-Sleep -Milliseconds 1200
    [W]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 500
    $r = New-Object W+RECT
    [W]::GetWindowRect($hwnd, [ref]$r) | Out-Null
    $w = $r.R - $r.L; $h = $r.B - $r.T
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $fits = $r.L -ge 0 -and $r.T -ge 0 -and $r.R -le $screen.Width -and $r.B -le $screen.Height
    if ($fits) {
        $g.CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $h))
    } else {
        $hdc = $g.GetHdc(); [W]::PrintWindow($hwnd, $hdc, 2) | Out-Null; $g.ReleaseHdc($hdc)
    }
    $g.Dispose()
    $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "shot $name ($w x $h, $(if ($fits) { 'screen' } else { 'PrintWindow' }))"
}
function ScreenShot([string]$name) {
    $bmp = New-Object System.Drawing.Bitmap $screen.Width, $screen.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen(0, 0, 0, 0, $bmp.Size); $g.Dispose()
    $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}

function Start-App {
    $env:PANELPORT_PROFILE_DIR = $prof
    $env:PANELPORT_E2E_DIR = $e2e
    $env:PANELPORT_LANG = 'tr'
    $p = Start-Process -FilePath $Exe -PassThru
    Remove-Item Env:PANELPORT_PROFILE_DIR, Env:PANELPORT_E2E_DIR, Env:PANELPORT_LANG
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 90 -and $h -eq [IntPtr]::Zero; $i++) { Start-Sleep 1; $h = [W]::Find($p.Id, 'Panelport') }
    return @{ proc = $p; hwnd = $h }
}

# Caption button centre in screen pixels: index 0 = minimize, 1 = maximize, 2 = close.
function CaptionPoint([IntPtr]$hwnd, [int]$index) {
    $r = New-Object W+RECT
    [W]::GetWindowRect($hwnd, [ref]$r) | Out-Null
    $s = [W]::GetDpiForWindow($hwnd) / 96.0
    $bw = [int][math]::Round(46 * $s); $bh = [int][math]::Round(44 * $s)
    $x = $r.R - (3 - $index) * $bw + [int]($bw / 2)
    $y = $r.T + [int]($bh / 2)
    return @($x, $y)
}

# ---- Dark run -----------------------------------------------------------------------------
Set-Theme $false
$app = Start-App
Check 'launcher window appears' ($app.hwnd -ne [IntPtr]::Zero)
$L = $app.hwnd
Start-Sleep 6

# Panel window with a real preview first (the launcher stage shows it afterwards).
Req @{ op = 'open'; id = 'psample' } | Out-Null
$sample = [IntPtr]::Zero
for ($i = 0; $i -lt 30 -and $sample -eq [IntPtr]::Zero; $i++) { Start-Sleep 1; $sample = [W]::Find($app.proc.Id, 'Sample panel') }
Check 'panel window appears' ($sample -ne [IntPtr]::Zero)
if ($sample -eq [IntPtr]::Zero) { SaveLogs 'error'; throw 'the panel window did not appear' }
$info = Req @{ op = 'winfo'; label = 'pwin-psample' } | ConvertFrom-Json
Check "panel window holds two webviews ($($info.webviews -join ','))" ((@($info.webviews) | Sort-Object) -join ',' -eq 'bar-psample,panel-psample')
Size 'pwin-psample' 1440 900
Start-Sleep 5
Shot $sample 'win-panel-window-1440'
$ipc = Probe 'panel-psample' "(window.__TAURI_INTERNALS__ ? Promise.all(['bar_info','win_close','list_panels'].map(c => window.__TAURI_INTERNALS__.invoke(c).then(() => 'ALLOWED', () => 'denied'))) : 'no-bridge')"
Add-Content (Join-Path $OutDir 'ipc-probe.txt') "panel origin in the two-webview window: $ipc"
Check "panel page in the two-webview window is refused bar_info, win_close, list_panels ($ipc)" ($ipc -notmatch 'ALLOWED')

# Native caption buttons of the panel window: hit test, then the real maximize click.
$pt = CaptionPoint $sample 1
$hit = [W]::WindowFromPoint((New-Object W+POINT -Property @{ X = $pt[0]; Y = $pt[1] }))
$cls = [W]::Class($hit)
$code = [W]::SendMessage($hit, 0x84, [IntPtr]::Zero, [IntPtr](($pt[1] -shl 16) -bor ($pt[0] -band 0xffff))).ToInt64()
Add-Content (Join-Path $OutDir 'hit-test.txt') "panel window maximize button: window class $cls, WM_NCHITTEST = $code"
Check "window under the maximize glyph is the caption hit area ($cls)" ($cls -eq 'PANELPORT_CAPTION_BUTTONS')
Check "WM_NCHITTEST over the maximize glyph answers HTMAXBUTTON (got $code)" ($code -eq 9)
[W]::SetCursorPos($pt[0] - 6, $pt[1] - 4) | Out-Null
Start-Sleep -Milliseconds 300
for ($i = 0; $i -lt 6; $i++) { [W]::SetCursorPos($pt[0] + ($i % 2), $pt[1]) | Out-Null; Start-Sleep -Milliseconds 150 }
Start-Sleep -Milliseconds 2500
ScreenShot 'win-snap-layouts-hover'
[W]::Click($pt[0] + 60, $pt[1] + 300, 1) | Out-Null   # move away and dismiss a flyout, if any
Start-Sleep 1
[W]::Click($pt[0], $pt[1], 1)
Start-Sleep 2
Check 'clicking the drawn maximize button maximizes the panel window' ([W]::IsZoomed($sample))
$pt = CaptionPoint $sample 1
[W]::Click($pt[0], $pt[1], 1)
Start-Sleep 2
Check 'clicking it again restores the window' (-not [W]::IsZoomed($sample))

# Double-click on empty title bar space maximizes; again restores.
$r = New-Object W+RECT; [W]::GetWindowRect($sample, [ref]$r) | Out-Null
$s = [W]::GetDpiForWindow($sample) / 96.0
$dx = $r.L + [int](($r.R - $r.L) * 0.55); $dy = $r.T + [int](22 * $s)
[W]::Click($dx, $dy, 2); Start-Sleep 2
Check 'double-click on the title bar maximizes' ([W]::IsZoomed($sample))
[W]::GetWindowRect($sample, [ref]$r) | Out-Null
$dx = $r.L + [int](($r.R - $r.L) * 0.55); $dy = $r.T + [int](22 * $s)
[W]::Click($dx, $dy, 2); Start-Sleep 2
Check 'double-click again restores' (-not [W]::IsZoomed($sample))

# Win+Up / Win+Down (system shortcuts).
[W]::SetForegroundWindow($sample) | Out-Null; Start-Sleep 1
[W]::Keys([byte[]](0x5B, 0x26)); Start-Sleep 2
Check 'Win+Up maximizes the frameless window' ([W]::IsZoomed($sample))
[W]::Keys([byte[]](0x5B, 0x28)); Start-Sleep 2
Check 'Win+Down restores it' (-not [W]::IsZoomed($sample))
$style = [W]::GetWindowLong($sample, -16)
Add-Content (Join-Path $OutDir 'hit-test.txt') ("panel window style 0x{0:X8}" -f $style)

# Launcher screens.
Size 'launcher' 1440 900
Js 'launcher' "show('panels'); select('psample')"
Start-Sleep 4
Shot $L 'win-launcher-1440'
Js 'launcher' "select('poffline')"; Start-Sleep 3
Shot $L 'win-launcher-down-1440'
Js 'launcher' "select('pdaemon')"; Start-Sleep 3
Shot $L 'win-launcher-notpage-1440'
Js 'launcher' "show('find')"; Start-Sleep 12
Shot $L 'win-find-1440'
$skip = Probe 'launcher' "L.find.skipped.map(s => s.port + ':' + s.reason).join(',')"
Check "Find panels lists 62310 as skipped ($skip)" ($skip -match '62310:401')
Js 'launcher' "show('settings')"; Start-Sleep 3
Shot $L 'win-settings-1440'
Size 'launcher' 1000 700
Js 'launcher' "show('panels'); select('psample')"; Start-Sleep 3
Shot $L 'win-launcher-1000'
Js 'launcher' "show('find')"; Start-Sleep 3
Shot $L 'win-find-1000'
Js 'launcher' "show('settings')"; Start-Sleep 3
Shot $L 'win-settings-1000'
Size 'launcher' 900 600
Js 'launcher' "show('panels')"; Start-Sleep 3
Shot $L 'win-launcher-900'

# Launcher caption hit test and the close button (hides to tray, process keeps running).
$pt = CaptionPoint $L 1
$hit = [W]::WindowFromPoint((New-Object W+POINT -Property @{ X = $pt[0]; Y = $pt[1] }))
$code = [W]::SendMessage($hit, 0x84, [IntPtr]::Zero, [IntPtr](($pt[1] -shl 16) -bor ($pt[0] -band 0xffff))).ToInt64()
Add-Content (Join-Path $OutDir 'hit-test.txt') "launcher maximize button: window class $([W]::Class($hit)), WM_NCHITTEST = $code"
Check "launcher: WM_NCHITTEST over the maximize glyph answers HTMAXBUTTON (got $code)" ($code -eq 9)

# Down page in the offline panel's window.
Req @{ op = 'open'; id = 'poffline' } | Out-Null
$off = [IntPtr]::Zero
for ($i = 0; $i -lt 30 -and $off -eq [IntPtr]::Zero; $i++) { Start-Sleep 1; $off = [W]::Find($app.proc.Id, 'Offline panel') }
Size 'pwin-poffline' 1440 900; Start-Sleep 4
Shot $off 'win-down-page-1440'
Size 'pwin-poffline' 1000 700; Start-Sleep 3
Shot $off 'win-down-page-1000'
# Alt+F4 closes the focused panel window.
[W]::SetForegroundWindow($off) | Out-Null; Start-Sleep 1
[W]::Keys([byte[]](0x12, 0x73)); Start-Sleep 2
Check 'Alt+F4 closes the panel window' (-not [W]::IsWindow($off))
# The drawn close button closes the sample panel window.
$pt = CaptionPoint $sample 2
[W]::Click($pt[0], $pt[1], 1); Start-Sleep 2
Check 'the drawn close button closes the panel window' (-not [W]::IsWindow($sample))

$log = Get-ChildItem (Join-Path $prof 'logs') -Filter 'panelport*.log' -ErrorAction SilentlyContinue | Select-Object -First 1
if ($log) { Copy-Item $log.FullName (Join-Path $OutDir 'panelport-dark.log') }
Stop-Process -Id $app.proc.Id -Force
Start-Sleep 2

# ---- Light run: one shot of the launcher ---------------------------------------------------
Set-Theme $true
$app = Start-App
Start-Sleep 5
Size 'launcher' 1440 900
Js 'launcher' "show('panels'); select('psample')"; Start-Sleep 4
Shot $app.hwnd 'win-launcher-light-1440'
Stop-Process -Id $app.proc.Id -Force
Set-Theme $false
Stop-Process -Id $srv.Id, $api.Id -Force -ErrorAction SilentlyContinue

$fails = @(Select-String -Path $Results -Pattern '^FAIL').Count
$passes = @(Select-String -Path $Results -Pattern '^PASS').Count
Write-Host "$passes passed, $fails failed"
if ($fails -gt 0) { exit 1 }
