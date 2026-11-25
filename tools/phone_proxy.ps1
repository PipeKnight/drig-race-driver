# Phone proxy: predicts how a source change moves the Xiaomi 13 Ultra frame from a phone-like desktop run (docs/PERFORMANCE.md "Phone proxy").
#
# Builds grid-app (feature profile by default, as the phone's profile APK) through the
# machine-wide build lock, then runs the phone's race on the desktop as the phone runs it:
# the bundle's settings.ini and Donington event.ini, the event menu's Continue by
# GRID_MENU_AUTO, autopilot, the mobile preset with 6 compute threads, 480 render rows (of the
# default 1280x800 window: builds before F906 know no [video] window/resolution keys, and both
# sides of a comparison must draw the same pixels), indirect-call validation on (as Android),
# no vsync, muted, the process
# pinned to a CPU subset (-Mask, `start /affinity`). By default three runs of the normal
# perf_test state (-States normal,shadows,empty_scene for a calibration), all under one
# build-lock hold, alternating the revisions when comparing. Then tools/phone_proxy.py maps the desktop split to the phone's with the
# calibration in tools/phone_proxy_calibration.json and prints the predicted phone split,
# the delta between revisions and the workload counters that moved, ending in a VERDICT line.
#
#   -Rev <commit>      build and run that commit (side worktree local/phone-proxy/src, exe
#                      cached in local/phone-proxy/bin by commit); default: this checkout's
#                      working tree (target/release/grid-app.exe, rebuilt).
#   -Compare <commit>  also run that commit and print the delta (Rev, or the working tree, is
#                      the base; Compare is the change).
#   -Exe / -CompareExe use built executables instead (never the owner's prototype).
#   -CheckOnly         re-analyse an earlier -OutDir without building or running.
#   -SelfTest          smoke step: analyser selftest and argument handling, no build or run.
# Never runs the main checkout's target/release/grid-app.exe.
param(
    [string]$Rev = '',
    [string]$Compare = '',
    [string]$Exe = '',
    [string]$CompareExe = '',
    [string[]]$States = @('normal'),
    # CPU set of the run (hex affinity mask; logical CPUs 0-15 are the i7-12700's P-core
    # threads, 16-19 its E-cores). F0000 = the four E-cores: the calibration's mask, and the
    # quietest on a PC shared with other jobs (docs/PERFORMANCE.md "Phone proxy").
    [string]$Mask = 'F0000',
    # Runs per state and revision; with 3 the noise limit of a comparison is about 2 ms.
    [int]$Repeat = 3,
    [int]$Ticks = 2400,
    [int]$TimeoutSec = 240,
    [string]$Features = 'profile',
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$OutDir = '',
    [string]$Calibration = '',
    [switch]$CheckOnly,
    # Smoke step without a build or a run: the analyser's selftest (fit, fixed point, log
    # parsing, report with a comparison) and this script's argument handling.
    [switch]$SelfTest,
    # Internal: the run phase, started under the build lock by this script.
    [switch]$InLock
)
$ErrorActionPreference = 'Stop'
$States = @($States | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$bad = @($States | Where-Object { $_ -notin 'normal', 'shadows', 'empty_scene' })
if ($bad.Count) { Write-Output "phone_proxy FAIL: unknown state $($bad -join ',') (normal, shadows, empty_scene)"; exit 2 }
if ($Mask -notmatch '^[0-9A-Fa-f]{1,5}$') { Write-Output "phone_proxy FAIL: -Mask $Mask is not a hex affinity mask"; exit 2 }
if ($SelfTest) {
    & py -I (Join-Path $PSScriptRoot 'phone_proxy.py') selftest
    if ($LASTEXITCODE -ne 0 -or $States.Count -ne 1) { Write-Output 'phone_proxy selftest FAIL'; exit 1 }
    Write-Output 'phone_proxy selftest PASS'; exit 0
}
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$work = Join-Path $root 'local\phone-proxy'
if (-not $OutDir) { $OutDir = Join-Path $work ('runs\' + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
$OutDir = [IO.Path]::GetFullPath($OutDir)
if (-not $Calibration) { $Calibration = Join-Path $PSScriptRoot 'phone_proxy_calibration.json' }
$common = (& git rev-parse --path-format=absolute --git-common-dir 2>$null)
$ownerExe = if ($common) { [IO.Path]::GetFullPath((Join-Path $common '..\target\release\grid-app.exe')) } else { '' }

function Get-Busy {
    $names = 'cargo', 'rustc', 'clippy-driver', 'rust-lld', 'link', 'grid-app', 'grid', 'grid-race-soak', 'grid-soak', 'gradle', 'java'
    $busy = @(Get-Process -Name $names -ErrorAction SilentlyContinue | Group-Object Name | ForEach-Object { "$($_.Name)($($_.Count))" })
    if ($busy.Count) { 'no:' + ($busy -join ',') } else { 'yes' }
}

function Invoke-Locked([string]$Dir, [int]$Minutes, [string[]]$Command) {
    Push-Location $Dir
    try {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'tools\build_lock.ps1') -HoldMinutes $Minutes @Command | Out-Host
        return $LASTEXITCODE
    } finally { Pop-Location }
}

# The executable of one side: an explicit exe, a commit (side worktree, cached), or this tree.
function Get-Exe([string]$GivenExe, [string]$Commit, [string]$Label) {
    $featArgs = if ($Features) { @('--features', $Features) } else { @() }
    $tag = if ($Features) { '-' + ($Features -replace '[^a-z0-9]', '_') } else { '' }
    if ($GivenExe) { return [IO.Path]::GetFullPath($GivenExe) }
    if (-not $Commit) {
        $code = Invoke-Locked $root 60 (@('cargo', 'build', '--release', '-p', 'grid-app') + $featArgs)
        if ($code -ne 0) { throw "build of the working tree failed ($code)" }
        return (Join-Path $root 'target\release\grid-app.exe')
    }
    $sha = (& git rev-parse --verify "$Commit^{commit}").Trim()
    $bin = Join-Path $work 'bin'
    $cached = Join-Path $bin ($sha.Substring(0, 12) + "$tag.exe")
    if (Test-Path $cached) { return $cached }
    $src = Join-Path $work 'src'
    if (-not (Test-Path (Join-Path $src '.git'))) {
        & git worktree add --detach $src $sha | Out-Host
        Push-Location $src
        try { & powershell -NoProfile -ExecutionPolicy Bypass -File tools\seed_build_dir.ps1 | Out-Host } finally { Pop-Location }
    } else {
        & git -C $src checkout --detach --quiet $sha
        if ($LASTEXITCODE -ne 0) { throw "checkout of $sha in $src failed" }
    }
    $code = Invoke-Locked $src 90 (@('cargo', 'build', '--release', '-p', 'grid-app') + $featArgs)
    if ($code -ne 0) { throw "build of $sha failed ($code)" }
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item (Join-Path $src 'target\release\grid-app.exe') $cached
    return $cached
}

# One race: the phone's data dir, the phone's knobs, pinned to $Mask; returns the exit state.
function Invoke-Race([string]$ExePath, [string]$Dir, [string]$State) {
    $data = Join-Path $Dir 'data'
    New-Item -ItemType Directory -Force $data | Out-Null
    $n = 1
    while (Test-Path (Join-Path $Dir ($(if ($n -eq 1) { 'stdout.log' } else { "stdout-$n.log" })))) { $n++ }
    $log = Join-Path $Dir ($(if ($n -eq 1) { 'stdout.log' } else { "stdout-$n.log" }))
    $settings = [IO.File]::ReadAllText((Join-Path $root 'android\app\src\main\assets\grid-import\settings.ini'))
    $settings = ($settings -replace "`r`n", "`n").TrimEnd() + "`n[game]`nroot = $($GameRoot -replace '\\', '/')`n" +
        "[osd]`nfps = on`n[sound]`neffects = 0`nspeech = 0`nmusic = 0`n" +
        "[video]`nrender_height = 480`nrender_height_version = 2`npresent_mode = auto_no_vsync`n"
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $data 'settings.ini'), $settings, $utf8)
    Copy-Item -LiteralPath (Join-Path $root 'android\events\donington.ini') -Destination (Join-Path $data 'event.ini') -Force
    Remove-Item -LiteralPath (Join-Path $data 'event-menu.ini') -ErrorAction SilentlyContinue
    $env:GRID_MOBILE_PRESET = '1'
    $env:GRID_COMPUTE_THREADS = '6'
    $env:GRID_MENU_AUTO = "$Ticks"
    $env:GRID_AUTOPILOT = '1'
    $env:WGPU_VALIDATION_INDIRECT_CALL = '1'
    if ($State -eq 'normal') { Remove-Item Env:GRID_PERF_TEST -ErrorAction SilentlyContinue } else { $env:GRID_PERF_TEST = $State }
    # A cmd redirect, not Start-Process -RedirectStandardOutput (F843: ~60 us per stdout write).
    $line = "start `"`" /b /wait /affinity $Mask `"$ExePath`" --menu --data-dir `"$data`" > `"$log`" 2> `"$log.err`""
    $p = Start-Process -FilePath cmd.exe -ArgumentList '/d', '/c', $line -WorkingDirectory $root -WindowStyle Hidden -PassThru
    $null = $p.Handle
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $state = 'timeout'
    while ((Get-Date) -lt $deadline) {
        if ($p.WaitForExit(1000)) { $state = "exit$($p.ExitCode)"; break }
        if ((Test-Path $log) -and (Select-String -LiteralPath $log -SimpleMatch 'drive_script_complete' -Quiet)) { $state = 'complete'; break }
    }
    # taskkill writes to stderr when cmd has already ended (fatal under Stop in PowerShell 5.1).
    $eap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    & taskkill /T /F /PID $p.Id 2>&1 | Out-Null
    $ErrorActionPreference = $eap
    Get-Process grid-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $ExePath } | Stop-Process -Force -ErrorAction SilentlyContinue
    foreach ($k in 'GRID_MOBILE_PRESET', 'GRID_COMPUTE_THREADS', 'GRID_MENU_AUTO', 'GRID_AUTOPILOT', 'WGPU_VALIDATION_INDIRECT_CALL', 'GRID_PERF_TEST') {
        Remove-Item "Env:$k" -ErrorAction SilentlyContinue
    }
    return $state
}

if ($InLock) {
    # Run phase (under the build lock): alternate base and change per state and repeat.
    $sides = @(@{ Name = 'base'; Exe = $Exe })
    if ($CompareExe) { $sides += @{ Name = 'change'; Exe = $CompareExe } }
    $idle = Get-Busy
    $mute = Join-Path (Split-Path $common) 'local\mute_grid.ps1'
    $results = @()
    for ($r = 0; $r -lt $Repeat; $r++) {
        foreach ($s in $States) {
            $order = if ($r % 2) { @($sides[($sides.Count - 1)..0]) } else { $sides }
            foreach ($side in $order) {
                $dir = Join-Path $OutDir "$($side.Name)\$s"
                $res = Invoke-Race $side.Exe $dir $s
                if (Test-Path $mute) { & powershell -NoProfile -ExecutionPolicy Bypass -File $mute *> $null }
                $results += "$($side.Name)/$s/$($r + 1):$res"
            }
        }
    }
    Set-Content -LiteralPath (Join-Path $OutDir 'runs.txt') -Value ("idle_start=$idle idle_end=$(Get-Busy) mask=$Mask ticks=$Ticks`n" + ($results -join "`n"))
    exit 0
}

if (-not $CheckOnly) {
    # Building the working tree of the main checkout would replace the owner's prototype.
    if (-not $Exe -and -not $Rev -and $ownerExe -and [IO.Path]::GetFullPath((Join-Path $root 'target\release\grid-app.exe')) -eq $ownerExe) {
        Write-Output 'phone_proxy FAIL: run it from a worktree or pass -Rev/-Exe (this is the main checkout)'; exit 2
    }
    New-Item -ItemType Directory -Force $OutDir | Out-Null
    $baseExe = Get-Exe $Exe $Rev 'base'
    $changeExe = if ($Compare -or $CompareExe) { Get-Exe $CompareExe $Compare 'change' } else { '' }
    foreach ($e in @($baseExe, $changeExe) | Where-Object { $_ }) {
        if ($ownerExe -and [IO.Path]::GetFullPath($e) -eq $ownerExe) { Write-Output "phone_proxy FAIL: $e is the owner's prototype"; exit 2 }
        if (-not (Test-Path $e)) { Write-Output "phone_proxy FAIL: $e is missing"; exit 2 }
    }
    $runs = $States.Count * $Repeat * ($(if ($changeExe) { 2 } else { 1 }))
    $minutes = [int][math]::Ceiling($runs * ($TimeoutSec + 15) / 60.0) + 5
    $cmd = @('powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'phone_proxy.ps1'), '-InLock',
        '-Exe', $baseExe, '-States', ($States -join ','), '-Mask', $Mask, '-Repeat', "$Repeat", '-Ticks', "$Ticks",
        '-TimeoutSec', "$TimeoutSec", '-GameRoot', $GameRoot, '-OutDir', $OutDir)
    if ($changeExe) { $cmd += @('-CompareExe', $changeExe) }
    $code = Invoke-Locked $root $minutes $cmd
    if ($code -ne 0) { Write-Output "phone_proxy FAIL: run phase exit $code (log under $OutDir)"; exit 1 }
    Set-Content -LiteralPath (Join-Path $OutDir 'exes.txt') -Value ("base=$baseExe rev=$Rev features=$Features`nchange=$changeExe rev=$Compare")
}
if (Test-Path (Join-Path $OutDir 'runs.txt')) { Get-Content (Join-Path $OutDir 'runs.txt') | ForEach-Object { "  $_" } | Select-Object -First 1 }
$fails = @(Get-Content (Join-Path $OutDir 'runs.txt') -ErrorAction SilentlyContinue | Where-Object { $_ -match ':(timeout|exit[1-9-])' })
$pyArgs = @('-I', (Join-Path $PSScriptRoot 'phone_proxy.py'), 'report', (Join-Path $OutDir 'base'), '--calibration', $Calibration)
if (Test-Path (Join-Path $OutDir 'change')) { $pyArgs += @('--compare', (Join-Path $OutDir 'change')) }
& py @pyArgs
$code = $LASTEXITCODE
foreach ($f in $fails) { Write-Output "  run problem: $f" }
Write-Output "  out: $OutDir"
if ($code -ne 0 -or $fails.Count) { exit 1 }
exit 0
