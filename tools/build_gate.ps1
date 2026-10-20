# Build gate (docs/TOOLCHAIN.md "Build gate"): end-to-end checks of the real app before a
# rebuild of the PC playable, the portable copy or the APKs. Windows PowerShell 5.1.
#   1. perf:   grid-soak perf, one full-field quick scenario per track, CPU time per tick
#              (one worker thread) and wall p99 against tools/build_gate/perf_budget.tsv
#              (runs first and alone).
#   2. memory: a 10-minute 12-Lacetti race (tools/build_gate/memory-race.ini); fails when private bytes grow faster than
#              -SlopeLimit after a 2-minute warm-up (F482's audio leak grew 5-8 MB/s).
#   3. smoke:  every launcher profile (settings/*.ini, the Play GRID*.cmd races; Codex's
#              Graphics launchers excluded) and the event menu's default event, 60 s each.
#              Fails on an ERROR line, "driving paused", an audio source failure, a panic, an
#              early exit, peak private bytes over -MemoryCapMB or average FPS under -FpsFloor.
# Every run is muted (Windows mixer and [sound] effects = 0) and driven by the ported AI
# (diagnostics autopilot). Known problems are listed in tools/build_gate/expected.tsv and
# reported as xfail. Evidence: -Out (default local/build-gate/gate-<time>). Exit code 1 when
# anything unexpected failed. Never runs the original grid.exe.
param(
    [string]$Exe,
    [string]$Soak,
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$PrivateDir,
    [string]$Out,
    [int]$Jobs = 4,
    [int]$DriveSeconds = 60,
    [double]$MemoryMinutes = 10,
    # A settings profile: a path, or a name in settings\ (default tools\build_gate\memory-race.ini).
    [string]$MemoryProfile,
    [double]$SlopeLimit = 1.0,
    [double]$MemoryCapMB = 4096,
    # Gross only: four gate windows share the GPU (full-3: a run at 13 FPS kept real time).
    [double]$FpsFloor = 10,
    [double]$RealtimeFloor = 56,
    [double]$MinFreeGB = 4,
    [int]$PerfTicks = 3000,
    # P-core threads of the i7-12700 (an E-core costs twice the CPU time per tick).
    [string]$PerfAffinity = 'FFFF',
    # Perf waits this long for other sessions' compilers to finish (perf-8: a cargo build beside it
    # put 6 of 13 tracks 7-50% over).
    [double]$PerfWaitMinutes = 15,
    [string]$Only,
    [switch]$SkipPerf,
    [switch]$SkipMemory,
    [switch]$SkipSmoke,
    [switch]$RecordBudget
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'build_gate\gate_lib.ps1')
$repo = $script:GateRepo
if (-not $Exe) { $Exe = Join-Path $repo 'target\release\grid-app.exe' }
if (-not $Soak) { $Soak = Join-Path $repo 'target\release\grid-soak.exe' }
$Exe = [IO.Path]::GetFullPath($Exe); $Soak = [IO.Path]::GetFullPath($Soak)
if (-not $Out) { $Out = Join-Path $repo ('local\build-gate\gate-' + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
$Out = [IO.Path]::GetFullPath($Out)
if ($Out -notmatch '\\local\\') { throw 'Gate evidence must stay under a local\ folder' }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$PrivateDir = Get-GatePrivateDir $PrivateDir
$expected = Read-GateExpected (Join-Path $PSScriptRoot 'build_gate\expected.tsv')
$budgetFile = Join-Path $PSScriptRoot 'build_gate\perf_budget.tsv'
$started = Get-Date
$revision = Get-GateRevision
$exeHash = (Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash.ToLowerInvariant()
$load = Get-GateLoad
Write-Host "gate_begin revision=$revision exe_sha256=$exeHash out=$Out jobs=$Jobs load=$load"
$results = New-Object System.Collections.Generic.List[object]

# ---- 1. perf ----------------------------------------------------------------------------
if (-not $SkipPerf) {
    $phase = Get-Date
    $waitUntil = (Get-Date).AddMinutes($PerfWaitMinutes)
    while ((Get-GateCompilers) -and (Get-Date) -lt $waitUntil) {
        Write-Host "gate_perf_wait compilers=$(Get-GateCompilers)"; Start-Sleep -Seconds 30
    }
    $compilers = Get-GateCompilers
    $perfOut = Join-Path $Out 'perf'
    $saved = @{}
    $envs = @{
        GRID_GAME_ROOT = $GameRoot
        GRID_CONTACT_DIRECTIONS = Join-Path $PrivateDir 'body-contact-directions.bin'
        GRID_BODY_EVENT_PARAMETERS = Join-Path $PrivateDir 'body-impact-parameters.bin'
        GRID_DEFORMATION_PARAMETERS = Join-Path $PrivateDir 'deformation-parameters.bin'
        GRID_AI_CHARACTERISTICS = Join-Path $PrivateDir 'ai-driver-characteristics.tsv'
    }
    foreach ($k in $envs.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k, 'Process'); [Environment]::SetEnvironmentVariable($k, $envs[$k], 'Process') }
    try {
        $perfArgs = @('perf', '--out', $perfOut, '--ticks', "$PerfTicks", '--game-root', $GameRoot)
        if ($PerfAffinity) { $perfArgs += @('--affinity', $PerfAffinity) }
        if ($RecordBudget) { $perfArgs += @('--write-budget', (Join-Path $perfOut 'perf_budget.tsv')) }
        elseif (Test-Path -LiteralPath $budgetFile) { $perfArgs += @('--budget', $budgetFile) }
        Push-Location $repo
        try { $lines = @(& $Soak @perfArgs 2>&1 | ForEach-Object { "$_" }) } finally { Pop-Location }
    } finally {
        foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k], 'Process') }
    }
    $lines | Set-Content -LiteralPath (Join-Path $Out 'perf.log')
    # A compiler at the start or end of perf (or started in between and still running) makes a case
    # over budget "noisy": listed, not failed (perf-8, perf-9: compilers beside perf moved single
    # cases 0.6-2.3x either way). A failed run is still a failure.
    $during = @($compilers, (Get-GateCompilers)) | Where-Object { $_ } | Select-Object -First 1
    foreach ($line in $lines) {
        if ($line -notmatch '^soak_perf_case ') { continue }
        $f = @{}
        foreach ($m in [regex]::Matches(($line -replace ' note=.*$', ''), '(\w+)=(\S+)')) { $f[$m.Groups[1].Value] = $m.Groups[2].Value }
        $note = ''
        if ($line -match ' note=(.*)$') { $note = $Matches[1] }
        $reasons = New-Object System.Collections.Generic.List[object]
        if ($f.status -eq 'over') { $reasons.Add([pscustomobject]@{ Check = 'perf'; Text = $(if ($during) { "$note; compilers ran during perf ($during): re-run tools\build_gate.ps1 -SkipMemory -SkipSmoke when no build runs" } else { $note }) }) }
        elseif ($f.status -eq 'changed') { Write-Host "gate_perf_rerecord $($f.track): $note" }
        elseif ($f.status -ne 'pass') { $reasons.Add([pscustomobject]@{ Check = 'perf'; Text = "run failed: $note" }) }
        $name = "perf-$($f.track)"
        $v = Resolve-GateStatus -Name $name -Reasons $reasons -Expected $expected
        if ($v.Status -eq 'fail' -and $f.status -eq 'over' -and $during) { $v.Status = 'noisy' }
        $detail = "cpu={0}ms/tick wall_mean={1}ms p99={2}ms budget={3}/{4}ms seed={5} cars={6} soak={7} bodies={8} perf={9}" -f $f.cpu_ms, $f.mean_ms, $f.p99_ms, $f.budget_cpu_ms, $f.budget_p99_ms, $f.seed, $f.cars, $f.soak, $f.max_bodies, $f.status
        $results.Add([pscustomobject]@{ Name = $name; Kind = 'perf'; Status = $v.Status; Detail = $detail; Seconds = [double]$f.seconds
            Reasons = (($reasons | ForEach-Object { "$($_.Check): $($_.Text)" }) -join ' | '); References = $v.References })
        Write-Host ("gate_result {0} {1} {2} {3:N0}s {4}" -f $name, $v.Status, $detail, [double]$f.seconds, (($reasons | ForEach-Object { $_.Text }) -join ' | '))
    }
    if (-not ($lines -match '^soak_perf_summary ')) {
        $results.Add([pscustomobject]@{ Name = 'perf'; Kind = 'perf'; Status = 'fail'; Detail = 'grid-soak perf gave no summary'; Seconds = 0
            Reasons = (($lines | Select-Object -Last 3) -join ' | '); References = '' })
        Write-Host "gate_result perf fail grid-soak perf gave no summary"
    }
    Write-Host ("gate_phase perf seconds={0:N0}" -f ((Get-Date) - $phase).TotalSeconds)
}

# ---- 2 and 3. memory and smoke runs in grid-app -------------------------------------------
$specs = New-Object System.Collections.Generic.List[object]
$ticks = [int]($DriveSeconds * $script:GateTicksPerSecond)
$load_allowance = 150
if (-not $SkipMemory) {
    # Stopped after -MemoryMinutes of driving (wall clock), so a slow race still gives its slope.
    $memSeconds = [int]($MemoryMinutes * 60)
    $memTicks = 50000
    $memIni = Join-Path $PSScriptRoot 'build_gate\memory-race.ini'
    if ($MemoryProfile -and (Test-Path -LiteralPath $MemoryProfile)) { $memIni = [IO.Path]::GetFullPath($MemoryProfile) }
    elseif ($MemoryProfile) { $memIni = Join-Path $repo "settings\$MemoryProfile.ini" }
    $specs.Add([pscustomobject]@{
        Name = ("memory-" + ([IO.Path]::GetFileNameWithoutExtension($memIni) -replace '^memory-', '')); Kind = 'memory'; Env = @{ GRID_AUTOPILOT = '1' }; Extra = ''
        Args = @('--settings', $memIni, '--game-root', $GameRoot, '--drive-ticks', "$memTicks")
        TimeoutSec = $memSeconds + 2 * $load_allowance; StopAfterDriveSec = $memSeconds; StopOnPause = $false; WarmupSec = 120; AllowExit = $true
    })
}
if (-not $SkipSmoke) {
    foreach ($ini in Get-ChildItem -LiteralPath (Join-Path $repo 'settings') -Filter '*.ini' | Sort-Object Name) {
        $name = $ini.BaseName
        if ($Only -and $name -notmatch $Only) { continue }
        $specs.Add([pscustomobject]@{
            Name = $name; Kind = 'smoke'; Env = @{ GRID_AUTOPILOT = '1' }; Extra = ''
            Args = @('--settings', $ini.FullName, '--game-root', $GameRoot, '--drive-ticks', "$ticks")
            TimeoutSec = $DriveSeconds * 2 + $load_allowance; StopOnPause = $true; WarmupSec = 0; AllowExit = $false
        })
    }
    if (-not $Only -or 'menu-default' -match $Only) {
        # Play GRID.cmd: the event menu's default event, Continue pressed by GRID_MENU_AUTO.
        $specs.Add([pscustomobject]@{
            Name = 'menu-default'; Kind = 'smoke'; Env = @{ GRID_AUTOPILOT = '1'; GRID_MENU_AUTO = "$ticks" }; Extra = ''
            Lead = @('--menu'); Args = @('--game-root', $GameRoot)
            TimeoutSec = $DriveSeconds * 2 + $load_allowance + 30; StopOnPause = $true; WarmupSec = 0; AllowExit = $false
        })
    }
}
if ($specs.Count -gt 0) {
    $phase = Get-Date
    $onFinish = {
        param($run)
        $limit = 0
        if ($run.Spec.Kind -eq 'memory') { $limit = $SlopeLimit }
        $reasons = Get-GateReasons -Run $run -MemoryCapMB $MemoryCapMB -FpsFloor $FpsFloor -SlopeLimit $limit -RealtimeFloor $RealtimeFloor
        $v = Resolve-GateStatus -Name $run.Spec.Name -Reasons $reasons -Expected $expected
        $avg = Get-GateAverageFps $run
        $minFps = $null
        if ($run.Fps.Count -gt 0) { $minFps = ($run.Fps | Measure-Object -Minimum).Minimum }
        $wall = ($run.Ended - $run.Started).TotalSeconds
        $drive = 0
        if ($run.DriveStarted) { $drive = ($run.Ended - $run.DriveStarted).TotalSeconds }
        $sim = Get-GateSimRate $run
        $detail = "peak={0:N0}MB fps={1}/min {2} sim={6} drive={3:N0}s ticks={4} props={7} stop={5}" -f $run.PeakMB, $(if ($null -ne $avg) { '{0:N1}' -f $avg } else { '-' }), $(if ($null -ne $minFps) { '{0:N1}' -f $minFps } else { '-' }), $drive, $run.LastTick, $(if ($run.Stop) { $run.Stop } else { "exit $($run.ExitCode)" }), $(if ($null -ne $sim) { '{0:N1}' -f $sim } else { '-' }), $run.PropBodies
        if ($run.Spec.Kind -eq 'memory') {
            $s = Get-GateSlope -Run $run -WarmupSec $run.Spec.WarmupSec
            if ($s) { $detail += (" slope={0:N3}MB/s over {1:N0}s ({2:N0}->{3:N0}MB)" -f $s.Slope, $s.Seconds, $s.FirstMB, $s.LastMB) }
            $run.Samples | ForEach-Object { "{0:N1}`t{1:N1}`t{2}" -f $_.T, $_.MB, $_.Drive } | Set-Content -LiteralPath (Join-Path $run.Dir 'memory.tsv')
        }
        $results.Add([pscustomobject]@{ Name = $run.Spec.Name; Kind = $run.Spec.Kind; Status = $v.Status; Detail = $detail; Seconds = $wall
            Reasons = (($reasons | ForEach-Object { "$($_.Check): $($_.Text)" }) -join ' | '); References = $v.References })
        Write-Host ("gate_result {0} {1} {2} {3:N0}s {4}" -f $run.Spec.Name, $v.Status, $detail, $wall, (($reasons | ForEach-Object { $_.Text }) -join ' | '))
        Clear-GateReplayCache $run $v.Status
    }
    $null = Invoke-GateQueue -Specs $specs -Exe $Exe -OutDir $Out -PrivateDir $PrivateDir -Jobs $Jobs -MinFreeGB $MinFreeGB -OnFinish $onFinish
    Write-Host ("gate_phase app runs={0} seconds={1:N0}" -f $specs.Count, ((Get-Date) - $phase).TotalSeconds)
}

# ---- Summary ------------------------------------------------------------------------------
$total = ((Get-Date) - $started).TotalSeconds
$failed = @($results | Where-Object { $_.Status -eq 'fail' }).Count
$xfail = @($results | Where-Object { $_.Status -eq 'xfail' }).Count
$noisy = @($results | Where-Object { $_.Status -eq 'noisy' }).Count
$md = New-Object System.Collections.Generic.List[string]
$md.Add("# Build gate $(Get-Date -Format 's')")
$md.Add('')
$md.Add("Revision $revision, grid-app sha256 $exeHash, jobs $Jobs, load at start: $load. Total $([int]$total) s.")
$md.Add("Results: $($results.Count) checks, $failed failed, $xfail expected failures, $noisy perf cases over budget while compilers ran (not verified).")
$md.Add('')
$md.Add('| check | kind | status | detail | seconds | reasons | reference |')
$md.Add('|---|---|---|---|---|---|---|')
foreach ($r in $results) { $md.Add("| $($r.Name) | $($r.Kind) | $($r.Status) | $($r.Detail) | $([int]$r.Seconds) | $($r.Reasons -replace '\|', '/') | $($r.References) |") }
$unseen = @($expected | Where-Object { -not $_.Seen -and ($results.Name -match "^(?:$($_.Run))$") })
if ($unseen.Count -gt 0) {
    $md.Add(''); $md.Add('Expected failures not seen this time (fixed, or intermittent):')
    foreach ($e in $unseen) { $md.Add("- $($e.Run) $($e.Check) ``$($e.Pattern)``: $($e.Reference)") }
}
$md | Set-Content -LiteralPath (Join-Path $Out 'summary.md') -Encoding UTF8
$results | ForEach-Object { "{0}`t{1}`t{2}`t{3}`t{4:N0}`t{5}`t{6}" -f $_.Name, $_.Kind, $_.Status, $_.Detail, $_.Seconds, $_.Reasons, $_.References } |
    Set-Content -LiteralPath (Join-Path $Out 'summary.tsv') -Encoding UTF8
foreach ($e in $unseen) { Write-Host "gate_expected_not_seen $($e.Run) $($e.Check) $($e.Reference)" }
Write-Host ("gate_end checks={0} failed={1} xfail={2} noisy={5} seconds={3:N0} summary={4}" -f $results.Count, $failed, $xfail, $total, (Join-Path $Out 'summary.md'), $noisy)
if ($failed -gt 0) { exit 1 }
exit 0
