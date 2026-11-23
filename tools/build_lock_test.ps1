<#
.SYNOPSIS
Self-test of the build lock's hold limit (tools\build_lock.ps1) against a private lock root; prints one verdict line.

.DESCRIPTION
Sets GRID_BUILD_LOCK_ROOT to a new folder (-Root, default a new folder in %TEMP%), so the real
lock in C:\cargo-build\build-lock and its holders are never touched, and runs five cases with
dummy PowerShell processes that carry a unique tag in their command line (the survivor check and
the final cleanup look only at those):

  wrapper-timeout  build_lock.ps1 -HoldMinutes 0.2 with a child that starts a grandchild through
                   an intermediate process that exits at once (an orphan, like a cargo test
                   binary) and then sleeps: exit 124, a "wrapper:" line in timeouts.log, no
                   survivor (child, orphan, watchdog), lock free.
  normal           a command that finishes in time: its exit code (3) and output pass through,
                   holds.log gains a line, no timeout line, lock free.
  nested           a holder that runs the wrapper as its child: the child runs under the
                   parent's hold at once ("held by parent") and exits 0.
  watchdog         a holder that ignores its limit (-HoldMinutes 0.05, -GraceSeconds 15) with an
                   orphaned grandchild: -Status prints OVER LIMIT; the watchdog then kills the
                   holder and the orphan, writes a "watchdog:" line, and the lock is free.
  step-timeout     the prepush step runner's functions (Invoke-BuildLockCmdLine with a deadline,
                   Get-BuildLockHangReport) on a fake test binary that prints libtest's "has
                   been running for over 60 seconds" line and hangs: exit 124, the report quotes
                   that line, no survivor.

About 90 s. Exit code 0 when every case passed.

    powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_lock_test.ps1 [-Root DIR]
#>
param([string]$Root)
$ErrorActionPreference = 'Stop'
if (-not $Root) { $Root = Join-Path ([IO.Path]::GetTempPath()) ('build-lock-test-{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), $PID) }
$Root = [IO.Path]::GetFullPath($Root)
New-Item -ItemType Directory -Force -Path $Root | Out-Null
$env:GRID_BUILD_LOCK_ROOT = $Root
Remove-Item Env:\GRID_BUILD_LOCK -ErrorAction SilentlyContinue
$lockScript = Join-Path $PSScriptRoot 'build_lock.ps1'
$ps = Join-Path $PSHOME 'powershell.exe'
$tag = "bltest$PID$((Get-Date).Ticks % 100000)"
$work = Join-Path $Root 'scripts'
New-Item -ItemType Directory -Force -Path $work | Out-Null
# No Cargo.toml here: the "started from this checkout's target dir" rule matches nothing.
Set-Location -LiteralPath $Root

# ---- dummy processes ----------------------------------------------------------------------------
Set-Content -LiteralPath (Join-Path $work 'sleeper.ps1') -Encoding ascii -Value 'param([string]$Tag) Start-Sleep -Seconds 600'
# Starts the sleeper and exits at once: the sleeper's parent is gone (an orphan).
Set-Content -LiteralPath (Join-Path $work 'spawn.ps1') -Encoding ascii -Value @'
param([string]$Tag)
Start-Process -FilePath "$PSHOME\powershell.exe" -WindowStyle Hidden -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSScriptRoot\sleeper.ps1`"", $Tag)
'@
Set-Content -LiteralPath (Join-Path $work 'child.ps1') -Encoding ascii -Value @'
param([string]$Tag, [int]$Sleep = 600)
& "$PSHOME\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\spawn.ps1" $Tag
[Console]::Out.WriteLine("child $Tag started its orphan"); [Console]::Out.Flush()
Start-Sleep -Seconds $Sleep
exit 3
'@
Set-Content -LiteralPath (Join-Path $work 'faketest.ps1') -Encoding ascii -Value @'
param([string]$Tag)
& "$PSHOME\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\spawn.ps1" $Tag
foreach ($l in @('running 2 tests', 'test quick::one ... ok', 'test hung::forever has been running for over 60 seconds')) { [Console]::Out.WriteLine($l) }
[Console]::Out.Flush()
Start-Sleep -Seconds 600
'@
# A holder of the lock. Mode nested: runs the wrapper as its child. Mode ignore: starts an orphan
# and sleeps past its limit.
Set-Content -LiteralPath (Join-Path $work 'holder.ps1') -Encoding ascii -Value @'
param([string]$LockScript, [string]$Mode, [string]$Tag, [string]$Result)
. $LockScript
if ($Mode -eq 'nested') {
    $lock = Enter-BuildLock -Purpose "test nested $Tag" -HoldMinutes 2
    $t0 = Get-Date
    $ErrorActionPreference = 'Continue'
    $out = & "$PSHOME\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File $LockScript "$PSHOME\powershell.exe" -NoProfile -Command "exit 0" 2>&1 | Out-String
    $code = $LASTEXITCODE
    Exit-BuildLock $lock
    [IO.File]::WriteAllText($Result, (@{ code = $code; out = $out; seconds = ((Get-Date) - $t0).TotalSeconds } | ConvertTo-Json -Compress))
} else {
    $lock = Enter-BuildLock -Purpose "test ignore-limit $Tag" -HoldMinutes 0.05 -GraceSeconds 15
    & "$PSHOME\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\spawn.ps1" $Tag
    Start-Sleep -Seconds 600
}
'@
# The prepush step runner on a hung fake test.
Set-Content -LiteralPath (Join-Path $work 'harness.ps1') -Encoding ascii -Value @'
param([string]$LockScript, [string]$Tag, [string]$Result)
. $LockScript
$lock = Enter-BuildLock -Purpose "test step-timeout $Tag" -HoldMinutes 2
$log = Join-Path (Split-Path $Result) 'step.log'
$r = Invoke-BuildLockCmdLine "`"$PSHOME\powershell.exe`" -NoProfile -ExecutionPolicy Bypass -File `"$PSScriptRoot\faketest.ps1`" $Tag > `"$log`" 2>&1" (Get-Date).AddSeconds(10)
$report = @(Get-BuildLockHangReport $log)
Exit-BuildLock $lock
[IO.File]::WriteAllText($Result, (@{ code = $r.ExitCode; timed_out = $r.TimedOut; killed = @($r.Killed); report = $report; seconds = $r.Seconds } | ConvertTo-Json -Compress))
'@

function Get-Tagged {
    @(Get-CimInstance Win32_Process -Property ProcessId, CommandLine | Where-Object { $_.ProcessId -ne $PID -and "$($_.CommandLine)" -like "*$tag*" })
}
# Watchdogs of this test's lock root (their spec is base64 JSON).
function Get-TestWatchdogs {
    @(Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" -Property ProcessId, CommandLine | Where-Object {
        $m = [regex]::Match("$($_.CommandLine)", '-Watchdog\s+(\S+)')
        if (-not $m.Success) { return $false }
        try { ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($m.Groups[1].Value)) | ConvertFrom-Json).root -eq $Root } catch { $false }
    })
}
function Get-Survivors {
    Start-Sleep -Seconds 2
    $s = @(Get-Tagged) + @(Get-TestWatchdogs)
    return @($s | ForEach-Object { "pid $($_.ProcessId)" })
}
function Test-Free { return ((& $ps -NoProfile -ExecutionPolicy Bypass -File $lockScript -Status | Out-String) -match 'build_lock: free') }
function Get-LogLines([string]$Name) {
    $f = Join-Path $Root $Name
    if (Test-Path -LiteralPath $f) { return @(Get-Content -LiteralPath $f) } else { return @() }
}
# Output to files, not a pipe: a survivor holding the pipe must not hang the test.
$script:wrapperRuns = 0
function Invoke-Wrapper([string[]]$WrapperArgs) {
    $script:wrapperRuns++
    $out = Join-Path $Root "wrapper$($script:wrapperRuns).out.txt"
    $quoted = @(@('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $lockScript) + $WrapperArgs | ForEach-Object { if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ } })
    $t0 = Get-Date
    $p = Start-Process -FilePath $ps -ArgumentList $quoted -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError "$out.err"
    $null = $p.Handle
    if (-not $p.WaitForExit(120000)) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; $null = $p.WaitForExit(10000) }
    $text = (@(Get-Content -LiteralPath $out, "$out.err" -ErrorAction SilentlyContinue) -join "`n")
    return [pscustomobject]@{ Code = $p.ExitCode; Out = $text; Seconds = [int]((Get-Date) - $t0).TotalSeconds }
}

$results = New-Object System.Collections.Generic.List[object]
function Add-Result([string]$Name, [string[]]$Problems, [string]$Note) {
    $results.Add([pscustomobject]@{ Name = $Name; Ok = ($Problems.Count -eq 0); Problems = $Problems; Note = $Note })
}
$childScript = Join-Path $work 'child.ps1'

try {
    # 1. wrapper-timeout
    $p = @()
    $r = Invoke-Wrapper @('-HoldMinutes', '0.2', $ps, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $childScript, $tag)
    if ($r.Code -ne 124) { $p += "exit $($r.Code), expected 124" }
    if ($r.Out -notmatch 'TIMEOUT') { $p += 'no TIMEOUT verdict' }
    if ($r.Out -notmatch "child $tag started its orphan") { $p += "the child's output did not pass through" }
    if ($r.Seconds -lt 10 -or $r.Seconds -gt 60) { $p += "took $($r.Seconds) s for a 12 s limit" }
    if (-not @(Get-LogLines 'timeouts.log' | Where-Object { $_ -match 'wrapper: .*reached its limit' }).Count) { $p += 'no wrapper line in timeouts.log' }
    $sv = @(Get-Survivors); if ($sv.Count) { $p += "survivors: $($sv -join ', ')" }
    if (-not (Test-Free)) { $p += 'lock not free' }
    Add-Result 'wrapper-timeout' $p "$($r.Seconds) s"

    # 2. normal
    $p = @()
    $timeouts0 = @(Get-LogLines 'timeouts.log').Count; $holds0 = @(Get-LogLines 'holds.log').Count
    $r = Invoke-Wrapper @('-HoldMinutes', '1', $ps, '-NoProfile', '-Command', "Write-Output 'ok $tag'; exit 3")
    if ($r.Code -ne 3) { $p += "exit $($r.Code), expected 3" }
    if ($r.Out -notmatch "ok $tag") { $p += 'output did not pass through' }
    if (@(Get-LogLines 'timeouts.log').Count -ne $timeouts0) { $p += 'a timeout was logged' }
    if (@(Get-LogLines 'holds.log').Count -ne $holds0 + 1) { $p += 'holds.log did not gain one line' }
    if (-not (Test-Free)) { $p += 'lock not free' }
    Add-Result 'normal' $p "$($r.Seconds) s"

    # 3. nested
    $p = @()
    $res = Join-Path $Root 'nested.json'
    $h = Start-Process -FilePath $ps -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$(Join-Path $work 'holder.ps1')`"", "`"$lockScript`"", 'nested', $tag, "`"$res`"")
    if (-not $h.WaitForExit(60000)) { $p += 'holder did not finish in 60 s'; Stop-Process -Id $h.Id -Force -ErrorAction SilentlyContinue }
    if (Test-Path -LiteralPath $res) {
        $j = Get-Content -Raw -LiteralPath $res | ConvertFrom-Json
        if ($j.code -ne 0) { $p += "inner wrapper exit $($j.code)" }
        if ($j.out -notmatch 'held by parent') { $p += 'inner wrapper did not run under the parent hold' }
        if ($j.seconds -gt 30) { $p += "inner wrapper took $([int]$j.seconds) s" }
    } else { $p += 'no result file' }
    if (-not (Test-Free)) { $p += 'lock not free' }
    Add-Result 'nested' $p ''

    # 4. watchdog (and -Status over the limit)
    $p = @()
    $t0 = Get-Date
    $h = Start-Process -FilePath $ps -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$(Join-Path $work 'holder.ps1')`"", "`"$lockScript`"", 'ignore', $tag, 'none')
    $held = $false
    for ($i = 0; $i -lt 40 -and -not $held; $i++) {
        Start-Sleep -Milliseconds 500
        $rec = $null; try { $rec = Get-Content -Raw -LiteralPath (Join-Path $Root 'lock') -ErrorAction Stop | ConvertFrom-Json } catch {}
        $held = ($rec -and $rec.pid -eq $h.Id -and $rec.deadline_utc)
    }
    if (-not $held) { $p += 'holder never took the lock' }
    else {
        $wait = ([DateTime]::Parse($rec.deadline_utc).ToUniversalTime() - [DateTime]::UtcNow).TotalSeconds + 2
        if ($wait -gt 0) { Start-Sleep -Seconds ([Math]::Ceiling($wait)) }
        $status = & $ps -NoProfile -ExecutionPolicy Bypass -File $lockScript -Status | Out-String
        if ($status -notmatch 'OVER LIMIT') { $p += "-Status did not flag the holder: $($status.Trim())" }
        if ($status -notmatch 'watchdog pid \d+ running') { $p += '-Status shows no running watchdog' }
    }
    if (-not $h.WaitForExit(60000)) { $p += 'watchdog did not kill the holder within 60 s'; Stop-Process -Id $h.Id -Force -ErrorAction SilentlyContinue }
    $wdSeconds = [int]((Get-Date) - $t0).TotalSeconds
    Start-Sleep -Seconds 3
    $line = @(Get-LogLines 'timeouts.log' | Where-Object { $_ -match "watchdog: pid $($h.Id) " })
    if (-not $line.Count) { $p += 'no watchdog line in timeouts.log' } elseif ($line[-1] -notmatch 'lock free') { $p += "watchdog line: $($line[-1])" }
    $sv = @(Get-Survivors); if ($sv.Count) { $p += "survivors: $($sv -join ', ')" }
    if (-not (Test-Free)) { $p += 'lock not free' }
    Add-Result 'watchdog' $p "$wdSeconds s for a 3 s limit + 15 s grace"

    # 5. step-timeout
    $p = @()
    $res = Join-Path $Root 'step.json'
    $h = Start-Process -FilePath $ps -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$(Join-Path $work 'harness.ps1')`"", "`"$lockScript`"", $tag, "`"$res`"")
    if (-not $h.WaitForExit(90000)) { $p += 'harness did not finish in 90 s'; Stop-Process -Id $h.Id -Force -ErrorAction SilentlyContinue }
    if (Test-Path -LiteralPath $res) {
        $j = Get-Content -Raw -LiteralPath $res | ConvertFrom-Json
        if ($j.code -ne 124 -or -not $j.timed_out) { $p += "exit $($j.code), timed out $($j.timed_out)" }
        if (-not (@($j.report) -match 'hung::forever has been running for over 60 seconds').Count) { $p += 'report does not quote the libtest line' }
        if (-not (@($j.report) -match 'last 15 lines').Count) { $p += 'report has no log tail' }
        $stepNote = "$($j.seconds) s, killed $(@($j.killed).Count)"
    } else { $p += 'no result file'; $stepNote = '' }
    $sv = @(Get-Survivors); if ($sv.Count) { $p += "survivors: $($sv -join ', ')" }
    if (-not (Test-Free)) { $p += 'lock not free' }
    Add-Result 'step-timeout' $p $stepNote
} finally {
    # Only processes this test started (tagged), and only the watchdogs of its own root.
    foreach ($x in @(Get-Tagged) + @(Get-TestWatchdogs)) { Stop-Process -Id $x.ProcessId -Force -ErrorAction SilentlyContinue }
}

$failed = @($results | Where-Object { -not $_.Ok })
$parts = @($results | ForEach-Object { if ($_.Ok) { "$($_.Name) ok$(if ($_.Note) { " ($($_.Note))" })" } else { "$($_.Name) FAIL ($($_.Problems -join '; '))" } })
$verdict = if ($failed.Count) { 'FAIL' } else { 'PASS' }
"build_lock_test: $verdict $($results.Count - $failed.Count)/$($results.Count): $($parts -join ', '); root $Root" | Tee-Object -FilePath (Join-Path $Root 'verdict.txt')
exit $(if ($failed.Count) { 1 } else { 0 })
