# Clean-machine smoke test for the Windows installer (run by CI on a fresh runner that has no
# source tree and no build tools). Installs silently per user, checks the shipped files, runs
# the headless self-test, starts the real app, waits for its window, takes a screenshot, checks
# single-instance behaviour and the log, then uninstalls.
param(
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$OutDir
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path

function Fail([string]$msg) { Write-Host "FAIL: $msg"; exit 1 }

# 1. Silent per-user install (no admin rights needed).
$p = Start-Process -FilePath $Installer -ArgumentList '/S' -Wait -PassThru
if ($p.ExitCode -ne 0) { Fail "installer exit code $($p.ExitCode)" }
$dir = Join-Path $env:LOCALAPPDATA 'Panelport'
if (-not (Test-Path $dir)) { Fail "install folder $dir not found" }
$exe = Get-ChildItem $dir -Filter *.exe | Where-Object { $_.Name -notlike 'uninstall*' } | Select-Object -First 1
if (-not $exe) { Fail "no program in $dir" }
Get-ChildItem $dir | Format-Table Name, Length | Out-String | Write-Host
foreach ($f in 'THIRD_PARTY_LICENSES.txt', 'LICENSE.txt') {
    if (-not (Test-Path (Join-Path $dir $f))) { Fail "$f missing from the install folder" }
}
Write-Host "installed: $($exe.FullName)"

# 2. Headless self-test.
$json = Join-Path $OutDir 'self-test.json'
$p = Start-Process -FilePath $exe.FullName -ArgumentList '--self-test', "`"$json`"" -Wait -PassThru
Get-Content $json | Write-Host
if ($p.ExitCode -ne 0) { Fail "self-test exit code $($p.ExitCode)" }
# Panel windows must have WebView2's password saving and form autofill switched on.
$st = Get-Content $json -Raw | ConvertFrom-Json
$pw = $st.checks | Where-Object name -eq 'panel_password_autosave'
if (-not $pw -or -not $pw.ok -or $pw.detail.IsPasswordAutosaveEnabled -ne $true -or $pw.detail.IsGeneralAutofillEnabled -ne $true) {
    Fail "panel_password_autosave read-back is not true: $($pw | ConvertTo-Json -Compress)"
}
Write-Host "password autosave read-back: IsPasswordAutosaveEnabled=$($pw.detail.IsPasswordAutosaveEnabled) IsGeneralAutofillEnabled=$($pw.detail.IsGeneralAutofillEnabled)"

# 3. Start the real app and wait for its window.
$app = Start-Process -FilePath $exe.FullName -PassThru
$title = ''
for ($i = 0; $i -lt 90 -and $title -ne 'Panelport'; $i++) {
    Start-Sleep -Seconds 1
    $proc = Get-Process -Id $app.Id -ErrorAction SilentlyContinue
    if (-not $proc) { Fail 'the app exited during start-up' }
    $title = $proc.MainWindowTitle
}
if ($title -ne 'Panelport') { Fail "no 'Panelport' window after 90 s (title: '$title')" }
Start-Sleep -Seconds 8

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
$bmp.Save((Join-Path $OutDir 'launcher.png'), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()

# 4. Single instance: a second start must hand over and exit; the first keeps running.
$second = Start-Process -FilePath $exe.FullName -PassThru
if (-not $second.WaitForExit(20000)) { Fail 'second instance did not exit' }
if (-not (Get-Process -Id $app.Id -ErrorAction SilentlyContinue)) { Fail 'first instance died' }
$count = @(Get-Process -Name $exe.BaseName -ErrorAction SilentlyContinue).Count
Write-Host "processes named $($exe.BaseName): $count"

# 5. Log: written under the per-user app folder, no errors.
$log = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'io.github.thealps01-netizen.panelport') -Recurse -Filter 'panelport*.log' -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $log) { Fail 'no log file under LOCALAPPDATA' }
Copy-Item $log.FullName (Join-Path $OutDir 'panelport.log')
Get-Content $log.FullName | Write-Host
if (Select-String -Path $log.FullName -Pattern '\[ERROR\]' -Quiet) { Fail 'the log contains errors' }

# 6. Stop the app.
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 2

# 7. Save-password probe (best effort, never fails the job): a throwaway login page on this
#    runner, throwaway credentials, an isolated Panelport profile and WebView2 data folder.
try {
    $probe = Join-Path $env:RUNNER_TEMP 'pp-pwprobe'
    $site = Join-Path $probe 'site'
    New-Item -ItemType Directory -Force -Path (Join-Path $probe 'data'), $site | Out-Null
    Set-Content (Join-Path $site 'index.html') -Encoding utf8 -Value '<!doctype html><html><head><title>Throwaway login</title></head><body><h1>Throwaway login</h1><form action="/welcome.html" method="get"><input id="u" name="username" autocomplete="username"><input id="p" name="password" type="password" autocomplete="current-password"><button id="b" type="submit">Sign in</button></form></body></html>'
    Set-Content (Join-Path $site 'welcome.html') -Encoding utf8 -Value '<!doctype html><html><head><title>Signed in</title></head><body><h1>Signed in</h1></body></html>'
    $settings = @{ schema = 1; discovery_offered = $true; close_hint_shown = $true; panels = @(@{ id = 'pprobelogin'; name = 'Throwaway login'; url = 'http://127.0.0.1:8765/'; health_path = '/'; icon = 'monogram' }) }
    $settings | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $probe 'data\settings.json') -Encoding utf8
    $srv = Start-Process -FilePath python -ArgumentList '-m', 'http.server', '8765', '--bind', '127.0.0.1', '--directory', "`"$site`"" -PassThru -WindowStyle Hidden
    Add-Type -Path (Join-Path $PSScriptRoot 'smoke-cdp.cs')
    $env:PANELPORT_PROFILE_DIR = $probe
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
    $pp = Start-Process -FilePath $exe.FullName -PassThru
    Remove-Item Env:PANELPORT_PROFILE_DIR, Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
    $launcher = $null
    for ($i = 0; $i -lt 60 -and -not $launcher; $i++) { Start-Sleep 1; try { $launcher = [Cdp]::FindTarget(9333, 'launcher.html') } catch {} }
    if (-not $launcher) { throw 'no DevTools target for the launcher' }
    Start-Sleep 4
    # The bundled launcher page opens the panel exactly as a click on its card does.
    [Cdp]::Send($launcher, 'Runtime.evaluate', '{"expression":"invoke(\"open_panel\",{id:\"pprobelogin\"}).then(()=>\"ok\",e=>JSON.stringify(e))","awaitPromise":true}') | Write-Host
    $panel = $null
    for ($i = 0; $i -lt 30 -and -not $panel; $i++) { Start-Sleep 1; try { $panel = [Cdp]::FindTarget(9333, '127.0.0.1:8765') } catch {} }
    if (-not $panel) { throw 'no DevTools target for the panel window' }
    Start-Sleep 3
    # Type like a user (Input.insertText), then press Enter to submit.
    [Cdp]::Send($panel, 'Runtime.evaluate', '{"expression":"document.getElementById(\"u\").focus()"}') | Out-Null
    [Cdp]::Send($panel, 'Input.insertText', '{"text":"throwaway-user"}') | Out-Null
    [Cdp]::Send($panel, 'Runtime.evaluate', '{"expression":"document.getElementById(\"p\").focus()"}') | Out-Null
    [Cdp]::Send($panel, 'Input.insertText', '{"text":"Throwaway-Only-1234"}') | Out-Null
    [Cdp]::Send($panel, 'Input.dispatchKeyEvent', '{"type":"keyDown","key":"Enter","code":"Enter","windowsVirtualKeyCode":13,"text":"\r"}') | Out-Null
    [Cdp]::Send($panel, 'Input.dispatchKeyEvent', '{"type":"keyUp","key":"Enter","code":"Enter","windowsVirtualKeyCode":13}') | Out-Null
    Start-Sleep 5
    $after = [Cdp]::Send($panel, 'Runtime.evaluate', '{"expression":"location.pathname + \" | \" + document.title"}')
    Write-Host "after submit: $after"
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save((Join-Path $OutDir 'save-password-probe.png'), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host 'save-password probe: screenshot taken (save-password-probe.png)'
    Stop-Process -Id $pp.Id -Force -ErrorAction SilentlyContinue
    Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
} catch {
    Write-Host "save-password probe could not run: $_"
}

# 8. Uninstall silently.
Start-Sleep -Seconds 2
$un = Join-Path $dir 'uninstall.exe'
if (Test-Path $un) {
    $p = Start-Process -FilePath $un -ArgumentList '/S' -Wait -PassThru
    Write-Host "uninstall exit code $($p.ExitCode)"
}
Write-Host 'SMOKE TEST PASSED'
