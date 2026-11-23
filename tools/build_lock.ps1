<#
.SYNOPSIS
One cargo build at a time across every checkout on this PC (docs/TOOLCHAIN.md, "Build lock").

.DESCRIPTION
The PC is memory-bound (64 GiB, commit ~62 GiB under load): two release builds at once push
it into paging. Every script that builds takes this lock first, and so should ad-hoc cargo
commands of the sessions:

    powershell -NoProfile -File tools\build_lock.ps1 cargo build --release -p grid-app
    powershell -NoProfile -File tools\build_lock.ps1 -HoldMinutes 240 cargo run --release -p grid-tools --bin grid-soak -- long
    (Bash:  powershell.exe -NoProfile -File tools/build_lock.ps1 cargo clippy --release -p grid-app -- -D warnings)

The wrapper waits for its turn, runs the command, releases the lock and exits with the
command's exit code; when the command outlives the hold limit (-HoldMinutes, default
$BuildLockDefaultHoldMinutes min) it kills the command's whole process tree, appends a line to
timeouts.log and exits with 124. `cargo t` and `cargo td` work through it in worktrees too: it
expands the repository alias itself (Expand-CargoAlias, docs/TOOLCHAIN.md P9). Scripts
dot-source it instead:

    . (Join-Path $PSScriptRoot 'build_lock.ps1')
    $lock = Enter-BuildLock -Purpose 'update_prototype' -HoldMinutes 60
    try { ... cargo ... } finally { Exit-BuildLock $lock }

How it works (state lives in C:\cargo-build\build-lock, beside the build dirs; the environment
variable GRID_BUILD_LOCK_ROOT moves it, for tests such as tools\build_lock_test.ps1):
- `lock`: the holder keeps this file open with no write sharing. Windows closes the handle when
  the holder dies, so a crashed holder never leaves a stuck lock. Its content (JSON: pid,
  process start time, worktree, purpose, start time, hold limit, deadline, watchdog pid) is for
  the waiters and -Status to print; a leftover file from a dead pid is reported as stale and
  taken over.
- `queue\<ticks>-<pid>.json`: one entry per waiter, first come first served. Entries of dead
  pids (or of a pid reused by another process: start time differs) are deleted by whoever
  sees them. Waiters print their queue position and the holder when either changes, and
  every -ReportSeconds.
- Hold limit: every hold has one (-HoldMinutes, never unlimited). At acquisition the holder
  puts itself into a named Windows job object, so every process it starts, and their children
  even after their parent exits (cargo's test binaries), are members; and it starts a detached
  watchdog (this script with -Watchdog). The watchdog waits until the deadline plus a grace
  period (-GraceSeconds); if the same holder (pid, pid start, acquisition time) still holds the
  lock then, it kills the job's members, the holder's descendants and every process started
  after the acquisition from the checkout's target or build dir, appends a line to
  `timeouts.log` and so leaves the lock free. Cooperative holders (this wrapper, prepush.ps1
  through Invoke-BuildLockProcess) stop their own commands at the deadline and fail with a
  verdict before the watchdog acts. Exit-BuildLock stops the watchdog, kills leftover cargo test
  binaries of the job and appends the hold to `holds.log` (data for the limits).
- Re-entrant: the holder sets GRID_BUILD_LOCK=<pid>; a child script that sees its ancestor
  still holding the lock (tools\release_playable.ps1 -> update_prototype.ps1) runs without
  queueing behind itself and under the ancestor's limit.
- Processes outside the lock are not stopped; the holder prints the cargo/rustc processes and
  the grid-app.exe, grid.exe, cargo test binaries and apitrace processes that run outside it
  (Get-BuildLockOutsideProcesses), so timing scripts can record a noisy measurement.

Inspect the lock and the queue:  powershell -NoProfile -File tools\build_lock.ps1 -Status
#>

$script:BuildLockRoot = if ($env:GRID_BUILD_LOCK_ROOT) { $env:GRID_BUILD_LOCK_ROOT } else { 'C:\cargo-build\build-lock' }
$script:BuildLockScript = Join-Path $PSScriptRoot 'build_lock.ps1'
# Default hold limits in minutes, about twice the longest successful run of each kind
# (measured 2026-11-23, docs/TOOLCHAIN.md "Build lock"): an ordinary cargo command 55 (an
# unseeded release build of grid-app 1607 s; the longest single cargo step of a passing gate
# 1258 s), Enter-BuildLock without -HoldMinutes the same.
$script:BuildLockDefaultHoldMinutes = 55
$script:BuildLockDefaultGraceSeconds = 120
$script:BuildLockJob = [IntPtr]::Zero
$script:BuildLockWatchdogPid = 0
$script:BuildLockDirs = $null

function Get-BuildLockProcessStart([int]$ProcessId) {
    try {
        $p = Get-Process -Id $ProcessId -ErrorAction Stop
        return $p.StartTime.ToUniversalTime().ToString('o')
    } catch { return $null }
}

# A record (lock content or queue entry) whose process is gone or was replaced.
function Test-BuildLockRecordDead($Record) {
    if (-not $Record -or -not $Record.pid) { return $true }
    $start = Get-BuildLockProcessStart ([int]$Record.pid)
    if (-not $start) { return $true }
    return ($Record.pid_start -and $start -ne $Record.pid_start)
}

function Read-BuildLockFile([string]$Path) {
    try {
        $fs = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite,Delete')
        try {
            $reader = New-Object IO.StreamReader($fs)
            $text = $reader.ReadToEnd()
        } finally { $fs.Close() }
        if (-not $text.Trim()) { return $null }
        return ($text | ConvertFrom-Json)
    } catch { return $null }
}

function Format-BuildLockHolder($Holder) {
    if (-not $Holder) { return 'unknown holder (lock just taken)' }
    $since = ''
    try { $since = [int]((Get-Date).ToUniversalTime() - [DateTime]::Parse($Holder.started_utc).ToUniversalTime()).TotalMinutes } catch {}
    $limit = if ($Holder.hold_minutes) { " of $($Holder.hold_minutes) min limit" } else { ' (no limit recorded)' }
    return "pid $($Holder.pid) [$($Holder.purpose)] in $($Holder.worktree), $since min$limit"
}

function Get-BuildLockQueue {
    $dir = Join-Path $script:BuildLockRoot 'queue'
    if (-not (Test-Path -LiteralPath $dir)) { return @() }
    $live = @()
    foreach ($f in @(Get-ChildItem -LiteralPath $dir -Filter '*.json' -File | Sort-Object Name)) {
        $r = Read-BuildLockFile $f.FullName
        if (Test-BuildLockRecordDead $r) {
            # Unreadable entries younger than 10 s may still be being written.
            if ($r -or $f.LastWriteTime -lt (Get-Date).AddSeconds(-10)) {
                Remove-Item -LiteralPath $f.FullName -Force -ErrorAction SilentlyContinue
            }
            continue
        }
        $live += [pscustomobject]@{ File = $f.FullName; Record = $r }
    }
    return $live
}

# ---- processes ---------------------------------------------------------------------------------
function Get-BuildLockProcessTable {
    return @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, ExecutablePath, CreationDate -ErrorAction SilentlyContinue)
}

# Pids of the live descendants of $RootPids (a child must not be older than its parent: pids are
# reused).
function Get-BuildLockDescendants([int[]]$RootPids, $Table) {
    $children = @{}; $created = @{}
    foreach ($p in $Table) {
        $id = [int]$p.ProcessId; $created[$id] = $p.CreationDate
        $pp = [int]$p.ParentProcessId
        if ($pp -eq $id) { continue }
        if (-not $children.ContainsKey($pp)) { $children[$pp] = New-Object System.Collections.Generic.List[int] }
        $children[$pp].Add($id)
    }
    $seen = @{}; $todo = New-Object System.Collections.Generic.Queue[int]
    foreach ($r in $RootPids) { if ($r) { $todo.Enqueue($r) } }
    while ($todo.Count) {
        $id = $todo.Dequeue()
        if (-not $children.ContainsKey($id)) { continue }
        foreach ($c in $children[$id]) {
            if ($seen.ContainsKey($c)) { continue }
            if ($created.ContainsKey($id) -and $created[$c] -and $created[$id] -and $created[$c] -lt $created[$id]) { continue }
            $seen[$c] = $true; $todo.Enqueue($c)
        }
    }
    return @($seen.Keys)
}

# Records the live descendants of $RootPids, and of the processes recorded before, as
# pid -> creation time: a grandchild whose parent exits stays known (the fallback when no job
# object is available).
function Update-BuildLockTracked([int[]]$RootPids, [hashtable]$Tracked) {
    $table = Get-BuildLockProcessTable
    $live = @{}; foreach ($p in $table) { $live[[int]$p.ProcessId] = $p }
    $roots = @($RootPids) + @($Tracked.Keys | Where-Object { $live.ContainsKey($_) -and $live[$_].CreationDate -eq $Tracked[$_] })
    foreach ($id in @(Get-BuildLockDescendants $roots $table)) {
        if (-not $Tracked.ContainsKey($id)) { $Tracked[$id] = $live[$id].CreationDate }
    }
}

function Initialize-BuildLockJobType {
    if ('GridBuildLockJob' -as [type]) { return $true }
    try {
        Add-Type -ErrorAction Stop -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GridBuildLockJob {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern IntPtr CreateJobObjectW(IntPtr attributes, string name);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern IntPtr OpenJobObjectW(uint access, bool inherit, string name);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool TerminateJobObject(IntPtr job, uint code);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool QueryInformationJobObject(IntPtr job, int infoClass, IntPtr info, int length, IntPtr returned);
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr handle);
    public static IntPtr Create(string name) { return CreateJobObjectW(IntPtr.Zero, name); }
    public static IntPtr Open(string name) { return OpenJobObjectW(0x1F001F, false, name); }
    public static bool Assign(IntPtr job, int pid) {
        IntPtr p = OpenProcess(0x0101, false, pid);
        if (p == IntPtr.Zero) return false;
        try { return AssignProcessToJobObject(job, p); } finally { CloseHandle(p); }
    }
    public static int[] Pids(IntPtr job) {
        int n = 16384, size = 8 + n * IntPtr.Size;
        IntPtr buf = Marshal.AllocHGlobal(size);
        try {
            if (!QueryInformationJobObject(job, 3, buf, size, IntPtr.Zero)) return new int[0];
            int count = Marshal.ReadInt32(buf, 4);
            int[] r = new int[count];
            for (int i = 0; i < count; i++) r[i] = (int)Marshal.ReadIntPtr(buf, 8 + i * IntPtr.Size).ToInt64();
            return r;
        } finally { Marshal.FreeHGlobal(buf); }
    }
    public static bool Terminate(IntPtr job, uint code) { return TerminateJobObject(job, code); }
    public static void Close(IntPtr handle) { CloseHandle(handle); }
}
'@
        return $true
    } catch { return $false }
}

function Get-BuildLockJobPids([IntPtr]$Job) {
    if ($Job -eq [IntPtr]::Zero) { return @() }
    return @([GridBuildLockJob]::Pids($Job))
}

# The checkout's target and build dirs (cargo metadata), for the "started from this checkout"
# rule; empty outside a cargo workspace.
function Get-BuildLockDirs([string]$Worktree = (Get-Location).Path) {
    if ($null -ne $script:BuildLockDirs) { return $script:BuildLockDirs }
    $dirs = @()
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo -and (Test-Path "$env:USERPROFILE\.cargo\bin\cargo.exe")) { $cargo = Get-Command "$env:USERPROFILE\.cargo\bin\cargo.exe" }
    if ($cargo -and (Test-Path -LiteralPath (Join-Path $Worktree 'Cargo.toml'))) {
        $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        try {
            Push-Location -LiteralPath $Worktree
            $json = & $cargo.Source metadata --format-version 1 --no-deps --offline 2>$null
            $m = ($json | Out-String | ConvertFrom-Json)
            foreach ($d in @($m.target_directory, $m.build_directory)) { if ($d) { $dirs += ([IO.Path]::GetFullPath($d).TrimEnd('\') + '\') } }
        } catch {} finally { Pop-Location; $ErrorActionPreference = $saved }
    }
    $script:BuildLockDirs = @($dirs | Sort-Object -Unique)
    return $script:BuildLockDirs
}

<#
Kills, twice over (a process may start another while the first pass runs): the members of $Job,
the live descendants of $RootPids, the live processes in $Tracked (same creation time) and the
processes started after $Since whose executable lies under one of $Dirs. Never $PID or $Except.
Returns the killed processes as "name(pid)" strings.
#>
function Stop-BuildLockProcesses {
    param([int[]]$RootPids = @(), [hashtable]$Tracked = @{}, [IntPtr]$Job = [IntPtr]::Zero, [string[]]$Dirs = @(), [datetime]$Since = [datetime]::MaxValue, [int[]]$Except = @())
    $killed = New-Object System.Collections.Generic.List[string]
    $skip = @{ $PID = $true }; foreach ($e in $Except) { $skip[$e] = $true }
    for ($pass = 0; $pass -lt 2; $pass++) {
        $table = Get-BuildLockProcessTable
        $byId = @{}; foreach ($p in $table) { $byId[[int]$p.ProcessId] = $p }
        $victims = @{}  # not [ordered]: an int key would index it by position
        foreach ($id in $RootPids) { if ($byId.ContainsKey($id)) { $victims[$id] = $true } }
        foreach ($id in @(Get-BuildLockJobPids $Job)) { $victims[$id] = $true }
        foreach ($id in @(Get-BuildLockDescendants $RootPids $table)) { $victims[$id] = $true }
        foreach ($id in @($Tracked.Keys)) { if ($byId.ContainsKey($id) -and $byId[$id].CreationDate -eq $Tracked[$id]) { $victims[$id] = $true } }
        if ($Dirs.Count) {
            foreach ($p in $table) {
                if (-not $p.ExecutablePath -or -not $p.CreationDate -or $p.CreationDate -lt $Since) { continue }
                foreach ($d in $Dirs) { if ($p.ExecutablePath.StartsWith($d, [StringComparison]::OrdinalIgnoreCase)) { $victims[[int]$p.ProcessId] = $true; break } }
            }
        }
        foreach ($id in @($victims.Keys)) {
            if ($skip.ContainsKey($id) -or -not $byId.ContainsKey($id)) { continue }
            try { Stop-Process -Id $id -Force -ErrorAction Stop; $killed.Add("$($byId[$id].Name)($id)") } catch {}
        }
        if ($pass -eq 0) { Start-Sleep -Milliseconds 500 }
    }
    return @($killed | Sort-Object -Unique)
}

function Write-BuildLockTimeoutLine([string]$Line) {
    try {
        New-Item -ItemType Directory -Force -Path $script:BuildLockRoot | Out-Null
        [IO.File]::AppendAllText((Join-Path $script:BuildLockRoot 'timeouts.log'), "$([DateTime]::UtcNow.ToString('o')) $Line`r`n")
    } catch {}
}

<#
Runs one command line (`$FilePath $Arguments`) as a child process with the console of this one
and waits for it, until $Deadline (local time; $null waits without one). On expiry it kills the
child's whole tree, including processes whose parent has already exited (the job object of this
process's hold, the descendants recorded while waiting, and processes started since from the
checkout's target/build dir) and returns exit code 124. Returns ExitCode, TimedOut, Killed,
Seconds.
#>
function Invoke-BuildLockProcess {
    param([string]$FilePath, [string]$Arguments = '', $Deadline = $null, [int]$PollSeconds = 5)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath; $psi.Arguments = $Arguments; $psi.UseShellExecute = $false
    $psi.WorkingDirectory = (Get-Location).ProviderPath
    $t0 = Get-Date
    $p = [System.Diagnostics.Process]::Start($psi)
    $since = $p.StartTime.AddSeconds(-1)
    $tracked = @{}
    $timedOut = $false
    while ($true) {
        $wait = $PollSeconds * 1000
        if ($null -ne $Deadline) { $wait = [Math]::Max(0, [Math]::Min($wait, [int]([datetime]$Deadline - (Get-Date)).TotalMilliseconds)) }
        if ($p.WaitForExit($wait)) { break }
        if ($null -ne $Deadline -and (Get-Date) -ge [datetime]$Deadline) { $timedOut = $true; break }
        Update-BuildLockTracked @($p.Id) $tracked
    }
    if (-not $timedOut) {
        $p.WaitForExit()
        return [pscustomobject]@{ ExitCode = $p.ExitCode; TimedOut = $false; Killed = @(); Seconds = [int]((Get-Date) - $t0).TotalSeconds }
    }
    Update-BuildLockTracked @($p.Id) $tracked
    $killed = @(Stop-BuildLockProcesses -RootPids @($p.Id) -Tracked $tracked -Job $script:BuildLockJob -Dirs (Get-BuildLockDirs) -Since $since -Except @($script:BuildLockWatchdogPid))
    [void]$p.WaitForExit(15000)
    return [pscustomobject]@{ ExitCode = 124; TimedOut = $true; Killed = $killed; Seconds = [int]((Get-Date) - $t0).TotalSeconds }
}

# Invoke-BuildLockProcess for a cmd.exe line (redirections included). The line goes through an
# environment variable: PowerShell 5.1 breaks a native argument with inner quoted paths that
# contain spaces ("C:\Program Files (x86)\...").
function Invoke-BuildLockCmdLine([string]$Line, $Deadline = $null) {
    $env:GRID_CMDLINE = $Line
    try { return Invoke-BuildLockProcess -FilePath $env:ComSpec -Arguments '/d /c %GRID_CMDLINE%' -Deadline $Deadline }
    finally { Remove-Item Env:\GRID_CMDLINE -ErrorAction SilentlyContinue }
}

# Lines worth quoting when a step hit its deadline: libtest's "has been running for over 60
# seconds" lines (they name the hung test) and the last $Tail lines of the log.
function Get-BuildLockHangReport([string]$Log, [int]$Tail = 15) {
    $out = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $Log)) { return @("(no log at $Log)") }
    $slow = @(Select-String -LiteralPath $Log -SimpleMatch 'has been running for over 60 seconds' | ForEach-Object { $_.Line.Trim() })
    if ($slow.Count) { $out.Add("libtest, tests still running after 60 s ($($slow.Count)):"); foreach ($l in $slow) { $out.Add("  $l") } }
    else { $out.Add('libtest named no test running over 60 s') }
    $out.Add("last $Tail lines of ${Log}:")
    foreach ($l in @(Get-Content -LiteralPath $Log -Tail $Tail)) { $out.Add("  $l") }
    return @($out)
}

# ---- processes outside the lock -----------------------------------------------------------------
# Compilers, the port, the original, cargo test binaries and apitrace that do not descend from the
# lock holder: builds outside the lock, or a run that makes a timing measurement noisy.
function Get-BuildLockOutsideProcesses([int]$HolderPid) {
    $table = Get-BuildLockProcessTable
    $parent = @{}
    foreach ($p in $table) { $parent[[int]$p.ProcessId] = [int]$p.ParentProcessId }
    $out = @()
    foreach ($p in $table) {
        $name = "$($p.Name)"
        $kind = $null
        if ($name -in @('cargo.exe', 'rustc.exe', 'clippy-driver.exe')) { $kind = 'compiler' }
        elseif ($name -in @('grid-app.exe', 'grid.exe')) { $kind = 'game' }
        elseif ($name -match '^[a-z0-9_]+-[0-9a-f]{16}\.exe$') { $kind = 'test binary' }
        elseif ($name -match '(apitrace|retrace)') { $kind = 'apitrace' }
        if (-not $kind) { continue }
        $id = [int]$p.ProcessId; $mine = $false
        for ($i = 0; $i -lt 64 -and $id -and $HolderPid; $i++) {
            if ($id -eq $HolderPid) { $mine = $true; break }
            if (-not $parent.ContainsKey($id) -or $parent[$id] -eq $id) { break }
            $id = $parent[$id]
        }
        if (-not $mine) { $out += [pscustomobject]@{ Pid = [int]$p.ProcessId; Name = $name; Kind = $kind } }
    }
    return $out
}

# cargo/rustc processes not descended from the lock holder (builds outside the lock).
function Get-BuildLockOutsiders([int]$HolderPid) {
    return @(Get-BuildLockOutsideProcesses $HolderPid | Where-Object { $_.Kind -eq 'compiler' }).Count
}

function Format-BuildLockOutsiders($List) {
    $list = @($List)
    if (-not $list.Count) { return '' }
    $compilers = @($list | Where-Object { $_.Kind -eq 'compiler' }).Count
    $parts = @()
    if ($compilers) { $parts += "$compilers cargo/rustc" }
    foreach ($p in @($list | Where-Object { $_.Kind -ne 'compiler' })) { $parts += "$($p.Name) pid $($p.Pid)" }
    return ($parts -join ', ')
}

function Test-BuildLockHeldByAncestor {
    $held = [Environment]::GetEnvironmentVariable('GRID_BUILD_LOCK', 'Process')
    if (-not $held) { return $false }
    $holder = Read-BuildLockFile (Join-Path $script:BuildLockRoot 'lock')
    if (-not $holder -or "$($holder.pid)" -ne $held) { return $false }
    return -not (Test-BuildLockRecordDead $holder)
}

<#
Waits for the lock and returns a handle object for Exit-BuildLock. -TimeoutMinutes 0 waits
forever for the turn; on a timeout it throws. -HoldMinutes bounds the hold itself (must be > 0;
a long legitimate job passes a larger value explicitly): after it plus -GraceSeconds the
watchdog kills the holder's process tree and the lock is free again. The handle's Deadline is
when cooperative holders should stop their commands (Invoke-BuildLockProcess -Deadline).
#>
function Enter-BuildLock {
    param(
        [string]$Purpose = 'build',
        [double]$TimeoutMinutes = 0,
        [int]$ReportSeconds = 60,
        [double]$HoldMinutes = $script:BuildLockDefaultHoldMinutes,
        [int]$GraceSeconds = $script:BuildLockDefaultGraceSeconds
    )
    if (Test-BuildLockHeldByAncestor) {
        Write-Host "build_lock: held by parent pid $env:GRID_BUILD_LOCK; $Purpose runs under it and its limit"
        return [pscustomobject]@{ Stream = $null; Nested = $true; Purpose = $Purpose; Acquired = Get-Date; Deadline = $null; HoldMinutes = 0; WatchdogPid = 0 }
    }
    if (-not ($HoldMinutes -gt 0)) { throw "build_lock: -HoldMinutes must be > 0 (got $HoldMinutes); pass a larger explicit limit for a long job, never none" }
    $haveJobType = Initialize-BuildLockJobType
    $root = $script:BuildLockRoot
    $queueDir = Join-Path $root 'queue'
    New-Item -ItemType Directory -Force -Path $queueDir | Out-Null
    $lockPath = Join-Path $root 'lock'
    $me = [ordered]@{
        pid = $PID
        pid_start = Get-BuildLockProcessStart $PID
        worktree = (Get-Location).Path
        purpose = $Purpose
        queued_utc = (Get-Date).ToUniversalTime().ToString('o')
        started_utc = $null
        hold_minutes = $HoldMinutes
        deadline_utc = $null
        watchdog_pid = $null
        job = $null
    }
    $entry = Join-Path $queueDir ('{0:D19}-{1}.json' -f [DateTime]::UtcNow.Ticks, $PID)
    [IO.File]::WriteAllText($entry, ($me | ConvertTo-Json -Compress))
    $began = Get-Date
    $lastLine = ''; $lastReport = [DateTime]::MinValue
    try {
        while ($true) {
            $queue = @(Get-BuildLockQueue)
            $pos = 0
            for ($i = 0; $i -lt $queue.Count; $i++) { if ($queue[$i].File -eq $entry) { $pos = $i; break } }
            if ($queue.Count -eq 0 -or -not ($queue | Where-Object { $_.File -eq $entry })) {
                # Our entry was removed (should not happen); queue again at the back.
                [IO.File]::WriteAllText($entry, ($me | ConvertTo-Json -Compress)); continue
            }
            if ($pos -eq 0) {
                $stale = Read-BuildLockFile $lockPath
                $fs = $null
                try { $fs = [IO.File]::Open($lockPath, 'OpenOrCreate', 'ReadWrite', 'Read') } catch { $fs = $null }
                if ($fs) {
                    if ($stale -and $stale.pid -and $stale.pid -ne $PID) {
                        Write-Host "build_lock: previous holder pid $($stale.pid) [$($stale.purpose)] is gone; taking over its stale lock"
                    }
                    $acquired = Get-Date
                    $me.started_utc = $acquired.ToUniversalTime().ToString('o')
                    $deadline = $acquired.AddMinutes($HoldMinutes)
                    $me.deadline_utc = $deadline.ToUniversalTime().ToString('o')
                    # The job first (not yet joined), then the watchdog (outside the job, so a
                    # kill of the job's members never hits it), then this process joins: every
                    # process it starts from now on is a member, orphans included.
                    $jobName = "Local\grid-build-lock-$PID-$($acquired.Ticks)"
                    $job = [IntPtr]::Zero
                    if ($haveJobType) { $job = [GridBuildLockJob]::Create($jobName) }
                    $watch = $null
                    try {
                        $spec = [ordered]@{ root = $root; pid = $PID; pid_start = $me.pid_start; started_utc = $me.started_utc
                            deadline_utc = $me.deadline_utc; grace_s = $GraceSeconds; job = $(if ($job -ne [IntPtr]::Zero) { $jobName } else { $null })
                            worktree = $me.worktree; purpose = $Purpose; hold_minutes = $HoldMinutes }
                        $b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($spec | ConvertTo-Json -Compress)))
                        $watch = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -WindowStyle Hidden -PassThru -ArgumentList @(
                            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$script:BuildLockScript`"", '-Watchdog', $b64)
                    } catch { Write-Host "build_lock: WARNING could not start the watchdog: $($_.Exception.Message)" }
                    $joined = $false
                    if ($job -ne [IntPtr]::Zero) { $joined = [GridBuildLockJob]::Assign($job, $PID) }
                    if ($job -ne [IntPtr]::Zero -and -not $joined) { Write-Host 'build_lock: note: could not join the job object; orphaned children are found by descendant records and paths instead' }
                    $script:BuildLockJob = if ($joined) { $job } else { [IntPtr]::Zero }
                    $script:BuildLockWatchdogPid = if ($watch) { $watch.Id } else { 0 }
                    $me.watchdog_pid = $script:BuildLockWatchdogPid
                    $me.job = if ($joined) { $jobName } else { $null }
                    $bytes = [Text.Encoding]::UTF8.GetBytes(($me | ConvertTo-Json -Compress))
                    $fs.SetLength(0); $fs.Write($bytes, 0, $bytes.Length); $fs.Flush()
                    Remove-Item -LiteralPath $entry -Force -ErrorAction SilentlyContinue
                    $waited = [int]($acquired - $began).TotalSeconds
                    $outside = Format-BuildLockOutsiders (Get-BuildLockOutsideProcesses $PID)
                    $note = if ($outside) { "; outside the lock: $outside" } else { '' }
                    Write-Host "build_lock: acquired for $Purpose after $waited s, limit $HoldMinutes min$note"
                    $env:GRID_BUILD_LOCK = "$PID"
                    return [pscustomobject]@{ Stream = $fs; Nested = $false; Purpose = $Purpose; Acquired = $acquired; Deadline = $deadline
                        HoldMinutes = $HoldMinutes; WatchdogPid = $script:BuildLockWatchdogPid; WatchdogStart = $(if ($watch) { Get-BuildLockProcessStart $watch.Id } else { $null }); Job = $script:BuildLockJob }
                }
            }
            $holder = Read-BuildLockFile $lockPath
            $line = "build_lock: waiting for $Purpose, queue position $($pos + 1) of $($queue.Count); lock held by $(Format-BuildLockHolder $holder)"
            if ($line -ne $lastLine -or ((Get-Date) - $lastReport).TotalSeconds -ge $ReportSeconds) {
                Write-Host $line; $lastLine = $line; $lastReport = Get-Date
            }
            if ($TimeoutMinutes -gt 0 -and ((Get-Date) - $began).TotalMinutes -ge $TimeoutMinutes) {
                throw "build_lock: gave up after $TimeoutMinutes min waiting for $Purpose"
            }
            Start-Sleep -Seconds 3
        }
    } catch {
        Remove-Item -LiteralPath $entry -Force -ErrorAction SilentlyContinue
        throw
    }
}

function Exit-BuildLock($Lock) {
    if (-not $Lock -or $Lock.Nested -or -not $Lock.Stream) { return }
    if ($Lock.WatchdogPid -and $Lock.WatchdogStart -and (Get-BuildLockProcessStart $Lock.WatchdogPid) -eq $Lock.WatchdogStart) {
        Stop-Process -Id $Lock.WatchdogPid -Force -ErrorAction SilentlyContinue
    }
    # cargo test binaries started under this hold must not outlive it (one kept a timing script's
    # wait busy for hours on 2026-11-23); other leftovers are only named.
    $leftover = @()
    if ($Lock.Job -and $Lock.Job -ne [IntPtr]::Zero) {
        $table = Get-BuildLockProcessTable
        $byId = @{}; foreach ($p in $table) { $byId[[int]$p.ProcessId] = $p }
        foreach ($id in @(Get-BuildLockJobPids $Lock.Job)) {
            if ($id -eq $PID -or -not $byId.ContainsKey($id)) { continue }
            $name = "$($byId[$id].Name)"
            if ($name -match '^[a-z0-9_]+-[0-9a-f]{16}\.exe$') {
                Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
                $leftover += "$name($id) killed"
            } elseif ($name -notin @('conhost.exe')) { $leftover += "$name($id) still running" }
        }
    }
    try { $Lock.Stream.SetLength(0) } catch {}
    $Lock.Stream.Close()
    $Lock.Stream = $null
    $script:BuildLockWatchdogPid = 0
    if ($env:GRID_BUILD_LOCK -eq "$PID") { Remove-Item Env:\GRID_BUILD_LOCK -ErrorAction SilentlyContinue }
    $held = [int]((Get-Date) - $Lock.Acquired).TotalSeconds
    try {
        [IO.File]::AppendAllText((Join-Path $script:BuildLockRoot 'holds.log'),
            ("{0}`t{1}`t{2}`t{3}`t{4}`t{5}`r`n" -f [DateTime]::UtcNow.ToString('o'), $PID, $held, $Lock.HoldMinutes, $Lock.Purpose, (Get-Location).Path))
    } catch {}
    $note = if ($leftover.Count) { "; processes started under it: $($leftover -join ', ')" } else { '' }
    Write-Host "build_lock: released by $($Lock.Purpose) after $held s (limit $($Lock.HoldMinutes) min)$note"
}

function Show-BuildLockStatus {
    $holder = Read-BuildLockFile (Join-Path $script:BuildLockRoot 'lock')
    $free = $true
    if (Test-Path -LiteralPath (Join-Path $script:BuildLockRoot 'lock')) {
        try { $fs = [IO.File]::Open((Join-Path $script:BuildLockRoot 'lock'), 'Open', 'ReadWrite', 'Read'); $fs.Close() } catch { $free = $false }
    }
    if ($free) { Write-Output 'build_lock: free' }
    else {
        Write-Output "build_lock: held by $(Format-BuildLockHolder $holder)"
        if ($holder -and $holder.deadline_utc) {
            $over = ((Get-Date).ToUniversalTime() - [DateTime]::Parse($holder.deadline_utc).ToUniversalTime()).TotalMinutes
            if ($over -gt 0) {
                $wd = if ($holder.watchdog_pid -and (Get-BuildLockProcessStart ([int]$holder.watchdog_pid))) { "watchdog pid $($holder.watchdog_pid) running" } else { 'no watchdog running' }
                Write-Output ("build_lock: OVER LIMIT by {0:N1} min ({1})" -f $over, $wd)
            }
        } elseif ($holder) { Write-Output 'build_lock: holder records no limit (an older build_lock.ps1)' }
    }
    $i = 0
    foreach ($q in @(Get-BuildLockQueue)) {
        $i++; Write-Output ("  {0}. pid {1} [{2}] in {3}, queued {4}" -f $i, $q.Record.pid, $q.Record.purpose, $q.Record.worktree, $q.Record.queued_utc)
    }
    $outside = Format-BuildLockOutsiders (Get-BuildLockOutsideProcesses $(if ($holder -and -not $free) { [int]$holder.pid } else { 0 }))
    if ($outside) { Write-Output "build_lock: outside the lock: $outside" }
}

# The watchdog of one hold (started by Enter-BuildLock; not for direct use).
function Invoke-BuildLockWatchdog([string]$Spec) {
    $s = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Spec)) | ConvertFrom-Json
    $script:BuildLockRoot = $s.root
    $lockPath = Join-Path $s.root 'lock'
    $deadline = [DateTime]::Parse($s.deadline_utc).ToUniversalTime().AddSeconds([int]$s.grace_s)
    try { $holder = Get-Process -Id ([int]$s.pid) -ErrorAction Stop } catch { return }
    if ($holder.StartTime.ToUniversalTime().ToString('o') -ne $s.pid_start) { return }
    $job = [IntPtr]::Zero
    $tracked = @{}
    $haveJobType = $false
    $sameHold = {
        $r = Read-BuildLockFile $lockPath
        return ($r -and "$($r.pid)" -eq "$($s.pid)" -and $r.pid_start -eq $s.pid_start -and $r.started_utc -eq $s.started_utc)
    }
    while ($true) {
        if ($s.job -and $job -eq [IntPtr]::Zero) {
            if (-not $haveJobType) { $haveJobType = Initialize-BuildLockJobType }
            if ($haveJobType) { $job = [GridBuildLockJob]::Open($s.job) }
        }
        $left = ($deadline - [DateTime]::UtcNow).TotalMilliseconds
        if ($left -le 0) { break }
        if ($holder.WaitForExit([int][Math]::Min(10000, $left))) { return }
        if (-not (& $sameHold)) { return }
        # Without a job, record the descendants so orphans stay known.
        if ($job -eq [IntPtr]::Zero) { Update-BuildLockTracked @([int]$s.pid) $tracked }
    }
    if (-not (& $sameHold)) { return }
    $since = [DateTime]::Parse($s.started_utc).ToLocalTime()
    $dirs = @(Get-BuildLockDirs $s.worktree)
    $killed = @()
    if ($job -ne [IntPtr]::Zero) {
        $names = @{}; foreach ($p in Get-BuildLockProcessTable) { $names[[int]$p.ProcessId] = $p.Name }
        $killed += @(Get-BuildLockJobPids $job | ForEach-Object { "$($names[$_])($_)" })
        [void][GridBuildLockJob]::Terminate($job, 124)
    }
    $killed = @(@($killed) + @(Stop-BuildLockProcesses -RootPids @([int]$s.pid) -Tracked $tracked -Job $job -Dirs $dirs -Since $since) | Sort-Object -Unique)
    $free = $holder.WaitForExit(15000)
    $held = '{0:N1}' -f ([DateTime]::UtcNow - [DateTime]::Parse($s.started_utc).ToUniversalTime()).TotalMinutes
    Write-BuildLockTimeoutLine ("watchdog: pid {0} [{1}] in {2} held the lock {3} min, limit {4} min + {5} s grace; killed {6} process(es): {7}; lock {8}" -f
        $s.pid, $s.purpose, $s.worktree, $held, $s.hold_minutes, $s.grace_s, $killed.Count, ($killed -join ' '), $(if ($free) { 'free' } else { 'STILL HELD (holder did not exit)' }))
}

function Expand-CargoAlias {
    # A worktree under C:\Dev\Reassembly\.claude\worktrees also reads the main checkout's
    # .cargo/config.toml, and cargo joins the two alias arrays: `cargo td` then fails with
    # "the argument '--lib' cannot be used multiple times" (docs/TOOLCHAIN.md, P9). The
    # wrapper expands an array alias of this checkout's own config itself, so cargo gets the
    # builtin command. Other arguments and unknown aliases pass unchanged.
    param([string[]]$CargoArgs, [string]$ConfigPath = (Join-Path (Split-Path $PSScriptRoot -Parent) '.cargo\config.toml'))
    if ($CargoArgs.Count -eq 0 -or -not (Test-Path $ConfigPath)) { return ,$CargoArgs }
    $inAlias = $false
    foreach ($line in Get-Content $ConfigPath) {
        if ($line -match '^\s*\[(.+)\]\s*$') { $inAlias = ($Matches[1].Trim() -eq 'alias'); continue }
        if (-not $inAlias) { continue }
        if ($line -match '^\s*([A-Za-z0-9_-]+)\s*=\s*\[(.*)\]\s*$' -and $Matches[1] -eq $CargoArgs[0]) {
            $expanded = @([regex]::Matches($Matches[2], '"([^"]*)"') | ForEach-Object { $_.Groups[1].Value })
            return ,@($expanded + @($CargoArgs | Select-Object -Skip 1))
        }
    }
    return ,$CargoArgs
}

# One argument quoted for CreateProcess (the MSVC runtime's parsing rules).
function ConvertTo-BuildLockArgument([string]$Arg) {
    if ($Arg.Length -and $Arg -notmatch '[\s"]') { return $Arg }
    $sb = New-Object System.Text.StringBuilder; [void]$sb.Append('"'); $slashes = 0
    foreach ($ch in $Arg.ToCharArray()) {
        if ($ch -eq '\') { $slashes++; continue }
        if ($ch -eq '"') { [void]$sb.Append('\' * (2 * $slashes + 1)); [void]$sb.Append('"') }
        else { [void]$sb.Append('\' * $slashes); [void]$sb.Append($ch) }
        $slashes = 0
    }
    [void]$sb.Append('\' * (2 * $slashes)); [void]$sb.Append('"')
    return $sb.ToString()
}

# ---- wrapper mode: powershell -File tools\build_lock.ps1 [-Status] [-HoldMinutes N] COMMAND ARGS... --
if ($MyInvocation.InvocationName -ne '.') {
    $ErrorActionPreference = 'Stop'
    $rest = @($args)
    if ($rest.Count -eq 2 -and $rest[0] -eq '-Watchdog') { Invoke-BuildLockWatchdog $rest[1]; exit 0 }
    if ($rest.Count -eq 1 -and $rest[0] -eq '-Status') { Show-BuildLockStatus; exit 0 }
    $hold = $script:BuildLockDefaultHoldMinutes
    $grace = $script:BuildLockDefaultGraceSeconds
    while ($rest.Count -ge 2 -and $rest[0] -in @('-HoldMinutes', '-GraceSeconds')) {
        $v = 0.0
        if (-not [double]::TryParse("$($rest[1])", [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$v)) { Write-Error "build_lock: $($rest[0]) needs a number, got '$($rest[1])'"; exit 2 }
        if ($rest[0] -eq '-HoldMinutes') { $hold = $v } else { $grace = [int]$v }
        $rest = @($rest | Select-Object -Skip 2)
    }
    if ($rest.Count -gt 0 -and $rest[0] -eq '--') { $rest = @($rest | Select-Object -Skip 1) }
    if ($rest.Count -eq 0) { Write-Error 'usage: build_lock.ps1 [-Status] [-HoldMinutes N] COMMAND [ARGS...]'; exit 2 }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH" }
    $command = $rest[0]
    $commandArgs = @($rest | Select-Object -Skip 1)
    if ($command -eq 'cargo') { $commandArgs = Expand-CargoAlias $commandArgs }
    $resolved = Get-Command $command -CommandType Application, ExternalScript -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $resolved) { Write-Error "build_lock: '$command' is not an executable or a script"; exit 2 }
    $argText = (@($commandArgs | ForEach-Object { ConvertTo-BuildLockArgument "$_" }) -join ' ')
    if ($resolved.CommandType -eq 'ExternalScript') {
        $file = Join-Path $PSHOME 'powershell.exe'
        $argText = "-NoProfile -ExecutionPolicy Bypass -File $(ConvertTo-BuildLockArgument $resolved.Source) $argText"
    } else { $file = $resolved.Source }
    $purpose = ("$command " + ($commandArgs -join ' ')).Trim()
    $lock = Enter-BuildLock -Purpose $purpose -HoldMinutes $hold -GraceSeconds $grace
    $code = 1
    try {
        $ErrorActionPreference = 'Continue'
        $r = Invoke-BuildLockProcess -FilePath $file -Arguments $argText -Deadline $lock.Deadline
        $code = $r.ExitCode
        if ($r.TimedOut) {
            Write-BuildLockTimeoutLine ("wrapper: pid {0} [{1}] in {2} reached its limit of {3} min; killed {4} process(es): {5}" -f $PID, $purpose, (Get-Location).Path, $hold, $r.Killed.Count, ($r.Killed -join ' '))
        }
    } finally { Exit-BuildLock $lock }
    if ($r -and $r.TimedOut) {
        Write-Host ("build_lock: TIMEOUT {0} ran {1} s, over its limit of {2} min; killed {3} process(es) ({4}); lock released; exit 124; logged in {5}" -f
            $purpose, $r.Seconds, $hold, $r.Killed.Count, ($r.Killed -join ' '), (Join-Path $script:BuildLockRoot 'timeouts.log'))
    }
    exit $code
}
