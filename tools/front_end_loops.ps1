# One-window front end smoke (docs/FRONT_END_FLOW.md "Port state machine"): grid-app --menu
# in one process, Menu -> Loading -> Race -> Loading -> Menu, -Races times, with the menu's
# Continue pressed by GRID_MENU_AUTO and the ported AI driving each race for -Ticks ticks.
# The host samples private bytes, handles, threads and its own entity count at every menu
# and prints a verdict (GRID_MENU_LOOPS, crates/grid-app/src/front_end_host.rs).
# Muted by its data dir ([sound] effects = 0). Prints one summary line; the full log is in
# -Out. Exit code 0 when the host's verdict passed. Windows PowerShell 5.1.
param(
    [string]$Exe,
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [int]$Races = 8,
    [int]$Ticks = 300,
    [string]$Out
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
if (-not $Exe) { $Exe = Join-Path $root 'target\release\grid-app.exe' }
if (-not $Out) { $Out = Join-Path $root ("local\front-end-flow\loops-" + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
$data = Join-Path $Out 'data'
New-Item -ItemType Directory -Force $data | Out-Null
$settings = "[sound]`neffects = 0`n[front_end]`nshow = off`n[game]`nroot = $($GameRoot -replace '\\', '/')`n"
[IO.File]::WriteAllText((Join-Path $data 'settings.ini'), $settings)
$log = Join-Path $Out 'grid-app.log'
$exePath = (Resolve-Path -LiteralPath $Exe).Path
$env:GRID_MENU_AUTO = "$Ticks"
$env:GRID_AUTOPILOT = '1'
$env:GRID_MENU_LOOPS = "$Races"
$started = Get-Date
try {
    $app = Start-Process -FilePath $exePath -ArgumentList "--menu --data-dir `"$data`"" -WorkingDirectory $root `
        -RedirectStandardOutput $log -RedirectStandardError "$log.err" -PassThru
    # Keep the handle, or ExitCode reads empty after the exit.
    $null = $app.Handle
    # Load, 3 s menu and the race per loop, generously.
    $limit = 120 + $Races * (90 + $Ticks * 0.05)
    if (-not $app.WaitForExit([int]($limit * 1000))) {
        Stop-Process -Id $app.Id -Force
        $timedOut = $true
    }
} finally {
    Remove-Item Env:GRID_MENU_AUTO, Env:GRID_AUTOPILOT, Env:GRID_MENU_LOOPS -ErrorAction SilentlyContinue
}
$seconds = [int]((Get-Date) - $started).TotalSeconds
$lines = Get-Content -LiteralPath $log
$verdict = $lines | Where-Object { $_ -match '^front_end_loops version=1 races=' } | Select-Object -Last 1
$attach = @($lines | Where-Object { $_ -match 'front_end version=1 event=attach' } | ForEach-Object { ($_ -split 'screen=')[1] }) -join ','
$built = @($lines | Where-Object { $_ -match 'front_end version=1 event=built' } | ForEach-Object { if ($_ -match 'screen=(\w+) ok=(\w+) seconds=([\d.]+)') { "$($Matches[1]):$($Matches[3])" } }) -join ','
$errors = @(Get-Content -LiteralPath "$log.err" | Where-Object { $_ -match 'panicked|ERROR|grid-app:' }).Count
$code = if ($timedOut) { 'timeout' } else { $app.ExitCode }
$pass = (-not $timedOut) -and $app.ExitCode -eq 0 -and $verdict -match 'verdict=pass' -and $errors -eq 0
Write-Host "front_end_loops_smoke exit=$code seconds=$seconds error_lines=$errors attach=[$attach] built=[$built]"
Write-Host "  $verdict"
Write-Host ("front_end_loops_smoke verdict={0} log={1}" -f $(if ($pass) { 'pass' } else { 'fail' }), $log)
if ($pass) { exit 0 } else { exit 1 }
