# Interactive menu-loop smoke: a launcher race, its results, back to the event menu, a menu race, and quit (docs/SCREEN_FLOW.md).
#
# Runs the release grid-app in real windows, unattended: GRID_AUTOPILOT drives
# 1-lap races with 3 opponents, GRID_RESULTS_CONTINUE leaves the results, and
# GRID_MENU_AUTO presses Continue in the first menu round and Back in the
# next. Sound is 0 in a scratch data dir under local/. Expects, in order:
#   race (launcher) -> results -> back_to_menu -> hand-off -> menu -> Go ->
#   hand-off -> race -> results -> back_to_menu -> hand-off -> menu -> quit,
#   exit 0, and the menu's front-end sound source (F772) on the way.
# -WindowShots (GRID_WINDOW_SHOTS, default 1.5 s) makes every screen save a
# window shot into the data dir; each of the 4 screens must save one. The
# extra system in `Last` once kept the window hidden with no frames drawn
# (the F773 window-show hang, reported by career-ui).
# Prints a verdict and the log path. Run it under tools\build_lock.ps1 only
# when it also builds; this script does not build.
param(
    [string]$Exe = 'target\release\grid-app.exe',
    [string]$OutDir = '',
    [int]$Ticks = 36000,
    [int]$TimeoutSec = 900,
    [string]$WindowShots = '1.5'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if (-not (Test-Path $Exe)) { Write-Output "FAIL: $Exe is missing (build grid-app --release first)"; exit 2 }
if (-not $OutDir) { $OutDir = Join-Path 'local\menu-loop' (Get-Date -Format 'yyyyMMdd-HHmmss') }
$data = Join-Path $OutDir 'data'
New-Item -ItemType Directory -Force $data | Out-Null
$utf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText((Join-Path $data 'settings.ini'), "[event]`ntype = race`nlaps = 1`nopponents = 3`n[sound]`neffects = 0`nspeech = 0`nmusic = 0`n", $utf8)
$raceIni = Join-Path $OutDir 'race.ini'
[IO.File]::WriteAllText($raceIni, "[drive]`nenabled = on`nlaps = 1`nopponents = 3`ngrid_position = back`n[private]`nbody_contacts = derive`nbody_events = derive`ndeformation = derive`n", $utf8)
$log = Join-Path $OutDir 'menu-loop.log'
$err = Join-Path $OutDir 'menu-loop.err.log'

$env:GRID_AUTOPILOT = '1'
$env:GRID_RESULTS_CONTINUE = '3'
$env:GRID_MENU_AUTO = "$Ticks"
$env:GRID_MENU_AUTO_RACES = '1'
$env:GRID_WINDOW_SHOTS = $WindowShots
$appArgs = @('--settings', $raceIni, '--data-dir', $data, '--drive-ticks', "$Ticks")
$started = Get-Date
$p = Start-Process -FilePath $Exe -ArgumentList $appArgs -RedirectStandardOutput $log -RedirectStandardError $err -PassThru -NoNewWindow
$null = $p.Handle  # keeps ExitCode readable after the exit
if (-not $p.WaitForExit($TimeoutSec * 1000)) {
    # The supervisor and its children: stop the whole tree.
    & taskkill /T /F /PID $p.Id | Out-Null
    $code = 'timeout'
} else {
    $code = $p.ExitCode
}
$seconds = [int]((Get-Date) - $started).TotalSeconds

# The flow, in order; each step must follow the previous one.
$steps = @(
    @('launcher race fades in', 'reason=race_start'),
    @('launcher race back to menu', 'drive_event_menu version=1 event=back_to_menu'),
    @('launcher race hands off', 'event=handoff code=21'),
    @('menu opens', 'drive_event_menu version=1 event=open'),
    @('menu sound starts', 'hud_audio version=1 event=queued .*cues=menu,screen'),
    @('menu shows', 'event=handoff_ready'),
    @('launcher race exits', 'event=exit next=Code\(21\) waited='),
    @('menu Go', 'drive_event_menu version=1 event=go '),
    @('menu hands off', 'event=handoff code=20'),
    @('race shows', 'event=handoff_ready'),
    @('menu race fades in', 'reason=race_start'),
    @('menu race back to menu', 'drive_event_menu version=1 event=back_to_menu'),
    @('race hands off', 'event=handoff code=21'),
    @('menu shows', 'event=handoff_ready'),
    @('menu fades in again', 'reason=screen_open'),
    @('menu quits', 'drive_event_menu version=1 event=quit'),
    @('menu closes', 'event=exit next=Close')
)
$lines = if (Test-Path $log) { Get-Content $log } else { @() }
$at = 0
$missing = $null
foreach ($s in $steps) {
    $found = $false
    for ($i = $at; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $s[1]) { $at = $i + 1; $found = $true; break }
    }
    if (-not $found) { $missing = $s[0]; break }
}
$waits = @($lines | Select-String 'event=exit next=Code\(\d+\) waited=([\d.]+)' | ForEach-Object { $_.Matches[0].Groups[1].Value })
# One shot per screen: launcher race, menu, menu race, menu.
# Bevy logs the saved shot to stderr.
$errLines = if (Test-Path $err) { Get-Content $err } else { @() }
$shots = @(@($lines) + @($errLines) | Select-String 'Screenshot saved').Count
$shotsOk = (-not $WindowShots) -or ($shots -ge 4)
$ok = ($code -eq 0) -and (-not $missing) -and $shotsOk
$verdict = if ($ok) { 'PASS' } else { 'FAIL' }
Write-Output "$verdict menu-loop: exit=$code seconds=$seconds steps=$($steps.Count) missing=$(if ($missing) { $missing } else { 'none' }) handoff_waits=$($waits -join ',') window_shots=$shots"
Write-Output "log: $log"
if (-not $ok) { exit 1 }
