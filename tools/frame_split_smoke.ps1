# Frame-split smoke: an interactive windowed race with the fps log, checks the frame_stats split fields (docs/PERFORMANCE.md "Frame split").
#
# Runs the release grid-app in a real window (not minimized, not offscreen):
# the autopilot drives the player's car for -Ticks simulation ticks of the
# given settings (default: the Detroit derby, 12 cars), with [osd] fps = on and
# sound 0 in a scratch data dir under local/. Then it checks the log:
#   - the `frame_split version=1` start line (adapter, timestamp features);
#   - every driving frame_stats line carries the main, render, GPU and count
#     fields;
#   - main_ms + outside_ms matches avg_ms (the main thread's whole frame);
#   - the render stages sum to render_ms and render_ms fits in the frame;
#   - with timestamp queries: gpu=on, no invalid intervals, GPU detail lines
#     (frame_gpu_views) with the 3D view's passes and the sun cascades.
# Prints a verdict, the mean split, the worst windows (min fps, frames over
# 10 ms, the slowest window's split), whether the machine was idle (no cargo,
# rustc or other grid process at the start and the end) and the log path.
# Never runs the owner's
# prototype path. -CheckOnly re-checks the log of an earlier run in -OutDir.
param(
    [string]$Exe = 'target\release\grid-app.exe',
    [string]$Settings = 'settings\demolition-derby.ini',
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$OutDir = '',
    [int]$Ticks = 2500,
    [int]$TimeoutSec = 420,
    # Render height for the run; below the window height the 3D image is upscaled by the
    # 2D display camera (the Mac's usual path), 'native' renders straight to the window.
    [string]$RenderHeight = '720',
    # Extra settings.ini text for the data dir.
    [string]$Extra = '',
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$exePath = [IO.Path]::GetFullPath($Exe)
$common = (& git rev-parse --path-format=absolute --git-common-dir 2>$null)
if ($common) {
    $owner = [IO.Path]::GetFullPath((Join-Path $common '..\target\release\grid-app.exe'))
    if ($exePath -eq $owner) { Write-Output "FAIL: $exePath is the owner's prototype; run from a worktree build"; exit 2 }
}
if (-not $CheckOnly -and -not (Test-Path $exePath)) { Write-Output "FAIL: $Exe is missing (build grid-app --release first)"; exit 2 }
if (-not $OutDir) { $OutDir = Join-Path 'local\frame-split-smoke' (Get-Date -Format 'yyyyMMdd-HHmmss') }
$data = Join-Path $OutDir 'data'
$log = Join-Path $OutDir 'stdout.log'
$err = Join-Path $OutDir 'stderr.log'

# Other builds or game processes share the CPU and memory: name them.
function Get-Busy {
    $names = 'cargo', 'rustc', 'clippy-driver', 'rust-lld', 'link', 'grid-app', 'grid', 'grid-race-soak', 'grid-soak', 'gradle', 'java'
    $busy = @(Get-Process -Name $names -ErrorAction SilentlyContinue | Group-Object Name | ForEach-Object { "$($_.Name)($($_.Count))" })
    if ($busy.Count) { 'no:' + ($busy -join ',') } else { 'yes' }
}
$idleStart = if ($CheckOnly) { 'unknown' } else { Get-Busy }
if ($CheckOnly) { $code = 0 } else {
    New-Item -ItemType Directory -Force $data | Out-Null
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $data 'settings.ini'),
        "[osd]`nfps = on`n[sound]`neffects = 0`nspeech = 0`nmusic = 0`n[front_end]`nshow = off`n[diagnostics]`nautopilot = on`nresults_continue = 3`n[video]`nrender_height = $RenderHeight`nrender_height_version = 2`n$Extra", $utf8)
    $env:GRID_AUTOPILOT = '1'
    $appArgs = @('--settings', $Settings, '--data-dir', $data, '--game-root', "`"$GameRoot`"", '--drive-ticks', "$Ticks")
    $p = Start-Process -FilePath $exePath -ArgumentList $appArgs -RedirectStandardOutput $log -RedirectStandardError $err -PassThru
    $null = $p.Handle
    # Like the gate: the drive ends with drive_script_complete; stop the tree 2 s later.
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $code = 'timeout'
    while ((Get-Date) -lt $deadline) {
        if ($p.WaitForExit(1000)) { $code = $p.ExitCode; break }
        if ((Test-Path $log) -and (Select-String -LiteralPath $log -SimpleMatch 'drive_script_complete' -Quiet)) {
            if (-not $p.WaitForExit(2000)) { & taskkill /T /F /PID $p.Id | Out-Null }
            $code = 0
            break
        }
    }
if ($code -eq 'timeout') { & taskkill /T /F /PID $p.Id | Out-Null }
}

$fails = New-Object System.Collections.Generic.List[string]
if (-not (Test-Path $log)) { Write-Output "FAIL: no log at $log"; exit 1 }
$lines = Get-Content -LiteralPath $log | ForEach-Object { $_ -replace '\x1b\[[0-9;]*m', '' }
$start = $lines | Where-Object { $_ -like 'frame_split version=1 *' } | Select-Object -First 1
if (-not $start) { $fails.Add('no frame_split start line') }
$timestamps = $start -match ' timestamp_query=true '
$gpuExpected = $start -match ' gpu_markers=on '

function Get-Fields([string]$line) {
    $h = @{}
    foreach ($m in [regex]::Matches($line, '(\w+)=(-?[0-9.]+)\b')) { $h[$m.Groups[1].Value] = [double]$m.Groups[2].Value }
    $h['gpu_state'] = if ($line -match ' gpu=(\w+)') { $Matches[1] } else { '' }
    return $h
}
$stats = @($lines | Where-Object { $_ -like 'frame_stats *' -and $_ -match ' sim_ticks_per_s=[0-9.]+ ' } | ForEach-Object { Get-Fields $_ })
# The first driving windows hold the loading tail: skip two.
$drive = @($stats | Select-Object -Skip 2)
if ($drive.Count -lt 5) { $fails.Add("only $($drive.Count) driving frame_stats windows after the first two") }
$required = 'main_ms', 'fixed_ms', 'outside_ms', 'extract_ms', 'r_meshes', 'r_views', 'acquire', 'r_queue', 'r_sort',
    'r_prepare', 'r_bind', 'r_encode', 'r_submit', 'r_present', 'r_cleanup', 'render_ms', 'render_frames',
    'd_opaque', 'd_alpha', 'd_shadow', 'd_transp', 'd_ui', 'shadow_views', 'vis', 'casters', 'mesh_up_s', 'mesh_kb_s', 'mesh_stream_kb_s'
$missing = @($required | Where-Object { $k = $_; @($drive | Where-Object { -not $_.ContainsKey($k) }).Count -gt 0 })
if ($missing.Count) { $fails.Add("frame_stats lack: $($missing -join ',')") }

function Mean([string]$k) {
    $v = @($drive | Where-Object { $_.ContainsKey($k) } | ForEach-Object { $_[$k] })
    if ($v.Count -eq 0) { return [double]::NaN }
    return ($v | Measure-Object -Average).Average
}
$avg = Mean 'avg_ms'
$mainSum = (Mean 'main_ms') + (Mean 'outside_ms')
$stageSum = 0.0
foreach ($k in 'r_meshes', 'r_views', 'acquire', 'r_queue', 'r_sort', 'r_prepare', 'r_bind', 'r_encode', 'r_submit', 'r_present', 'r_cleanup') { $stageSum += Mean $k }
$render = Mean 'render_ms'
if ($drive.Count -ge 5) {
    if ([math]::Abs($mainSum - $avg) -gt [math]::Max(0.5, 0.1 * $avg)) { $fails.Add(("main_ms+outside_ms {0:N2} != avg_ms {1:N2}" -f $mainSum, $avg)) }
    if ([math]::Abs($stageSum - $render) -gt 0.05 + 0.02 * $render) { $fails.Add(("render stages sum {0:N2} != render_ms {1:N2}" -f $stageSum, $render)) }
    if ($render -gt 1.1 * $avg + 0.5) { $fails.Add(("render_ms {0:N2} > frame {1:N2}" -f $render, $avg)) }
}
$gpuLine = ''
$views = @($lines | Where-Object { $_ -like 'frame_gpu_views *' })
if ($gpuExpected) {
    $off = @($drive | Where-Object { $_.gpu_state -ne 'on' }).Count
    if ($off -gt 0) { $fails.Add("gpu not on in $off of $($drive.Count) windows") }
    $bad = ($drive | ForEach-Object { if ($_.ContainsKey('g_bad')) { $_['g_bad'] } else { 0 } } | Measure-Object -Sum).Sum
    if ($bad -gt 0) { $fails.Add("g_bad=$bad invalid GPU intervals") }
    $v3 = @($views | Where-Object { $_ -match ' kind=3d ' -and $_ -match ' shadow_c0=' })
    if ($v3.Count -eq 0) { $fails.Add('no frame_gpu_views line of a 3D view with cascades') }
    $upscaled = @($lines | Where-Object { $_ -like 'render_scale *mode=upscaled-image*' }).Count -gt 0
    $v2 = @($views | Where-Object { $_ -match ' kind=2d ' -and $_ -notmatch ' g_ui=0\.000 g_upscale=0\.000' })
    if ($upscaled -and $v2.Count -eq 0) { $fails.Add('upscaled run without a timed frame_gpu_views line of the 2D display view') }
    $gpuLine = (" gpu_ms={0:N2} g_pre={1:N2} g_shadow={2:N2} g_opaque={3:N2} g_transp={4:N2} g_post={5:N2} g_ui={6:N2} g_upscale={7:N2} g_gap={8:N2}" -f
        (Mean 'gpu_ms'), (Mean 'g_pre'), (Mean 'g_shadow'), (Mean 'g_opaque'), (Mean 'g_transp'), (Mean 'g_post'), (Mean 'g_ui'), (Mean 'g_upscale'), (Mean 'g_gap'))
}
if (-not $CheckOnly -and -not @($lines | Where-Object { $_ -like 'drive_script_complete *' }).Count) { $fails.Add('no drive_script_complete') }
if ($code -eq 'timeout') { $fails.Add("no exit after $TimeoutSec s") }
elseif ($code -ne 0) { $fails.Add("exit code $code") }
$panic = $lines | Where-Object { $_ -match 'panicked at' } | Select-Object -First 1
if ($panic) { $fails.Add("panic: $panic") }

$idleEnd = if ($CheckOnly) { 'unknown' } else { Get-Busy }

# Worst windows: the lowest fps, and the share of frames over 10 ms (100 fps)
# and 16.7 ms (60 fps) from each window's slow-frame counts.
$dips = ''
if ($drive.Count -ge 5) {
    $sorted = @($drive | Sort-Object { $_['fps'] })
    $w = $sorted[0]
    $frames = ($drive | ForEach-Object { $_['frames'] } | Measure-Object -Sum).Sum
    $slow = if ($drive[0].ContainsKey('over10')) {
        $o10 = ($drive | ForEach-Object { $_['over10'] } | Measure-Object -Sum).Sum
        $o16 = ($drive | ForEach-Object { $_['over16'] } | Measure-Object -Sum).Sum
        $p99 = ($drive | ForEach-Object { $_['p99_ms'] } | Measure-Object -Maximum).Maximum
        " over10={0:N2}% over16={1:N2}% p99_max_ms={2:N2}" -f (100.0 * $o10 / [math]::Max(1, $frames)), (100.0 * $o16 / [math]::Max(1, $frames)), $p99
    } else { '' }
    # 1 % low: the frame time that 1 % of all driving frames reach or exceed, from the
    # summed ft_hist buckets (lower_edge_ms:count), as fps at the bucket's middle.
    $hist = @{}
    $histLines = @($lines | Where-Object { $_ -like 'frame_stats *' -and $_ -match ' sim_ticks_per_s=[0-9.]+ ' } | Select-Object -Skip 2)
    foreach ($l in $histLines) {
        if ($l -match ' ft_hist=([0-9.:,]+)') {
            foreach ($pair in $Matches[1].Split(',')) { $e, $n = $pair.Split(':'); $hist[[double]$e] = $hist[[double]$e] + [int]$n }
        }
    }
    if ($hist.Count) {
        $total = ($hist.Values | Measure-Object -Sum).Sum
        $need = [math]::Ceiling($total / 100.0)
        $acc = 0
        foreach ($e in @($hist.Keys | Sort-Object -Descending)) {
            $acc += $hist[$e]
            if ($acc -ge $need) { break }
        }
        $edgesMs = @(0, 5, 5.5, 6, 6.5, 7, 7.5, 8, 8.5, 9, 9.5, 10, 10.5, 11, 11.5, 12, 12.5, 13, 13.5, 14, 14.5, 15, 15.5, 16, 16.5, 17, 17.5, 18, 18.5, 19, 19.5, 20, 25, 33.3, 50)
        $hi = @($edgesMs | Where-Object { $_ -gt $e } | Select-Object -First 1)
        $hi = if ($hi.Count) { [double]$hi[0] } else { 60.0 }
        $slow += (" low1_fps={0:N1} (bucket {1}-{2} ms of {3} frames)" -f (2000.0 / ($e + $hi)), $e, $hi, $total)
    }
    $below100 = @($drive | Where-Object { $_['fps'] -lt 100 }).Count
    $dips = ("  worst: min_fps={0:N1} p10_fps={1:N1} windows_below_100={2}/{3}{4}" -f $w['fps'], $sorted[[int][math]::Floor($drive.Count / 10)]['fps'], $below100, $drive.Count, $slow)
    $dips += "`n" + ("  slowest window: fps={0:N1} avg_ms={1:N2} max_ms={2:N2} main_ms={3:N2} fixed_ms={4:N2} render_ms={5:N2} r_meshes={6:N2} r_encode={7:N2} r_prepare={8:N2} mesh_up_s={9:N0} mesh_kb_s={10:N0}" -f
        $w['fps'], $w['avg_ms'], $w['max_ms'], $w['main_ms'], $w['fixed_ms'], $w['render_ms'], $w['r_meshes'], $w['r_encode'], $w['r_prepare'], $w['mesh_up_s'], $w['mesh_kb_s'])
}

$verdict = if ($fails.Count) { 'FAIL' } else { 'PASS' }
$mode = ($lines | Where-Object { $_ -like 'render_scale height=*' } | Select-Object -Last 1) -replace '^render_scale ', ''
Write-Output ("frame_split_smoke {0} windows={1} timestamps={2} gpu_markers={3} {4}" -f $verdict, $drive.Count, [bool]$timestamps, [bool]$gpuExpected, $mode)
Write-Output ("  fps={0:N1} avg_ms={1:N2} main_ms={2:N2} fixed_ms={3:N2} outside_ms={4:N2} extract_ms={5:N2} render_ms={6:N2} r_encode={7:N2} r_submit={8:N2} r_present={9:N2} acquire={10:N2}" -f
    (Mean 'fps'), $avg, (Mean 'main_ms'), (Mean 'fixed_ms'), (Mean 'outside_ms'), (Mean 'extract_ms'), $render, (Mean 'r_encode'), (Mean 'r_submit'), (Mean 'r_present'), (Mean 'acquire'))
if ($gpuLine) { Write-Output "  $gpuLine" }
if ($dips) { Write-Output $dips }
Write-Output "  idle: start=$idleStart end=$idleEnd"
Write-Output ("  d_opaque={0:N0} d_alpha={1:N0} d_shadow={2:N0} d_transp={3:N0} shadow_views={4:N1} vis={5:N0} casters={6:N0} mesh_up_s={7:N1} mesh_kb_s={8:N0} mesh_stream_kb_s={9:N0}" -f
    (Mean 'd_opaque'), (Mean 'd_alpha'), (Mean 'd_shadow'), (Mean 'd_transp'), (Mean 'shadow_views'), (Mean 'vis'), (Mean 'casters'), (Mean 'mesh_up_s'), (Mean 'mesh_kb_s'), (Mean 'mesh_stream_kb_s'))
if ($start) { Write-Output "  $start" }
foreach ($f in $fails) { Write-Output "  fail: $f" }
Write-Output "  log: $log"
if ($fails.Count) { exit 1 }
exit 0
