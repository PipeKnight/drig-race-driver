<#
.SYNOPSIS
One cargo build at a time across every checkout on this PC (docs/TOOLCHAIN.md, "Build lock").

.DESCRIPTION
The PC is memory-bound (64 GiB, commit ~62 GiB under load): two release builds at once push
it into paging. Every script that builds takes this lock first, and so should ad-hoc cargo
commands of the sessions:

    powershell -NoProfile -File tools\build_lock.ps1 cargo build --release -p grid-app
    (Bash:  powershell.exe -NoProfile -File tools/build_lock.ps1 cargo clippy --release -p grid-app -- -D warnings)

The wrapper waits for its turn, runs the command, releases the lock and exits with the
command's exit code. Scripts dot-source it instead:

    . (Join-Path $PSScriptRoot 'build_lock.ps1')
    $lock = Enter-BuildLock -Purpose 'update_prototype'
    try { ... cargo ... } finally { Exit-BuildLock $lock }

How it works (state lives in C:\cargo-build\build-lock, beside the build dirs):
- `lock`: the holder keeps this file open with no write sharing. Windows closes the handle when
  the holder dies, so a crashed holder never leaves a stuck lock. Its content (JSON: pid,
  process start time, worktree, purpose, start time) is for the waiters to print; a leftover
  file from a dead pid is reported as stale and taken over.
- `queue\<ticks>-<pid>.json`: one entry per waiter, first come first served. Entries of dead
  pids (or of a pid reused by another process: start time differs) are deleted by whoever
  sees them. Waiters print their queue position and the holder when either changes, and
  every -ReportSeconds.
- Re-entrant: the holder sets GRID_BUILD_LOCK=<pid>; a child script that sees its ancestor
  still holding the lock (tools\release_playable.ps1 -> update_prototype.ps1) runs without
  queueing behind itself.
- Builds started outside the lock (sessions that run cargo directly) are not stopped; the
  holder prints how many cargo/rustc processes run outside it.

Inspect the lock and the queue:  powershell -NoProfile -File tools\build_lock.ps1 -Status
#>

$script:BuildLockRoot = 'C:\cargo-build\build-lock'

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
    return "pid $($Holder.pid) [$($Holder.purpose)] in $($Holder.worktree), $since min"
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

# cargo/rustc processes not descended from the lock holder (builds outside the lock).
function Get-BuildLockOutsiders([int]$HolderPid) {
    $all = @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name -ErrorAction SilentlyContinue)
    $parent = @{}
    foreach ($p in $all) { $parent[[int]$p.ProcessId] = [int]$p.ParentProcessId }
    $count = 0
    foreach ($p in $all) {
        if ($p.Name -notin @('cargo.exe', 'rustc.exe', 'clippy-driver.exe')) { continue }
        $id = [int]$p.ProcessId; $mine = $false
        for ($i = 0; $i -lt 64 -and $id; $i++) {
            if ($id -eq $HolderPid) { $mine = $true; break }
            if (-not $parent.ContainsKey($id) -or $parent[$id] -eq $id) { break }
            $id = $parent[$id]
        }
        if (-not $mine) { $count++ }
    }
    return $count
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
forever; on a timeout it throws.
#>
function Enter-BuildLock {
    param(
        [string]$Purpose = 'build',
        [double]$TimeoutMinutes = 0,
        [int]$ReportSeconds = 60
    )
    if (Test-BuildLockHeldByAncestor) {
        Write-Host "build_lock: held by parent pid $env:GRID_BUILD_LOCK; $Purpose runs under it"
        return [pscustomobject]@{ Stream = $null; Nested = $true; Purpose = $Purpose; Acquired = Get-Date }
    }
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
                    $me.started_utc = (Get-Date).ToUniversalTime().ToString('o')
                    $bytes = [Text.Encoding]::UTF8.GetBytes(($me | ConvertTo-Json -Compress))
                    $fs.SetLength(0); $fs.Write($bytes, 0, $bytes.Length); $fs.Flush()
                    Remove-Item -LiteralPath $entry -Force -ErrorAction SilentlyContinue
                    $waited = [int]((Get-Date) - $began).TotalSeconds
                    $outside = Get-BuildLockOutsiders $PID
                    $note = if ($outside) { "; $outside cargo/rustc process(es) running outside the lock" } else { '' }
                    Write-Host "build_lock: acquired for $Purpose after $waited s$note"
                    $env:GRID_BUILD_LOCK = "$PID"
                    return [pscustomobject]@{ Stream = $fs; Nested = $false; Purpose = $Purpose; Acquired = Get-Date }
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
    try { $Lock.Stream.SetLength(0) } catch {}
    $Lock.Stream.Close()
    $Lock.Stream = $null
    if ($env:GRID_BUILD_LOCK -eq "$PID") { Remove-Item Env:\GRID_BUILD_LOCK -ErrorAction SilentlyContinue }
    $held = [int]((Get-Date) - $Lock.Acquired).TotalSeconds
    Write-Host "build_lock: released by $($Lock.Purpose) after $held s"
}

function Show-BuildLockStatus {
    $holder = Read-BuildLockFile (Join-Path $script:BuildLockRoot 'lock')
    $free = $true
    try { $fs = [IO.File]::Open((Join-Path $script:BuildLockRoot 'lock'), 'OpenOrCreate', 'ReadWrite', 'Read'); $fs.Close() } catch { $free = $false }
    if ($free) { Write-Output 'build_lock: free' } else { Write-Output "build_lock: held by $(Format-BuildLockHolder $holder)" }
    $i = 0
    foreach ($q in @(Get-BuildLockQueue)) {
        $i++; Write-Output ("  {0}. pid {1} [{2}] in {3}, queued {4}" -f $i, $q.Record.pid, $q.Record.purpose, $q.Record.worktree, $q.Record.queued_utc)
    }
}

# ---- wrapper mode: powershell -File tools\build_lock.ps1 [-Status] COMMAND ARGS... ----------
if ($MyInvocation.InvocationName -ne '.') {
    $ErrorActionPreference = 'Stop'
    $rest = @($args)
    if ($rest.Count -eq 1 -and $rest[0] -eq '-Status') { Show-BuildLockStatus; exit 0 }
    if ($rest.Count -gt 0 -and $rest[0] -eq '--') { $rest = @($rest | Select-Object -Skip 1) }
    if ($rest.Count -eq 0) { Write-Error 'usage: build_lock.ps1 [-Status] COMMAND [ARGS...]'; exit 2 }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH" }
    $command = $rest[0]
    $commandArgs = @($rest | Select-Object -Skip 1)
    $lock = Enter-BuildLock -Purpose ("$command " + ($commandArgs -join ' ')).Trim()
    $code = 1
    try {
        # Native stderr must not become a terminating error under Stop (PowerShell 5.1).
        $ErrorActionPreference = 'Continue'
        & $command @commandArgs
        $code = $LASTEXITCODE
        if ($null -eq $code) { $code = if ($?) { 0 } else { 1 } }
    } finally { Exit-BuildLock $lock }
    exit $code
}
