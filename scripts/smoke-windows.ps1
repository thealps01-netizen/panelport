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

# 6. Stop and uninstall silently.
Stop-Process -Id $app.Id -Force
Start-Sleep -Seconds 2
$un = Join-Path $dir 'uninstall.exe'
if (Test-Path $un) {
    $p = Start-Process -FilePath $un -ArgumentList '/S' -Wait -PassThru
    Write-Host "uninstall exit code $($p.ExitCode)"
}
Write-Host 'SMOKE TEST PASSED'
