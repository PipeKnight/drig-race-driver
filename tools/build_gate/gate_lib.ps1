# Shared functions of the build gate (tools/build_gate.ps1, tools/build_gate_nightly.ps1;
# docs/TOOLCHAIN.md "Build gate"). Windows PowerShell 5.1. Independently authored
# diagnostics: it runs the port's grid-app and grid-soak, never the original grid.exe.
#
# A run is one grid-app process (or the event menu's supervisor and its children) in its
# own data dir under the run folder: muted, the ported AI driving the player's car
# (diagnostics autopilot with --drive-ticks), the FPS log on. The monitor samples memory,
# reads the logs as they grow and stops the run when its script completes.

$script:GateRepo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$script:GateTicksPerSecond = 62.5

# The private inputs (AI characteristics) live in local/ of the main checkout; a worktree's
# local/ usually lacks them.
function Get-GatePrivateDir {
    param([string]$PrivateDir)
    if ($PrivateDir) { return [IO.Path]::GetFullPath($PrivateDir) }
    $own = Join-Path $script:GateRepo 'local'
    if (Test-Path -LiteralPath (Join-Path $own 'ai-driver-characteristics.tsv')) { return $own }
    $common = (& git -C $script:GateRepo rev-parse --git-common-dir 2>$null)
    if ($LASTEXITCODE -eq 0 -and $common) {
        if (-not [IO.Path]::IsPathRooted($common)) { $common = Join-Path $script:GateRepo $common }
        $main = [IO.Path]::GetFullPath((Join-Path $common '..\local'))
        if (Test-Path -LiteralPath (Join-Path $main 'ai-driver-characteristics.tsv')) { return $main }
    }
    return $own
}

# ---- Muting ------------------------------------------------------------------
# The owner may be at the PC: every audio session of the gate's processes is muted in the
# Windows mixer (the data dir also sets [sound] effects = 0, which keeps every source
# decoding and mixing, only at zero trim).
if (-not ('GateMute' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
[Guid("BCDE0395-E52F-467C-8E3D-C4579291692E"), ComImport] class GateMMDeviceEnumerator {}
[Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IGateDeviceEnumerator { int EnumAudioEndpoints(int dataFlow, int mask, out IntPtr devices); int GetDefaultAudioEndpoint(int dataFlow, int role, out IGateDevice ep); }
[Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IGateDevice { int Activate(ref Guid iid, int ctx, IntPtr p, [MarshalAs(UnmanagedType.IUnknown)] out object o); }
[Guid("77AA99A0-1BD6-484F-8BC7-2C654C9A9B6F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IGateSessionManager2 { int f1(); int f2(); int GetSessionEnumerator(out IGateSessionEnumerator e); }
[Guid("E2F5BB11-0570-40CA-ACDD-3AA01277DEE8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IGateSessionEnumerator { int GetCount(out int n); int GetSession(int i, out IGateSessionControl2 s); }
[Guid("bfb7ff88-7239-4fc9-8fa2-07c950be9c6d"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IGateSessionControl2 { int a1(); int a2(); int a3(); int a4(); int a5(); int a6(); int a7(); int a8(); int a9(); int b1(); int b2(); int GetProcessId(out uint pid); }
[Guid("87CE5498-68D6-44E5-9215-6DA47EF883D8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IGateSimpleVolume { int SetMasterVolume(float v, ref Guid ctx); int GetMasterVolume(out float v); int SetMute(bool m, ref Guid ctx); int GetMute(out bool m); }
public static class GateMute {
  // Mutes the sessions of these processes; returns the ids that had a session.
  public static uint[] Mute(uint[] pids) {
    var hit = new List<uint>();
    try {
      var en = (IGateDeviceEnumerator)(new GateMMDeviceEnumerator()); IGateDevice dev;
      if (en.GetDefaultAudioEndpoint(0, 1, out dev) != 0 || dev == null) return hit.ToArray();
      Guid iid = typeof(IGateSessionManager2).GUID; object o; dev.Activate(ref iid, 23, IntPtr.Zero, out o);
      IGateSessionEnumerator se; ((IGateSessionManager2)o).GetSessionEnumerator(out se); int n; se.GetCount(out n);
      for (int i = 0; i < n; i++) { IGateSessionControl2 s; se.GetSession(i, out s); uint pid; s.GetProcessId(out pid);
        if (Array.IndexOf(pids, pid) >= 0) { Guid g = Guid.Empty; ((IGateSimpleVolume)s).SetMute(true, ref g); hit.Add(pid); } }
    } catch (Exception) { }
    return hit.ToArray();
  }
}
'@
}

# ---- Processes and memory ------------------------------------------------------

function Get-GateProcessTable {
    # One query for every grid-app process: id, parent, private bytes, peak private bytes.
    $table = @{}
    foreach ($p in @(Get-CimInstance Win32_Process -Filter "Name='grid-app.exe'" -ErrorAction SilentlyContinue)) {
        $table[[int]$p.ProcessId] = [pscustomobject]@{
            Parent = [int]$p.ParentProcessId
            PrivateMB = [double]$p.PrivatePageCount / 1MB
            PeakMB = [double]$p.PeakPageFileUsage / 1KB
        }
    }
    return $table
}

function Get-GateTree {
    param([int]$RootId, $Table)
    $ids = New-Object System.Collections.Generic.List[int]
    if ($Table.ContainsKey($RootId)) { $ids.Add($RootId) }
    $grew = $true
    while ($grew) {
        $grew = $false
        foreach ($id in @($Table.Keys)) {
            if (-not $ids.Contains($id) -and $ids.Contains($Table[$id].Parent)) { $ids.Add($id); $grew = $true }
        }
    }
    return , $ids
}

function Stop-GateTree {
    param([int]$RootId)
    & "$env:SystemRoot\System32\taskkill.exe" /T /F /PID $RootId 2>&1 | Out-Null
}

function Get-GateFreeMemoryGB {
    $os = Get-CimInstance Win32_OperatingSystem
    return [pscustomobject]@{
        PhysicalGB = [double]$os.FreePhysicalMemory / 1MB
        CommitGB = [double]$os.FreeVirtualMemory / 1MB
    }
}

# ---- Data dir and start ----------------------------------------------------------

function New-GateDataDir {
    param([string]$Dir, [string]$PrivateDir, [string]$Extra = '')
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    $tsv = Join-Path $PrivateDir 'ai-driver-characteristics.tsv'
    if (Test-Path -LiteralPath $tsv) { Copy-Item -LiteralPath $tsv -Destination $Dir }
    $text = "# Build gate run (tools/build_gate/gate_lib.ps1): muted, autopilot, FPS log.`r`n" +
        "[osd]`r`nfps = on`r`n[sound]`r`neffects = 0`r`n[front_end]`r`nshow = off`r`n" +
        "[diagnostics]`r`nautopilot = on`r`nresults_continue = 3`r`n" + $Extra
    [IO.File]::WriteAllText((Join-Path $Dir 'settings.ini'), $text, (New-Object Text.UTF8Encoding($false)))
}

function Format-GateArgument {
    param([string]$Value)
    if ($Value -match '[\s"]') { return '"' + ($Value -replace '"', '\"') + '"' }
    return $Value
}

# Starts one run. $Spec: Name, Kind, Args (string[] after --data-dir), Env (hashtable),
# Lead (arguments before --data-dir), TimeoutSec, Extra (settings.ini text), StopOnPause,
# WarmupSec, AllowExit (a full event that ends with its results may exit 0).
function Start-GateRun {
    param($Spec, [string]$Exe, [string]$OutDir, [string]$PrivateDir)
    $dir = Join-Path $OutDir $Spec.Name
    New-GateDataDir -Dir $dir -PrivateDir $PrivateDir -Extra $Spec.Extra
    # Files: private copies the run needs in its data dir (a repro's AI characteristics).
    foreach ($f in @($Spec.Files)) { if ($f) { Copy-Item -LiteralPath $f -Destination $dir -Force } }
    # Lead: arguments that must come first (--menu).
    $argv = @(@($Spec.Lead) + @('--data-dir', $dir) + @($Spec.Args) | Where-Object { $null -ne $_ })
    $line = ($argv | ForEach-Object { Format-GateArgument $_ }) -join ' '
    $saved = @{}
    foreach ($k in $Spec.Env.Keys) {
        $saved[$k] = [Environment]::GetEnvironmentVariable($k, 'Process')
        [Environment]::SetEnvironmentVariable($k, [string]$Spec.Env[$k], 'Process')
    }
    try {
        $process = Start-Process -FilePath $Exe -ArgumentList $line -WorkingDirectory $script:GateRepo `
            -RedirectStandardOutput (Join-Path $dir 'stdout.log') -RedirectStandardError (Join-Path $dir 'stderr.log') `
            -WindowStyle Minimized -PassThru
    } finally {
        foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k], 'Process') }
    }
    $null = $process.Handle  # keeps the exit code readable after the process ends
    Set-Content -LiteralPath (Join-Path $dir 'command.txt') -Value ("$Exe $line`r`nenv: " + (($Spec.Env.Keys | ForEach-Object { "$_=$($Spec.Env[$_])" }) -join ' '))
    return [pscustomobject]@{
        Spec = $Spec; Dir = $dir; Process = $process; Id = $process.Id
        Started = Get-Date; DriveStarted = $null; Ended = $null
        Offsets = @{ 'stdout.log' = 0L; 'stderr.log' = 0L }; Partial = @{ 'stdout.log' = ''; 'stderr.log' = '' }
        Samples = New-Object System.Collections.Generic.List[object]
        PeakMB = 0.0; Muted = New-Object System.Collections.Generic.HashSet[int]
        Errors = New-Object System.Collections.Generic.List[string]
        Paused = $null; Panic = $null
        Audio = New-Object System.Collections.Generic.List[string]
        Fps = New-Object System.Collections.Generic.List[double]
        Frames = 0.0; FrameSeconds = 0.0; SimSum = 0.0; SimCount = 0; PropBodies = 0
        Complete = $false; RaceResult = $false; LastTick = 0
        Stop = $null; ExitCode = $null; Finished = $false; CompleteAt = $null
    }
}

# ---- Log reading -------------------------------------------------------------------

$script:GateAnsi = [regex]'\x1b\[[0-9;]*m'

function Read-GateNewLines {
    param($Run, [string]$File)
    $path = Join-Path $Run.Dir $File
    if (-not (Test-Path -LiteralPath $path)) { return @() }
    $fs = [IO.File]::Open($path, 'Open', 'Read', 'ReadWrite')
    try {
        if ($fs.Length -le $Run.Offsets[$File]) { return @() }
        $null = $fs.Seek($Run.Offsets[$File], 'Begin')
        $count = [int]($fs.Length - $Run.Offsets[$File])
        $buffer = New-Object byte[] $count
        $read = $fs.Read($buffer, 0, $count)
        $Run.Offsets[$File] += $read
    } finally { $fs.Close() }
    $text = $Run.Partial[$File] + [Text.Encoding]::UTF8.GetString($buffer, 0, $read)
    $parts = $text -split "`n"
    $Run.Partial[$File] = $parts[-1]
    if ($parts.Count -le 1) { return @() }
    return $parts[0..($parts.Count - 2)]
}

function Update-GateLog {
    param($Run)
    foreach ($file in @('stdout.log', 'stderr.log')) {
        foreach ($raw in (Read-GateNewLines -Run $Run -File $file)) {
            # Hot path: the per-tick samples (most of stdout).
            if ($raw.StartsWith('drive_sample ')) {
                $i = $raw.IndexOf(' tick=')
                if ($i -gt 0) {
                    $j = $raw.IndexOf(' ', $i + 6)
                    $t = 0L
                    if ($j -gt $i -and [int64]::TryParse($raw.Substring($i + 6, $j - $i - 6), [ref]$t) -and $t -gt $Run.LastTick) { $Run.LastTick = $t }
                }
                continue
            }
            if ($raw.StartsWith('drive_lap_sample ') -or $raw.StartsWith('drive_handling_sample ') -or
                $raw.StartsWith('drive_deformation_sample ') -or $raw.StartsWith('drive_input ')) { continue }
            $line = $script:GateAnsi.Replace($raw, '').TrimEnd("`r")
            if ($line.StartsWith('frame_stats ')) {
                if ($line -match ' fps=([0-9.]+) .* frames=([0-9]+) sim_ticks_per_s=([0-9.]+)') {
                    $fps = [double]$Matches[1]; $frames = [double]$Matches[2]
                    if ([double]$Matches[3] -gt 0) {
                        if (-not $Run.DriveStarted) { $Run.DriveStarted = Get-Date }
                        $Run.Fps.Add($fps)
                        $Run.SimSum += [double]$Matches[3]; $Run.SimCount++
                        if ($fps -gt 0) { $Run.Frames += $frames; $Run.FrameSeconds += $frames / $fps }
                    }
                }
                continue
            }
            if ($line.Contains('drive_script_complete')) { $Run.Complete = $true }
            if ($line.StartsWith('drive_race_result')) { $Run.RaceResult = $true }
            if ($line.StartsWith('dynamic_props event=break') -and $line -match ' bodies=([0-9]+)' -and [int]$Matches[1] -gt $Run.PropBodies) { $Run.PropBodies = [int]$Matches[1] }
            if ($line.Contains('driving paused')) {
                if (-not $Run.Paused) { $Run.Paused = ($line -replace '^.*driving paused: ', '') }
                continue
            }
            if ($line -match 'panicked at|grid-app: .*panic') {
                if (-not $Run.Panic) { $Run.Panic = $line.Trim() }
                continue
            }
            $audio = $line.Contains('decoder_failed') -or $line.Contains('bank_failed') -or
                ($line -match '\b(WARN|ERROR)\b' -and $line -match '(bevy_audio|rodio|cpal)|(?i)audio.*(fail|error|unavailable)')
            if ($audio) {
                if ($Run.Audio.Count -lt 5) { $Run.Audio.Add($line.Trim()) }
                continue
            }
            if ($line -match '^\S+\s+ERROR\s' -or $line -match '^\s*ERROR\s') {
                if ($Run.Errors.Count -lt 5) { $Run.Errors.Add($line.Trim()) }
            }
        }
    }
}

# ---- Monitor -------------------------------------------------------------------------

# Advances every active run once: memory sample, mute, logs, completion. Returns nothing;
# a run whose process tree ended or was stopped gets Finished = $true.
function Update-GateRuns {
    param($Runs, [int]$AfterCompleteSec = 2)
    $table = Get-GateProcessTable
    $pids = New-Object System.Collections.Generic.List[uint32]
    foreach ($run in $Runs) {
        if ($run.Finished) { continue }
        $tree = Get-GateTree -RootId $run.Id -Table $table
        $private = 0.0
        foreach ($id in $tree) {
            $private += $table[$id].PrivateMB
            if ($table[$id].PeakMB -gt $run.PeakMB) { $run.PeakMB = $table[$id].PeakMB }
            if (-not $run.Muted.Contains($id)) { $pids.Add([uint32]$id) }
        }
        if ($private -gt $run.PeakMB) { $run.PeakMB = $private }
        if ($tree.Count -gt 0) {
            $run.Samples.Add([pscustomobject]@{ T = ((Get-Date) - $run.Started).TotalSeconds; MB = $private; Drive = [bool]$run.DriveStarted })
        }
        Update-GateLog -Run $run
        $now = Get-Date
        $elapsed = ($now - $run.Started).TotalSeconds
        if ($run.Process.HasExited -and $tree.Count -eq 0) {
            $run.ExitCode = $run.Process.ExitCode
            $run.Ended = $now; $run.Finished = $true
            Update-GateLog -Run $run
            continue
        }
        $stop = $null
        if ($run.Paused -and $run.Spec.StopOnPause) { $stop = 'paused' }
        elseif ($run.Panic) { $stop = 'panic' }
        elseif ($run.Spec.StopAfterDriveSec -and $run.DriveStarted -and ($now - $run.DriveStarted).TotalSeconds -ge $run.Spec.StopAfterDriveSec) { $stop = 'complete' }
        elseif ($run.Complete) {
            if (-not $run.CompleteAt) { $run.CompleteAt = $now }
            if (($now - $run.CompleteAt).TotalSeconds -ge $AfterCompleteSec) { $stop = 'complete' }
        }
        elseif ($elapsed -gt $run.Spec.TimeoutSec) { $stop = 'timeout' }
        if ($stop) {
            Stop-GateTree -RootId $run.Id
            Start-Sleep -Milliseconds 300
            Update-GateLog -Run $run
            $run.Stop = $stop; $run.Ended = Get-Date; $run.Finished = $true
        }
    }
    if ($pids.Count -gt 0) {
        foreach ($id in [GateMute]::Mute($pids.ToArray())) {
            foreach ($run in $Runs) { $null = $run.Muted.Add([int]$id) }
        }
    }
}

# Disk: the replay spool of a run (replay-cache, 70-190 MB for a long race) is kept only when
# the run failed.
function Clear-GateReplayCache([object]$Run, [string]$Status) {
    if ($Status -eq 'fail') { return }
    $cache = Join-Path $Run.Dir 'replay-cache'
    if (Test-Path -LiteralPath $cache) { Remove-Item -LiteralPath $cache -Recurse -Force -ErrorAction SilentlyContinue }
}

# Runs the specs, at most $Jobs at a time, starting a run only while at least $MinFreeGB of
# physical memory and twice that of commit are free. $OnFinish gets each finished run.
function Invoke-GateQueue {
    param($Specs, [string]$Exe, [string]$OutDir, [string]$PrivateDir, [int]$Jobs = 3,
        [double]$MinFreeGB = 4, [scriptblock]$OnFinish, [datetime]$Deadline = [datetime]::MaxValue)
    $queue = New-Object System.Collections.Generic.Queue[object]
    foreach ($s in $Specs) { $queue.Enqueue($s) }
    $active = New-Object System.Collections.Generic.List[object]
    $done = New-Object System.Collections.Generic.List[object]
    $lastStart = [datetime]::MinValue
    while ($queue.Count -gt 0 -or $active.Count -gt 0) {
        # Starts are spaced 5 s apart: loading is the memory peak.
        if ($queue.Count -gt 0 -and $active.Count -lt $Jobs -and ((Get-Date) - $lastStart).TotalSeconds -ge 5 -and (Get-Date) -lt $Deadline) {
            $free = Get-GateFreeMemoryGB
            if ($active.Count -eq 0 -or ($free.PhysicalGB -ge $MinFreeGB -and $free.CommitGB -ge 2 * $MinFreeGB)) {
                $spec = $queue.Dequeue()
                $run = Start-GateRun -Spec $spec -Exe $Exe -OutDir $OutDir -PrivateDir $PrivateDir
                $active.Add($run); $lastStart = Get-Date
                Write-Host ("gate_start {0} kind={1} pid={2} free_physical_gb={3:N1}" -f $spec.Name, $spec.Kind, $run.Id, $free.PhysicalGB)
            }
        } elseif ($queue.Count -gt 0 -and (Get-Date) -ge $Deadline -and $active.Count -eq 0) {
            break
        }
        Start-Sleep -Milliseconds 1000
        Update-GateRuns -Runs $active
        foreach ($run in @($active | Where-Object { $_.Finished })) {
            $null = $active.Remove($run)
            $done.Add($run)
            if ($OnFinish) { & $OnFinish $run }
        }
    }
    return , $done
}

# ---- Verdicts ---------------------------------------------------------------------------

# Least-squares slope (MB/s) of the private-bytes samples taken after $WarmupSec of driving.
function Get-GateSlope {
    param($Run, [double]$WarmupSec = 120)
    if (-not $Run.DriveStarted) { return $null }
    $drive0 = ($Run.DriveStarted - $Run.Started).TotalSeconds
    $pts = @($Run.Samples | Where-Object { $_.T -ge $drive0 + $WarmupSec })
    if ($pts.Count -lt 10) { return $null }
    $n = $pts.Count; $mt = ($pts | Measure-Object T -Average).Average; $mm = ($pts | Measure-Object MB -Average).Average
    $num = 0.0; $den = 0.0
    foreach ($p in $pts) { $num += ($p.T - $mt) * ($p.MB - $mm); $den += ($p.T - $mt) * ($p.T - $mt) }
    if ($den -le 0) { return $null }
    $first = @($pts | Select-Object -First ([math]::Max(1, [int]($n / 10))) | Measure-Object MB -Average).Average
    $last = @($pts | Select-Object -Last ([math]::Max(1, [int]($n / 10))) | Measure-Object MB -Average).Average
    return [pscustomobject]@{ Slope = $num / $den; Seconds = $pts[-1].T - $pts[0].T; Samples = $n; FirstMB = $first; LastMB = $last }
}

# Expected failures: tab-separated `run check pattern reference`, `#` comments; run is a
# regex over run names, pattern a regex over the reason text.
function Read-GateExpected {
    param([string]$Path)
    $list = New-Object System.Collections.Generic.List[object]
    if (-not (Test-Path -LiteralPath $Path)) { return , $list }
    foreach ($line in Get-Content -LiteralPath $Path -Encoding UTF8) {
        if ($line -match '^\s*(#|$)' -or $line.StartsWith("run`t")) { continue }
        $c = $line -split "`t"
        if ($c.Count -lt 4) { throw "expected-failure line needs 4 tab-separated columns: $line" }
        $list.Add([pscustomobject]@{ Run = $c[0]; Check = $c[1]; Pattern = $c[2]; Reference = $c[3]; Seen = $false })
    }
    return , $list
}

# Reasons of a run as (check, text) pairs.
function Get-GateReasons {
    param($Run, [double]$MemoryCapMB, [double]$FpsFloor, [double]$SlopeLimit = 0, [double]$RealtimeFloor = 0)
    $r = New-Object System.Collections.Generic.List[object]
    $add = { param($c, $t) $r.Add([pscustomobject]@{ Check = $c; Text = $t }) }
    # The peak prop body count goes into the slowness reasons: broken props that never sleep
    # are the known cause of slow races (docs/TOOLCHAIN.md "Build gate"), matched by expected.tsv.
    $props = "prop bodies up to $($Run.PropBodies)"
    if ($Run.Paused) { & $add 'paused' "driving paused: $($Run.Paused)" }
    if ($Run.Panic) { & $add 'panic' $Run.Panic }
    foreach ($a in $Run.Audio) { & $add 'audio' $a }
    foreach ($e in $Run.Errors) { & $add 'error' $e }
    if ($Run.Stop -eq 'timeout') { & $add 'timeout' "no end after $($Run.Spec.TimeoutSec) s (last tick $($Run.LastTick)); $props" }
    if ($null -ne $Run.ExitCode -and -not ($Run.Spec.AllowExit -and $Run.ExitCode -eq 0 -and $Run.RaceResult)) {
        & $add 'exit' "process ended with exit code $($Run.ExitCode) before the script completed (last tick $($Run.LastTick))"
    }
    if (-not $Run.DriveStarted) { & $add 'exit' 'the race never started driving (no frame_stats with sim ticks)' }
    if ($MemoryCapMB -gt 0 -and $Run.PeakMB -gt $MemoryCapMB) { & $add 'memory' ("peak private {0:N0} MB > cap {1:N0} MB" -f $Run.PeakMB, $MemoryCapMB) }
    $avg = Get-GateAverageFps $Run
    if ($FpsFloor -gt 0 -and $null -ne $avg -and $avg -lt $FpsFloor) { & $add 'fps' ("average {0:N1} FPS < floor {1}; {2}" -f $avg, $FpsFloor, $props) }
    $sim = Get-GateSimRate $Run
    if ($RealtimeFloor -gt 0 -and $null -ne $sim -and $sim -lt $RealtimeFloor) { & $add 'realtime' ("sim {0:N1} ticks/s < {1} (real time 62.5); {2}" -f $sim, $RealtimeFloor, $props) }
    if ($SlopeLimit -gt 0) {
        $s = Get-GateSlope -Run $Run -WarmupSec $Run.Spec.WarmupSec
        if ($null -eq $s) { & $add 'slope' 'too few samples after the warm-up for a memory slope' }
        elseif ($s.Slope -gt $SlopeLimit) { & $add 'slope' ("private bytes grow {0:N2} MB/s > limit {1} MB/s over {2:N0} s ({3:N0} -> {4:N0} MB)" -f $s.Slope, $SlopeLimit, $s.Seconds, $s.FirstMB, $s.LastMB) }
    }
    return , $r
}

function Get-GateSimRate {
    param($Run)
    if ($Run.SimCount -le 0) { return $null }
    return $Run.SimSum / $Run.SimCount
}

function Get-GateAverageFps {
    param($Run)
    if ($Run.FrameSeconds -le 0) { return $null }
    return $Run.Frames / $Run.FrameSeconds
}

# pass, xfail (every reason expected) or fail; marks the expected entries seen.
function Resolve-GateStatus {
    param([string]$Name, $Reasons, $Expected)
    $unexpected = New-Object System.Collections.Generic.List[object]
    $refs = New-Object System.Collections.Generic.List[string]
    foreach ($reason in $Reasons) {
        $hit = $null
        foreach ($e in $Expected) {
            if ($Name -match "^(?:$($e.Run))$" -and $e.Check -eq $reason.Check -and $reason.Text -match $e.Pattern) { $hit = $e; break }
        }
        if ($hit) { $hit.Seen = $true; if (-not $refs.Contains($hit.Reference)) { $refs.Add($hit.Reference) } }
        else { $unexpected.Add($reason) }
    }
    $status = 'pass'
    if ($unexpected.Count -gt 0) { $status = 'fail' } elseif ($Reasons.Count -gt 0) { $status = 'xfail' }
    return [pscustomobject]@{ Status = $status; Unexpected = $unexpected; References = ($refs -join '; ') }
}

function Get-GateRevision {
    $rev = (& git -C $script:GateRepo rev-parse --short=12 HEAD 2>$null)
    $dirty = @(& git -C $script:GateRepo status --porcelain --untracked-files=no 2>$null).Count -gt 0
    if ($dirty) { return "$rev-dirty" }
    return $rev
}

# Other sessions' compilers running now (perf waits for them), e.g. "cargo=2,rustc=1"; empty if none.
function Get-GateCompilers {
    $procs = @(Get-Process -Name 'rustc', 'cargo', 'link', 'rust-lld', 'clippy-driver' -ErrorAction SilentlyContinue)
    return (($procs | Group-Object ProcessName | ForEach-Object { "$($_.Name)=$($_.Count)" }) -join ',')
}

function Get-GateLoad {
    # Other sessions' compilers and grid processes running when the gate starts.
    $names = 'rustc', 'cargo', 'link', 'rust-lld', 'grid-app', 'grid-soak'
    $procs = @(Get-Process -Name $names -ErrorAction SilentlyContinue)
    return (($procs | Group-Object ProcessName | ForEach-Object { "$($_.Name)=$($_.Count)" }) -join ',')
}
