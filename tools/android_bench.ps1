# Phone GPU bench over USB with no touch (docs/ANDROID_GPU.md section 7): installs the
# profile APK, starts a scripted autopilot race with the Performance test Auto cycle, waits,
# pulls the newest phone log and prints the per-step table (phone_profile_summary.py --table).
# Every -PressSeconds it presses the race's on-screen VIEW and LOOK buttons in turn (owner,
# 2026-11-25: cameras change during the run and the screen stays awake), only while our game
# has the focus, and writes each press to presses.txt for the per-camera table.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\android_bench.ps1
#   ... -Apk local\android\apk\X.apk -Seconds 450 -Ticks 30000 -NoInstall -PerfTest auto
#   ... -NoInstall -PerfTest '' -Seconds 200 -PreRotation   (normal frames, Vulkan pre-rotation on)
#
# adb use is limited to: install -r (keeps the app data), push of two small files into the
# app's files folder (bench.ini, bench-args.txt), am start / am force-stop of our package,
# logcat, dumpsys battery/power/window (temperature, screen, focus), input swipe on the VIEW and
# LOOK touch zones while our package has the focus, pull of the new Download/GRID-Reassembly
# log, and rm of its own two files. It records only whether our game had the focus, never
# which other app had it. It never uninstalls, never changes phone settings and never deletes
# anything else: the app's data folder holds the owner's GRID copy and settings.ini.
#
# bench-args.txt is read and deleted by the game on its next start (event_menu.rs
# android_args, dev-tools builds only): --settings bench.ini and --drive-ticks N. bench.ini is
# read last, so it wins: the owner's usual race (Donington, Lacetti, 11 AI, Normal), force
# feedback off (no vibration: the phone lies on a table), autopilot and the Auto cycle. A run
# starts only at or below -MaxTempC (battery) and records the temperature before and after; it
# stops above -AbortTempC, and a run whose presses found the game without focus is not counted.
param(
    [string]$Apk = '',
    [string]$Adb = 'C:\Users\debros\Tools\android-sdk\platform-tools\adb.exe',
    [string]$Package = 'org.reassembly.grid',
    # The Auto cycle is 16 steps of 20 s from the first race tick (5 min 20 s), plus loading.
    [int]$Seconds = 450,
    # Fixed ticks of about 1/60 s; the game caps --drive-ticks at 72000.
    [int]$Ticks = 30000,
    # Developer switches for the phone, NAME=VALUE (GRID_ only), e.g. -SetEnv GRID_PERF_TIMING=1:
    # written to bench-args.txt as 'env' lines, which the dev build sets before the app starts.
    [string[]]$SetEnv = @(),
    [string]$PerfTest = 'auto',  # words for [diagnostics] perf_test; 'none' or '' writes none
    [int]$Laps = 10,
    [int]$Opponents = 11,
    # Fixed render rows for the whole run, so every step and A/B side renders the same pixels (the phone's
    # dynamic rows move between 480 and 720 with the frame time and blur the comparison). '' keeps the
    # phone's own setting (auto). The 720-row step still sets 720 itself.
    [string]$RenderHeight = '480',
    [double]$MaxTempC = 40,
    # The run stops (force-stop, VERDICT fail, exit 4) when the battery passes this during a run.
    [double]$AbortTempC = 45,
    # Camera presses: every -PressSeconds from the start, VIEW (Change View) and LOOK (Look Back,
    # held -LookSeconds) in turn, on the touch zones of touch_controls.rs for a -Screen landscape
    # panel. 0 sends none.
    [int]$PressSeconds = 30,
    [int]$LookSeconds = 5,
    [string]$Screen = '2308x1080',
    [int]$CoolMinutes = 15,
    [switch]$NoInstall,
    # [diagnostics] pre_rotation = on: Vulkan swapchain pre-rotation (third_party/wgpu-hal, F927).
    [switch]$PreRotation,
    # [diagnostics] sustained_performance = on: Window.setSustainedPerformanceMode and the
    # GameManager game state at start-up (thermal soaks); the log line says if the phone supports it.
    [switch]$Sustained,
    # [video] refresh_hz: the frame cap and display-rate hint (60, 90, 120 or system; F927 pacing A/B).
    [string]$RefreshHz = '',
    # Free text for run.txt, such as the phone's case (owner, 2026-11-25: runs before 18:50 were in the case).
    [string]$Note = '',
    [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$files = "/sdcard/Android/data/$Package/files"
$logs = '/sdcard/Download/GRID-Reassembly'
if ($Ticks -gt 72000) { throw "-Ticks $Ticks is above the game's cap of 72000" }

function Invoke-Adb([string[]]$a) {
    # Windows PowerShell 5.1 turns adb's stderr progress lines into errors under 'Stop'.
    $ErrorActionPreference = 'Continue'
    $text = & $Adb @a 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) { throw "adb $($a -join ' ') failed: $text" }
    $text
}

function Get-TempC {
    $line = (& $Adb shell dumpsys battery) -match 'temperature:' | Select-Object -First 1
    if ($line -match '(\d+)') { [double]$Matches[1] / 10 } else { -1 }
}

# 1. A phone must be connected and authorised.
$devices = (& $Adb devices) -split "`n" | Where-Object { $_ -match "`tdevice\s*$" }
if (@($devices).Count -ne 1) {
    Write-Output "VERDICT fail: need exactly one authorised phone (adb devices shows $(@($devices).Count)); enable USB debugging and accept this PC's prompt"
    exit 2
}
$installed = (Invoke-Adb @('shell', 'pm', 'list', 'packages', $Package)) -match "package:$Package\b"
# A locked or dark phone stops the game at once (onStop) and the run records nothing. The game
# keeps the screen on while it runs, but adb may not wake or unlock the phone: the owner does.
$awake = (Invoke-Adb @('shell', 'dumpsys', 'power')) -match 'mWakefulness=Awake'
$locked = (Invoke-Adb @('shell', 'dumpsys', 'window')) -match 'isKeyguardShowing=true'
if (-not $awake -or $locked) {
    Write-Output "VERDICT fail: the phone screen is off or locked (awake=$([bool]$awake), keyguard=$([bool]$locked)); unlock it and leave the screen on"
    exit 2
}

# 2. Install (keeps the data folder) or reuse the installed build.
if (-not $NoInstall) {
    if (-not $Apk) {
        $Apk = Get-ChildItem (Join-Path $root 'local\android\apk\*profile*.apk') -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $Apk -or -not (Test-Path $Apk)) { Write-Output 'VERDICT fail: no profile APK (build_android.ps1 -Profile)'; exit 2 }
    Write-Output "install $Apk"
    Invoke-Adb @('install', '-r', $Apk) | Out-Null
} elseif (-not $installed) {
    Write-Output "VERDICT fail: $Package is not installed and -NoInstall was given"; exit 2
}

# 3. Bench files into the app's own folder.
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
if (-not $Out) { $Out = Join-Path $root "local\android-bench\$stamp" }
New-Item -ItemType Directory -Force $Out | Out-Null
$ini = Join-Path $Out 'bench.ini'
$argsFile = Join-Path $Out 'bench-args.txt'
$benchIni = @(
    '# tools/android_bench.ps1: the owner''s Donington race, unattended.',
    '[drive]', 'enabled = on', "laps = $Laps", "opponents = $Opponents", 'grid_position = 4',
    'ai_characteristics = ai-driver-characteristics.tsv',
    '[ai]', 'difficulty = 0.606', 'catchup = on', 'choreographer = on', 'mistakes = on',
    '[input]', 'force_feedback = off',
    '[diagnostics]', 'autopilot = on'
)
if ($PerfTest -and $PerfTest -ne 'none') { $benchIni += "perf_test = $PerfTest" }  # 'none': the build's own default profile
if ($PreRotation) { $benchIni += 'pre_rotation = on' }
if ($Sustained) { $benchIni += 'sustained_performance = on' }
$video = @()
if ($RenderHeight) { $video += @("render_height = $RenderHeight", 'render_height_version = 2') }
if ($RefreshHz) { $video += @("refresh_hz = $RefreshHz", 'refresh_hz_version = 1') }
if ($video) { $benchIni += @('[video]') + $video }
[IO.File]::WriteAllText($ini, ($benchIni -join "`n") + "`n")
$envLines = ($SetEnv | Where-Object { $_ } | ForEach-Object { "env $_`n" }) -join ''  # developer switches, e.g. GRID_PERF_TIMING=1
[IO.File]::WriteAllText($argsFile, "--settings`n$files/bench.ini`n--drive-ticks`n$Ticks`n$envLines")
Invoke-Adb @('push', $ini, "$files/bench.ini") | Out-Null
Invoke-Adb @('push', $argsFile, "$files/bench-args.txt") | Out-Null

# 4. Cool down, note the newest log, start, wait, stop.
$temp = Get-TempC
$cool = [Diagnostics.Stopwatch]::StartNew()
while ($temp -gt $MaxTempC -and $cool.Elapsed.TotalMinutes -lt $CoolMinutes) {
    Start-Sleep -Seconds 60
    $temp = Get-TempC
}
if ($temp -gt $MaxTempC) {
    Invoke-Adb @('shell', 'rm', '-f', "$files/bench.ini", "$files/bench-args.txt") | Out-Null
    Write-Output "VERDICT fail: battery $temp C above $MaxTempC C after $CoolMinutes min"; exit 3
}
$tempStart = $temp
$before = (Invoke-Adb @('shell', "ls -t $logs 2>/dev/null | head -1")).Trim()
Invoke-Adb @('shell', 'am', 'force-stop', $Package) | Out-Null
Invoke-Adb @('logcat', '-c') | Out-Null
Invoke-Adb @('shell', 'am', 'start', '-n', "$Package/.LauncherActivity") | Out-Null
Write-Output "started at $(Get-Date -Format HH:mm:ss), battery $tempStart C; waiting $Seconds s"
$clock = [Diagnostics.Stopwatch]::StartNew()
$presses = Join-Path $Out 'presses.txt'
Set-Content -Path $presses -Value '# seconds since am start, action, result (touch_controls.rs zones; game log time ~= these seconds)'
if ($PressSeconds -gt 0) {
    $w, $h = $Screen -split 'x' | ForEach-Object { [int]$_ }
    # Zone centres: VIEW (0.44, 0.93), LOOK (0.56, 0.93) of the window. A tap ends inside one frame
    # and Bevy would miss it, so VIEW is held 150 ms and LOOK -LookSeconds.
    $zones = @{ view = @([int]($w * 0.44), [int]($h * 0.93), 150); look = @([int]($w * 0.56), [int]($h * 0.93), ($LookSeconds * 1000)) }
    $n = 0
    while ($true) {
        $next = ($n + 1) * $PressSeconds
        if ($next -ge $Seconds - 2) { break }
        $wait = $next - $clock.Elapsed.TotalSeconds
        if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]($wait * 1000)) }
        $action = if ($n % 2 -eq 0) { 'view' } else { 'look' }
        $n++
        $at = '{0:F1}' -f $clock.Elapsed.TotalSeconds
        $now = Get-TempC
        if ($now -gt $AbortTempC) {
            Invoke-Adb @('shell', 'am', 'force-stop', $Package) | Out-Null
            Invoke-Adb @('shell', 'rm', '-f', "$files/bench.ini", "$files/bench-args.txt") | Out-Null
            Add-Content -Path $presses -Value "$at abort battery_c=$now"
            Write-Output "VERDICT fail: battery $now C above $AbortTempC C at $at s; run stopped, not counted"; exit 4
        }
        $focus = (Invoke-Adb @('shell', 'dumpsys', 'window')) -split "`n" | Where-Object { $_ -match 'mCurrentFocus=' } | Select-Object -First 1
        if ($focus -notmatch [regex]::Escape($Package)) {
            Add-Content -Path $presses -Value "$at $action skipped_not_focused"
            continue
        }
        $z = $zones[$action]
        Invoke-Adb @('shell', 'input', 'swipe', $z[0], $z[1], $z[0], $z[1], $z[2]) | Out-Null
        Add-Content -Path $presses -Value "$at $action sent hold_ms=$($z[2])"
    }
}
$rest = $Seconds - $clock.Elapsed.TotalSeconds
if ($rest -gt 0) { Start-Sleep -Milliseconds ([int]($rest * 1000)) }
Invoke-Adb @('shell', 'am', 'force-stop', $Package) | Out-Null
$tempEnd = Get-TempC
& $Adb logcat -d -s 'grid:*' 'AndroidRuntime:E' > (Join-Path $Out 'logcat.txt') 2>$null
Invoke-Adb @('shell', 'rm', '-f', "$files/bench.ini", "$files/bench-args.txt") | Out-Null

# 5. Pull the log written by this run.
$after = (Invoke-Adb @('shell', "ls -t $logs 2>/dev/null | head -1")).Trim()
if (-not $after -or $after -eq $before) {
    Write-Output "VERDICT fail: no new log in $logs (logcat: $Out\logcat.txt)"; exit 1
}
$log = Join-Path $Out $after
Invoke-Adb @('pull', "$logs/$after", $log) | Out-Null
Set-Content -Path (Join-Path $Out 'run.txt') -Value @(
    "apk=$Apk", "seconds=$Seconds", "ticks=$Ticks", "perf_test=$PerfTest", "pre_rotation=$PreRotation", "sustained=$Sustained", "render_height=$RenderHeight", "refresh_hz=$RefreshHz",
    "temp_start_c=$tempStart", "temp_end_c=$tempEnd", "note=$Note", "log=$after")
$seen = Select-String -Path $log -Pattern 'android_bench args|drive_autopilot enabled|drive_force_feedback|drive_opponents|render_pre_rotation|android_sustained_performance|android_frame_cap|android_frame_rate|android_display_hz|mobile_preset|event=step step=1 ' |
    Select-Object -First 14 | ForEach-Object { $_.Line }
$seen | ForEach-Object { Write-Output "  $_" }
Write-Output "battery: start $tempStart C, end $tempEnd C"
$pressLines = @(Get-Content $presses | Where-Object { $_ -notmatch '^#' })
Write-Output "camera presses: $(@($pressLines | Where-Object { $_ -match ' sent' }).Count) sent, $(@($pressLines | Where-Object { $_ -match 'skipped' }).Count) skipped ($presses)"
& py -I (Join-Path $PSScriptRoot 'phone_profile_summary.py') --table $log --presses $presses
if (-not ($seen -match 'android_bench args')) { Write-Output "VERDICT fail: the game did not read bench-args.txt ($log)"; exit 1 }
if (-not ($seen -match 'drive_force_feedback off')) { Write-Output "VERDICT fail: force feedback was not off ($log)"; exit 1 }
if ($RefreshHz -match '^\d+$' -and -not ($seen -match "android_frame_cap hz=Some\($RefreshHz(\.0)?\)")) { Write-Output "VERDICT fail: the frame cap is not $RefreshHz Hz ($log)"; exit 1 }
if ($PreRotation -and -not ($seen -match 'render_pre_rotation on')) { Write-Output "VERDICT fail: pre-rotation was not switched on ($log)"; exit 1 }
if ($Sustained -and -not ($seen -match 'android_sustained_performance on supported=')) { Write-Output "VERDICT fail: sustained performance was not requested ($log)"; exit 1 }
$skipped = @($pressLines | Where-Object { $_ -match 'skipped' }).Count
if ($skipped -gt 1) { Write-Output "VERDICT fail: $skipped presses found the game without focus (phone taken or locked?); not counted ($presses)"; exit 5 }
Write-Output "VERDICT pass: battery $tempStart -> $tempEnd C, log $log"
