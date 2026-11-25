<#
.SYNOPSIS
Run one libtest binary N times in parallel mode and print a crash verdict (F922: the grid-app test process killed by an access violation in a GPU driver).

.DESCRIPTION
Each run starts the test exe with --include-ignored (plus -Filter) and the GRID_* inputs prepush gives the
tests, waits up to -Minutes, and records the exit code. A run that ends with an exception code is matched to
its Windows Application event 1000 by process id (faulting module and offset). The test exe's own
test_crash_report lines (grid-app test builds on Windows: faulting thread name and stack) are copied to the
run's .crash.txt. Prints one line per run and a verdict: runs, passes, crashes, timeouts, failures and the
faulting modules. Full output stays in -OutDir. Run it under the build lock:
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_lock.ps1 -HoldMinutes 90 powershell -NoProfile -ExecutionPolicy Bypass -File tools\test_crash_repro.ps1 -Exe <grid_app-*.exe> -Runs 10 -OutDir local\gpucrash\before
-Filter takes libtest substring filters (any matches; a comma-separated string works too); none runs the whole binary. The tests open no audio
device, so no mute is needed. -WatchModule names a module sampled every 20 s in the test process (default
nvwgf2umx.dll): the verdict counts the runs in which it was seen loaded.
#>
param(
    [Parameter(Mandatory)][string]$Exe,
    [int]$Runs = 10,
    [string[]]$Filter = @(),
    [double]$Minutes = 15,
    [string]$OutDir = 'local\test-crash-repro',
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$PrivateDir = '',
    # Sampled every 20 s in the test process: whether this module (the driver that faulted) is loaded.
    [string]$WatchModule = 'nvwgf2umx.dll'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (-not $PrivateDir) {
    $PrivateDir = Join-Path $repo 'local'
    if (-not (Test-Path -LiteralPath (Join-Path $PrivateDir 'body-contact-directions.bin'))) {
        $common = (git -C $repo rev-parse --path-format=absolute --git-common-dir | Select-Object -First 1).Trim()
        $PrivateDir = Join-Path (Split-Path -Parent $common) 'local'
    }
}
$env:GRID_GAME_ROOT = [IO.Path]::GetFullPath($GameRoot)
$env:GRID_CONTACT_DIRECTIONS = Join-Path $PrivateDir 'body-contact-directions.bin'
$env:GRID_BODY_EVENT_PARAMETERS = Join-Path $PrivateDir 'body-impact-parameters.bin'
$env:GRID_DEFORMATION_PARAMETERS = Join-Path $PrivateDir 'deformation-parameters.bin'
Remove-Item Env:\GRID_AI_CHARACTERISTICS, Env:\RUST_TEST_THREADS, Env:\RUST_MIN_STACK -ErrorAction SilentlyContinue
$Exe = (Resolve-Path -LiteralPath $Exe).Path
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path -LiteralPath $OutDir).Path
# build_lock.ps1 passes an array parameter on as one comma-joined argument.
$Filter = @($Filter | ForEach-Object { $_ -split ',' } | ForEach-Object Trim | Where-Object { $_ })
$testArgs = @('--include-ignored') + $Filter
$results = @()
for ($i = 1; $i -le $Runs; $i++) {
    $stem = Join-Path $OutDir ('run{0:d2}' -f $i)
    $started = Get-Date
    $p = Start-Process -FilePath $Exe -ArgumentList $testArgs -NoNewWindow -PassThru -RedirectStandardOutput "$stem.out.txt" -RedirectStandardError "$stem.err.txt"
    $null = $p.Handle
    $verdict = 'pass'; $detail = ''; $loaded = $null; $timedOut = $false
    $deadline = $started.AddMinutes($Minutes)
    while (-not $p.WaitForExit(20000)) {
        if ((Get-Date) -gt $deadline) { $timedOut = $true; break }
        if ($null -eq $loaded -and $WatchModule) {
            try { if ($p.Modules | Where-Object ModuleName -eq $WatchModule) { $loaded = [int]((Get-Date) - $started).TotalSeconds } } catch { }
        }
    }
    if ($timedOut) {
        & taskkill.exe /T /F /PID $p.Id *> $null
        $verdict = 'timeout'
    } else {
        $code = $p.ExitCode
        $summary = Select-String -LiteralPath "$stem.out.txt" -Pattern '^test result:' | Select-Object -Last 1
        if ($code -eq 0) {
            $detail = if ($summary) { $summary.Line } else { '' }
            if ($detail -match ' 0 passed; 0 failed') { throw "no test matched the filters: $detail" }
        }
        elseif ($code -eq 101) { $verdict = 'fail'; $detail = if ($summary) { $summary.Line } else { 'exit 101' } }
        else {
            $verdict = 'crash'
            $hex = '0x{0:x8}' -f $code  # x8 of a negative Int32 prints its two's complement
            Start-Sleep -Milliseconds 1500  # WER writes event 1000 just after the process ends
            $pidHex = '0x{0:X}' -f $p.Id
            $event = Get-WinEvent -FilterHashtable @{ LogName = 'Application'; Id = 1000; StartTime = $started } -ErrorAction SilentlyContinue |
                Where-Object { $_.Message -match "Faulting process id: $pidHex\b" } | Select-Object -First 1
            $module = if ($event) { $m = $event.Message; '{0}+{1}' -f [regex]::Match($m, 'Faulting module name: ([^,]+)').Groups[1].Value, [regex]::Match($m, 'Fault offset: (\S+)').Groups[1].Value } else { 'no event 1000' }
            $detail = "$hex $module"
            $report = @(Select-String -LiteralPath "$stem.err.txt", "$stem.out.txt" -Pattern '^test_crash_report:|^  \S+\+0x' -ErrorAction SilentlyContinue | ForEach-Object Line)
            if ($report.Count) { $report | Set-Content -Encoding utf8 "$stem.crash.txt"; $detail += " | " + ($report | Where-Object { $_ -match 'access violation' } | Select-Object -Last 1) }
        }
    }
    $seconds = [int]((Get-Date) - $started).TotalSeconds
    $watch = if ($null -ne $loaded) { "$WatchModule loaded at ${loaded}s" } elseif ($verdict -eq 'crash' -and $detail -match [regex]::Escape($WatchModule)) { "$WatchModule loaded" } else { "$WatchModule not seen" }
    $results += [pscustomobject]@{ Run = $i; Verdict = $verdict; Seconds = $seconds; Detail = $detail; Loaded = ($watch -notmatch 'not seen') }
    Write-Host ('run {0,2}: {1,-7} {2,4} s [{3}] {4}' -f $i, $verdict, $seconds, $watch, $detail)
}
$count = { param($v) @($results | Where-Object Verdict -eq $v).Count }
$modules = @($results | Where-Object Verdict -eq 'crash' | ForEach-Object { ($_.Detail -split ' ')[1] } | Sort-Object -Unique) -join ', '
$mean = [int](($results | Measure-Object Seconds -Average).Average)
Write-Host ("test_crash_repro: runs={0} pass={1} crash={2} timeout={3} fail={4} mean={5}s modules=[{6}] watch={9}:{10}/{0} filters=[{7}] out={8}" -f $Runs, (& $count 'pass'), (& $count 'crash'), (& $count 'timeout'), (& $count 'fail'), $mean, $modules, ($Filter -join ' '), $OutDir, $WatchModule, @($results | Where-Object Loaded).Count)
