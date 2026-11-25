# Steam Deck estimate: runs the phone bench's race on CPU/GPU stand-ins of this PC and scales it to the Deck (docs/STEAM_DECK_ESTIMATE.md).
#
# Each configuration runs the race of tools/android_bench.ps1 (F927) on the desktop: Donington,
# 10 laps, 11 AI opponents from grid position 4, autopilot, the bundle's settings.ini, the
# phone bench's camera presses ([diagnostics] camera_presses = 30,5: Change View and Look Back
# in turn every 30 s), --drive-ticks 30000 (the bench's length), a 1280x800 window without
# vsync, muted, force feedback off. Per configuration it sets, per process only:
#   - the CPU set with `start /affinity` (P = the P-core threads, E = the E-cores, both read from
#     GetSystemCpuSetInformation's efficiency classes, not assumed);
#   - the GPU with WGPU_BACKEND=vulkan and WGPU_ADAPTER_NAME (Bevy's adapter-by-name);
#   - the preset: desktop defaults (native rows) or GRID_MOBILE_PRESET=1 with 480 rows (the phone
#     bench's pinned height).
# No power plan, driver or Windows graphics setting is touched. The run phase holds the build
# lock (tools/build_lock.ps1) so no cargo build overlaps; tools/deck_estimate.py then checks
# from each log that the intended adapter and CPU set were used, prints the per-configuration
# table and the Deck scaling, and ends with one VERDICT line.
#
#   -Exe PATH       grid-app to run (default: this worktree's target\release\grid-app.exe);
#                   never the main checkout's prototype.
#   -Configs LIST   subset of the configurations (names below), comma-separated.
#   -CheckOnly      re-analyse -OutDir without running.
#   -SelfTest       smoke step: analyser selftest, topology read and argument handling, no run.
param(
    [string]$Exe = '',
    [string[]]$Configs = @(),
    [int]$Ticks = 30000,
    [int]$TimeoutSec = 600,
    [string]$Presses = '30,5',
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$OutDir = '',
    [switch]$CheckOnly,
    [switch]$SelfTest,
    # Internal: the run phase, started under the build lock by this script.
    [switch]$InLock
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# name = cpu, gpu, preset, rows ('' = the preset's own height).
$all = [ordered]@{
    'p-rtx-desktop'    = @('P', 'rtx 4070', 'desktop', '')
    'p-rtx-mobile'     = @('P', 'rtx 4070', 'mobile', '480')
    'e-rtx-desktop'    = @('E', 'rtx 4070', 'desktop', '')
    'e-rtx-mobile'     = @('E', 'rtx 4070', 'mobile', '480')
    'p-uhd-desktop'    = @('P', 'uhd graphics 770', 'desktop', '')
    'p-uhd-desktop480' = @('P', 'uhd graphics 770', 'desktop', '480')
    'p-uhd-mobile'     = @('P', 'uhd graphics 770', 'mobile', '480')
    'e-uhd-desktop'    = @('E', 'uhd graphics 770', 'desktop', '')
    'e-uhd-mobile'     = @('E', 'uhd graphics 770', 'mobile', '480')
}
$Configs = @($Configs | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if (-not $Configs.Count) { $Configs = @($all.Keys) }
$bad = @($Configs | Where-Object { -not $all.Contains($_) })
if ($bad.Count) { Write-Output "deck_estimate FAIL: unknown configuration $($bad -join ',') ($($all.Keys -join ', '))"; exit 2 }

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class CpuSets {
  [DllImport("kernel32.dll", SetLastError = true)]
  static extern bool GetSystemCpuSetInformation(IntPtr info, int len, out int returned, IntPtr process, int flags);
  // "index:efficiency_class:core" per logical CPU of group 0 (SYSTEM_CPU_SET_INFORMATION).
  public static string[] Read() {
    int n; GetSystemCpuSetInformation(IntPtr.Zero, 0, out n, IntPtr.Zero, 0);
    IntPtr buf = Marshal.AllocHGlobal(n);
    try {
      if (!GetSystemCpuSetInformation(buf, n, out n, IntPtr.Zero, 0)) return new string[0];
      var list = new System.Collections.Generic.List<string>();
      for (int off = 0; off < n;) {
        int size = Marshal.ReadInt32(buf, off);
        if (size <= 0) break;
        if (Marshal.ReadInt32(buf, off + 4) == 0 && Marshal.ReadInt16(buf, off + 12) == 0)
          list.Add(Marshal.ReadByte(buf, off + 14) + ":" + Marshal.ReadByte(buf, off + 18) + ":" + Marshal.ReadByte(buf, off + 15));
        off += size;
      }
      return list.ToArray();
    } finally { Marshal.FreeHGlobal(buf); }
  }
}
'@
# The masks of the two CPU sets from the efficiency classes: E = the lowest class, P = the highest.
function Get-Topology {
    $cpus = @([CpuSets]::Read() | ForEach-Object { $i, $c, $core = $_ -split ':'; [pscustomobject]@{ Index = [int]$i; Class = [int]$c; Core = [int]$core } })
    $lo = ($cpus | Measure-Object Class -Minimum).Minimum
    $hi = ($cpus | Measure-Object Class -Maximum).Maximum
    $mask = { param($set) $m = [int64]0; foreach ($c in $set) { $m = $m -bor ([int64]1 -shl $c.Index) }; '{0:X}' -f $m }
    $e = @($cpus | Where-Object Class -eq $lo); $p = @($cpus | Where-Object Class -eq $hi)
    [pscustomobject]@{
        Text = "logical=$($cpus.Count) classes=$lo..$hi E=[$(($e.Index) -join ',')] P=[$(($p.Index) -join ',')] E_cores=$(@($e.Core | Sort-Object -Unique).Count) P_cores=$(@($p.Core | Sort-Object -Unique).Count)"
        E = (& $mask $e); P = (& $mask $p); Hybrid = ($lo -ne $hi)
    }
}

$work = Join-Path $root 'local\deck-estimate'
if (-not $OutDir) { $OutDir = Join-Path $work ('runs\' + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
$OutDir = [IO.Path]::GetFullPath($OutDir)
$common = (& git rev-parse --path-format=absolute --git-common-dir 2>$null)
$ownerExe = if ($common) { [IO.Path]::GetFullPath((Join-Path (Split-Path $common) 'target\release\grid-app.exe')) } else { '' }
if (-not $Exe) { $Exe = Join-Path $root 'target\release\grid-app.exe' }
$Exe = [IO.Path]::GetFullPath($Exe)
$py = Join-Path $PSScriptRoot 'deck_estimate.py'

if ($SelfTest) {
    & py -I $py selftest
    $t = Get-Topology
    Write-Output "topology $($t.Text) E_mask=$($t.E) P_mask=$($t.P)"
    if ($LASTEXITCODE -ne 0 -or -not $t.Hybrid -or $Configs.Count -ne $all.Count) { Write-Output 'deck_estimate selftest FAIL'; exit 1 }
    Write-Output 'deck_estimate selftest PASS'; exit 0
}

function Get-Busy {
    $names = 'cargo', 'rustc', 'clippy-driver', 'rust-lld', 'link', 'grid-app', 'grid', 'grid-race-soak', 'grid-soak', 'gradle', 'java'
    $busy = @(Get-Process -Name $names -ErrorAction SilentlyContinue | Group-Object Name | ForEach-Object { "$($_.Name)($($_.Count))" })
    if ($busy.Count) { 'no:' + ($busy -join ',') } else { 'yes' }
}

# One race of configuration $Name; returns the exit state.
function Invoke-Race([string]$Name, $Topo) {
    $cpu, $gpu, $preset, $rows = $all[$Name]
    $mask = if ($cpu -eq 'E') { $Topo.E } else { $Topo.P }
    $dir = Join-Path $OutDir $Name
    $data = Join-Path $dir 'data'
    New-Item -ItemType Directory -Force $data | Out-Null
    $log = Join-Path $dir 'stdout.log'
    $settings = [IO.File]::ReadAllText((Join-Path $root 'android\app\src\main\assets\grid-import\settings.ini'))
    $video = "[video]`nresolution = 1280x800`nwindow = windowed`npresent_mode = auto_no_vsync`n"
    if ($rows) { $video += "render_height = $rows`nrender_height_version = 2`n" }
    $settings = ($settings -replace "`r`n", "`n").TrimEnd() + "`n[game]`nroot = $($GameRoot -replace '\\', '/')`n" +
        "[osd]`nfps = on`n[sound]`neffects = 0`nspeech = 0`nmusic = 0`n" + $video
    # tools/android_bench.ps1's bench.ini, plus the camera presses the bench sends by touch.
    $bench = "[drive]`nenabled = on`nlaps = 10`nopponents = 11`ngrid_position = 4`nai_characteristics = ai-driver-characteristics.tsv`n" +
        "[ai]`ndifficulty = 0.606`ncatchup = on`nchoreographer = on`nmistakes = on`n[input]`nforce_feedback = off`n" +
        "[diagnostics]`nautopilot = on`ncamera_presses = $Presses`n"
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $data 'settings.ini'), $settings, $utf8)
    [IO.File]::WriteAllText((Join-Path $data 'bench.ini'), $bench, $utf8)
    $env:WGPU_BACKEND = 'vulkan'
    $env:WGPU_ADAPTER_NAME = $gpu
    if ($preset -eq 'mobile') { $env:GRID_MOBILE_PRESET = '1' } else { Remove-Item Env:GRID_MOBILE_PRESET -ErrorAction SilentlyContinue }
    $idle = Get-Busy
    # A cmd redirect, not Start-Process -RedirectStandardOutput (F843: ~60 us per stdout write).
    $line = "start `"`" /b /wait /affinity $mask `"$Exe`" --data-dir `"$data`" --settings `"$data\bench.ini`" --drive-ticks $Ticks > `"$log`" 2> `"$log.err`""
    $p = Start-Process -FilePath cmd.exe -ArgumentList '/d', '/c', $line -WorkingDirectory $root -WindowStyle Hidden -PassThru
    $null = $p.Handle
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    $state = 'timeout'; $affinity = ''; $muted = $false
    while ((Get-Date) -lt $deadline) {
        if ($p.WaitForExit(1000)) { $state = "exit$($p.ExitCode)"; break }
        if (-not $affinity) {
            $g = Get-Process grid-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } | Select-Object -First 1
            if ($g) { $affinity = '{0:X}' -f [int64]$g.ProcessorAffinity }
        }
        if (-not $muted -and $affinity -and $mute -and (Test-Path $mute)) { & powershell -NoProfile -ExecutionPolicy Bypass -File $mute *> $null; $muted = $true }
        if ((Test-Path $log) -and (Select-String -LiteralPath $log -SimpleMatch 'drive_script_complete' -Quiet)) { $state = 'complete'; break }
    }
    $eap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    & taskkill /T /F /PID $p.Id 2>&1 | Out-Null
    $ErrorActionPreference = $eap
    $left = @(Get-Process grid-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe })
    $left | Stop-Process -Force -ErrorAction SilentlyContinue
    $left | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue
    foreach ($k in 'WGPU_BACKEND', 'WGPU_ADAPTER_NAME', 'GRID_MOBILE_PRESET') { Remove-Item "Env:$k" -ErrorAction SilentlyContinue }
    Set-Content -LiteralPath (Join-Path $dir 'run.txt') -Value (
        "config=$Name cpu=$cpu mask=$mask process_affinity=$affinity gpu=$gpu preset=$preset rows=$rows ticks=$Ticks presses=$Presses state=$state idle_start=$idle idle_end=$(Get-Busy)")
    return $state
}

if ($InLock) {
    $topo = Get-Topology
    $mute = Join-Path (Split-Path $common) 'local\mute_grid.ps1'
    Set-Content -LiteralPath (Join-Path $OutDir 'topology.txt') -Value "$($topo.Text) E_mask=$($topo.E) P_mask=$($topo.P)"
    if (-not $topo.Hybrid) { Write-Output 'deck_estimate FAIL: no hybrid CPU (one efficiency class)'; exit 2 }
    foreach ($c in $Configs) { $s = Invoke-Race $c $topo; Write-Output "  $c $s" }
    exit 0
}

if (-not $CheckOnly) {
    if ($ownerExe -and $Exe -eq $ownerExe) { Write-Output "deck_estimate FAIL: $Exe is the owner's prototype"; exit 2 }
    if (-not (Test-Path $Exe)) { Write-Output "deck_estimate FAIL: $Exe is missing (cargo build --release -p grid-app)"; exit 2 }
    New-Item -ItemType Directory -Force $OutDir | Out-Null
    $hash = (Get-FileHash -Algorithm SHA256 $Exe).Hash.ToLower()
    Set-Content -LiteralPath (Join-Path $OutDir 'exe.txt') -Value "exe=$Exe sha256=$hash rev=$(& git rev-parse --short=12 HEAD) dirty=$([bool](& git status --porcelain --untracked-files=no))"
    $minutes = [int][math]::Ceiling($Configs.Count * ($TimeoutSec + 20) / 60.0) + 5
    Push-Location $root
    try {
        & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'tools\build_lock.ps1') -HoldMinutes $minutes `
            powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'deck_estimate.ps1') -InLock -Exe $Exe `
            -Configs ($Configs -join ',') -Ticks $Ticks -TimeoutSec $TimeoutSec -Presses $Presses -GameRoot $GameRoot -OutDir $OutDir | Out-Host
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    if ($code -ne 0) { Write-Output "deck_estimate FAIL: run phase exit $code (logs under $OutDir)"; exit 1 }
}
& py -I $py report $OutDir
exit $LASTEXITCODE
