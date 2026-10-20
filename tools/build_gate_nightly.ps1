# Nightly long soak of the real app (docs/TOOLCHAIN.md "Build gate", part 4). Windows
# PowerShell 5.1. Overnight only: hours of grid-app runs, muted, the ported AI driving.
#   repro:  every private repro under local/repro/*/career-race.ini (the career's pauses,
#           e.g. lemans-manifold-40552), -ReproRepeats times, as the unattended career runs
#           them (50,000 scripted ticks).
#   events: every launcher profile at full length (its laps or time limit; the Le Mans
#           24 Hours' 720 s), one round per seed (GRID_SERIES_SEED = -SeedBase + round),
#           rounds until -Hours; free practice profiles stop after -PracticeMinutes.
#   soak:   then `grid-soak long` for -SoakHours (0 = skip).
# Fails like the gate's smoke runs (ERROR, "driving paused", audio failure, panic, early exit,
# memory cap, FPS floor); a race that ends with its results (exit 0) passes. Known problems:
# tools/build_gate/expected.tsv. Evidence: -Out (default local/build-gate/nightly-<time>).
param(
    [string]$Exe,
    [string]$Soak,
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$PrivateDir,
    [string]$ReproDir,
    [string]$Out,
    [double]$Hours = 8,
    [int]$Jobs = 2,
    [int]$ReproRepeats = 2,
    [int]$SeedBase = 1000,
    [double]$PracticeMinutes = 10,
    [double]$SoakHours = 0,
    [double]$MemoryCapMB = 4096,
    # Gross only: four gate windows share the GPU (full-3: a run at 13 FPS kept real time).
    [double]$FpsFloor = 10,
    [double]$RealtimeFloor = 56,
    [double]$MinFreeGB = 4,
    [string]$Only,
    [string]$Phases = 'repro,events,soak'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'build_gate\gate_lib.ps1')
$repo = $script:GateRepo
if (-not $Exe) { $Exe = Join-Path $repo 'target\release\grid-app.exe' }
if (-not $Soak) { $Soak = Join-Path $repo 'target\release\grid-soak.exe' }
$Exe = [IO.Path]::GetFullPath($Exe); $Soak = [IO.Path]::GetFullPath($Soak)
if (-not $Out) { $Out = Join-Path $repo ('local\build-gate\nightly-' + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
$Out = [IO.Path]::GetFullPath($Out)
if ($Out -notmatch '\\local\\') { throw 'Nightly evidence must stay under a local\ folder' }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$PrivateDir = Get-GatePrivateDir $PrivateDir
if (-not $ReproDir) { $ReproDir = Join-Path $PrivateDir 'repro' }
$expected = Read-GateExpected (Join-Path $PSScriptRoot 'build_gate\expected.tsv')
$started = Get-Date
$deadline = $started.AddHours($Hours)
$revision = Get-GateRevision
$exeHash = (Get-FileHash -LiteralPath $Exe -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "nightly_begin revision=$revision exe_sha256=$exeHash out=$Out jobs=$Jobs hours=$Hours load=$(Get-GateLoad)"
$results = New-Object System.Collections.Generic.List[object]
$tps = $script:GateTicksPerSecond
# The most scripted ticks grid-app accepts (19.2 min); the career's Le Mans script is 50,000.
$maxTicks = 72000
# A pile-up of broken props can hold the sim near 20 ticks/s for the rest of a race
# (nightly-repro-1: 21.7); the timeouts allow for that, so late pauses are still reached.
$slowest = 20

$specs = New-Object System.Collections.Generic.List[object]
if ($Phases -match 'repro' -and (Test-Path -LiteralPath $ReproDir)) {
    foreach ($dir in Get-ChildItem -LiteralPath $ReproDir -Directory | Sort-Object Name) {
        $ini = Join-Path $dir.FullName 'career-race.ini'
        if (-not (Test-Path -LiteralPath $ini)) { continue }
        if ($Only -and "repro-$($dir.Name)" -notmatch $Only) { continue }
        for ($k = 1; $k -le $ReproRepeats; $k++) {
            $specs.Add([pscustomobject]@{
                Name = "repro-$($dir.Name)-r$k"; Kind = 'repro'; Env = @{ GRID_AUTOPILOT = '1' }; Extra = ''; Lead = @()
                Args = @('--settings', $ini, '--game-root', $GameRoot, '--drive-ticks', '50000')
                Files = @(Get-ChildItem -LiteralPath $dir.FullName -Filter '*.tsv' | ForEach-Object { $_.FullName })
                TimeoutSec = [int](50000 / $slowest + 150); StopOnPause = $true; WarmupSec = 0; AllowExit = $true
            })
        }
    }
}
if ($Phases -match 'events') {
    $profiles = @(Get-ChildItem -LiteralPath (Join-Path $repo 'settings') -Filter '*.ini' | Sort-Object Name |
        Where-Object { -not $Only -or $_.BaseName -match $Only })
    # The longest event first in every round.
    $profiles = @($profiles | Where-Object { $_.BaseName -eq 'le-mans-24' }) + @($profiles | Where-Object { $_.BaseName -ne 'le-mans-24' })
    # Enough rounds for the night; the deadline stops new starts.
    $rounds = [math]::Max(1, [int][math]::Ceiling($Hours * 3600 / 1800))
    for ($round = 0; $round -lt $rounds; $round++) {
        $seed = $SeedBase + $round
        foreach ($ini in $profiles) {
            $text = Get-Content -LiteralPath $ini.FullName -Raw
            $open = $text -notmatch '(?m)^\s*laps\s*=\s*[1-9]' -and $text -notmatch '(?m)^\s*time_limit\s*='
            $ticks = $maxTicks
            if ($open) { $ticks = [int]($PracticeMinutes * 60 * $tps) }
            $specs.Add([pscustomobject]@{
                Name = "$($ini.BaseName)-s$seed"; Kind = 'event'; Env = @{ GRID_AUTOPILOT = '1'; GRID_SERIES_SEED = "$seed" }; Extra = ''; Lead = @()
                Args = @('--settings', $ini.FullName, '--game-root', $GameRoot, '--drive-ticks', "$ticks")
                Files = @(); TimeoutSec = [int]($ticks / $slowest + 150); StopOnPause = $true; WarmupSec = 0; AllowExit = $true
            })
        }
    }
}

$onFinish = {
    param($run)
    $reasons = Get-GateReasons -Run $run -MemoryCapMB $MemoryCapMB -FpsFloor $FpsFloor -RealtimeFloor $RealtimeFloor
    $v = Resolve-GateStatus -Name $run.Spec.Name -Reasons $reasons -Expected $expected
    $avg = Get-GateAverageFps $run
    $wall = ($run.Ended - $run.Started).TotalSeconds
    $end = $run.Stop
    if (-not $end) { $end = "exit $($run.ExitCode)" }
    if ($run.RaceResult) { $end += ' results' }
    $sim = Get-GateSimRate $run
    $detail = "peak={0:N0}MB fps={1} sim={4} ticks={2} props={5} end={3}" -f $run.PeakMB, $(if ($null -ne $avg) { '{0:N1}' -f $avg } else { '-' }), $run.LastTick, $end, $(if ($null -ne $sim) { '{0:N1}' -f $sim } else { '-' }), $run.PropBodies
    $results.Add([pscustomobject]@{ Name = $run.Spec.Name; Kind = $run.Spec.Kind; Status = $v.Status; Detail = $detail; Seconds = $wall
        Reasons = (($reasons | ForEach-Object { "$($_.Check): $($_.Text)" }) -join ' | '); References = $v.References })
    Write-Host ("nightly_result {0} {1} {2} {3:N0}s {4}" -f $run.Spec.Name, $v.Status, $detail, $wall, (($reasons | ForEach-Object { $_.Text }) -join ' | '))
    Clear-GateReplayCache $run $v.Status
    # Disk: only failing runs keep their full logs; passing ones keep the head and tail.
    if ($v.Status -eq 'pass') {
        foreach ($log in @('stdout.log', 'stderr.log')) {
            $p = Join-Path $run.Dir $log
            if ((Test-Path -LiteralPath $p) -and (Get-Item -LiteralPath $p).Length -gt 4MB) {
                $head = Get-Content -LiteralPath $p -TotalCount 200; $tail = Get-Content -LiteralPath $p -Tail 400
                Set-Content -LiteralPath $p -Value (@($head) + '... (trimmed by build_gate_nightly.ps1: the run passed) ...' + @($tail))
            }
        }
    }
    Write-NightlySummary
}

function Write-NightlySummary {
    $failed = @($results | Where-Object { $_.Status -eq 'fail' }).Count
    $xfail = @($results | Where-Object { $_.Status -eq 'xfail' }).Count
    $md = New-Object System.Collections.Generic.List[string]
    $md.Add("# Nightly soak $(Get-Date $started -Format 's')"); $md.Add('')
    $md.Add("Revision $revision, grid-app sha256 $exeHash, jobs $Jobs, $Hours h. Runs so far $($results.Count): $failed failed, $xfail expected failures; $([int]((Get-Date) - $started).TotalMinutes) min.")
    $md.Add(''); $md.Add('| run | kind | status | detail | seconds | reasons | reference |'); $md.Add('|---|---|---|---|---|---|---|')
    foreach ($r in $results) { $md.Add("| $($r.Name) | $($r.Kind) | $($r.Status) | $($r.Detail) | $([int]$r.Seconds) | $($r.Reasons -replace '\|', '/') | $($r.References) |") }
    $md | Set-Content -LiteralPath (Join-Path $Out 'summary.md') -Encoding UTF8
}

if ($specs.Count -gt 0) {
    $null = Invoke-GateQueue -Specs $specs -Exe $Exe -OutDir $Out -PrivateDir $PrivateDir -Jobs $Jobs -MinFreeGB $MinFreeGB -OnFinish $onFinish -Deadline $deadline
}
if ($Phases -match 'soak' -and $SoakHours -gt 0) {
    $envs = @{
        GRID_GAME_ROOT = $GameRoot
        GRID_CONTACT_DIRECTIONS = Join-Path $PrivateDir 'body-contact-directions.bin'
        GRID_BODY_EVENT_PARAMETERS = Join-Path $PrivateDir 'body-impact-parameters.bin'
        GRID_DEFORMATION_PARAMETERS = Join-Path $PrivateDir 'deformation-parameters.bin'
        GRID_AI_CHARACTERISTICS = Join-Path $PrivateDir 'ai-driver-characteristics.tsv'
    }
    foreach ($k in $envs.Keys) { [Environment]::SetEnvironmentVariable($k, $envs[$k], 'Process') }
    Push-Location $repo
    try { & $Soak long --hours "$SoakHours" --jobs 2 --out (Join-Path $Out 'soak-long') 2>&1 | Select-Object -Last 3 | ForEach-Object { Write-Host "$_" } }
    finally { Pop-Location }
    $code = $LASTEXITCODE
    $results.Add([pscustomobject]@{ Name = 'grid-soak-long'; Kind = 'soak'; Status = $(if ($code -eq 0) { 'pass' } else { 'fail' }); Detail = "summary $(Join-Path $Out 'soak-long\summary.md')"; Seconds = 0; Reasons = ''; References = '' })
}
Write-NightlySummary
$unseen = @($expected | Where-Object { -not $_.Seen -and ($results.Name -match "^(?:$($_.Run))$") })
foreach ($e in $unseen) { Write-Host "nightly_expected_not_seen $($e.Run) $($e.Check) $($e.Reference)" }
$failed = @($results | Where-Object { $_.Status -eq 'fail' }).Count
Write-Host ("nightly_end runs={0} failed={1} minutes={2:N0} summary={3}" -f $results.Count, $failed, ((Get-Date) - $started).TotalMinutes, (Join-Path $Out 'summary.md'))
if ($failed -gt 0) { exit 1 }
exit 0
