# Interactive career-loop smoke: GRID World -> driver offer -> race -> rewards -> GRID World -> quit (docs/CAREER_SCREEN.md).
#
# Runs the release grid-app --career in real windows, unattended, on a fresh
# career in a scratch data dir under local/ (sound 0): GRID_CAREER_AUTO=offer
# walks GRID World -> Driver Offers -> the first offer -> take it,
# GRID_AUTOPILOT drives the race, GRID_RESULTS_CONTINUE leaves the results,
# and the screen that comes back pages through the rewards and quits.
# -Shots S1,S2,... passes GRID_WINDOW_SHOTS: every process saves window shots
# at those seconds to the data dir, the last one (the rewards screen) wins.
# -PagesOnly runs only the career screen (no supervisor, no race), so its
# shots show the pages before the race; it ends with the hand-off code 22.
# -Season runs only the career screen on a seeded season (GRID_CAREER_SEED=
# season, GRID_CAREER_AUTO=season): team name, Team Branding, the Le Mans
# choice, Skip Le Mans, End Of Season, Start Of Season, quit (exit 0).
# The career screen's front-end sound source must start (the F772 sink);
# settings.ini keeps it silent. Prints a verdict and the log path. This
# script does not build.
param(
    [string]$Exe = 'target\release\grid-app.exe',
    [string]$OutDir = '',
    [string]$Auto = 'offer',
    [string]$Shots = '',
    [switch]$PagesOnly,
    [switch]$Season,
    [int]$TimeoutSec = 900
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if (-not (Test-Path $Exe)) { Write-Output "FAIL: $Exe is missing (build grid-app --release first)"; exit 2 }
if (-not $OutDir) { $OutDir = Join-Path 'local\career-loop' (Get-Date -Format 'yyyyMMdd-HHmmss') }
$data = Join-Path $OutDir 'data'
New-Item -ItemType Directory -Force $data | Out-Null
$utf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText((Join-Path $data 'settings.ini'), "[sound]`neffects = 0`nspeech = 0`nmusic = 0`n", $utf8)
$log = Join-Path $OutDir 'career-loop.log'
$err = Join-Path $OutDir 'career-loop.err.log'

if ($Season) { $Auto = 'season'; $env:GRID_CAREER_SEED = 'season' } else { Remove-Item Env:GRID_CAREER_SEED -ErrorAction SilentlyContinue }
$env:GRID_CAREER_AUTO = $Auto
$env:GRID_AUTOPILOT = '1'
$env:GRID_RESULTS_CONTINUE = '3'
if ($Shots) { $env:GRID_WINDOW_SHOTS = $Shots } else { Remove-Item Env:GRID_WINDOW_SHOTS -ErrorAction SilentlyContinue }
$appArgs = if ($PagesOnly -or $Season) { @('--career-screen', '--data-dir', $data) } else { @('--career', '--data-dir', $data) }
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
$pages = @(
    @('career opens', 'drive_career version=1 event=open '),
    @('sounds start', 'hud_audio version=1 event=queued .*cues=menu,screen'),
    @('screen fades in', 'reason=screen_open'),
    @('driver offers page', 'drive_career version=1 event=page page=driveroffers'),
    @('offer details page', 'drive_career version=1 event=page page=offerdetails'),
    @('offer taken', 'drive_career version=1 event=offer_taken '),
    @('screen fades out', 'reason=career_go')
)
# A leaving screen logs its hand-off before its own exit; the next process
# logs its start-up lines before its window shows (handoff_ready).
$alone = @(@('screen exits', 'event=exit next=Code\(22\)'))
$loop = @(
    @('screen hands off', 'event=handoff code=22'),
    @('race shows', 'event=handoff_ready'),
    @('race fades in', 'reason=race_start'),
    @('race hands back', 'event=handoff code=21'),
    @('race added', 'drive_career version=1 event=race_added '),
    @('offer paid', 'drive_career version=1 event=offer_paid '),
    @('career shows', 'event=handoff_ready'),
    @('rewards fade in', 'reason=screen_open'),
    @('rewards page', 'drive_career version=1 event=page page=rewards'),
    @('career quits', 'drive_career version=1 event=quit'),
    @('career closes', 'event=exit next=Close')
)
$seasonSteps = @(
    @('career opens', 'drive_career version=1 event=open '),
    @('sounds start', 'hud_audio version=1 event=queued .*cues=menu,screen'),
    @('team named', 'drive_career version=1 event=team_named name="My Team"'),
    @('my team page', 'drive_career version=1 event=page page=team'),
    @('team branding page', 'drive_career version=1 event=page page=livery'),
    @('livery kept', 'drive_career version=1 event=team_livery '),
    @('le mans choice', 'drive_career version=1 event=page page=lemans'),
    @('skip confirm', 'drive_career version=1 event=page page=confirm_skip_lemans'),
    @('le mans refused', 'drive_career version=1 event=lemans_refused '),
    @('end of season', 'drive_career version=1 event=page page=season_end'),
    @('driver rankings', 'drive_career version=1 event=page page=rankings:drivers:'),
    @('team rankings', 'drive_career version=1 event=page page=rankings:teams:'),
    @('season ended', 'drive_career version=1 event=season_ended season=2 '),
    @('start of season', 'drive_career version=1 event=page page=season_start'),
    @('grid world', 'drive_career version=1 event=page page=home'),
    @('career quits', 'drive_career version=1 event=quit'),
    @('career closes', 'event=exit next=Close')
)
$steps = if ($Season) { $seasonSteps } elseif ($PagesOnly) { $pages + $alone } else { $pages + $loop }
$expected = if ($PagesOnly -and -not $Season) { 22 } else { 0 }
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
$paid = @($lines | Select-String 'event=offer_paid .* money=(-?\d+)' | ForEach-Object { $_.Matches[0].Groups[1].Value }) | Select-Object -Last 1
$rewards = @($lines | Select-String 'event=page page=rewards' ).Count
$shotFiles = @(Get-ChildItem $data -Filter 'window-shot-*.png' -ErrorAction SilentlyContinue).Count
$ok = ($code -eq $expected) -and (-not $missing)
$verdict = if ($ok) { 'PASS' } else { 'FAIL' }
$mode = if ($Season) { 'season' } elseif ($PagesOnly) { 'pages' } else { 'loop' }
Write-Output "$verdict career-$mode`: exit=$code seconds=$seconds steps=$($steps.Count) missing=$(if ($missing) { $missing } else { 'none' }) money_after=$paid reward_pages=$rewards shots=$shotFiles"
Write-Output "log: $log"
if (-not $ok) { exit 1 }
