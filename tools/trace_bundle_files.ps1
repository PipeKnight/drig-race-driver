# Lists the GRID files grid-app reads for one bundled Android event (F608,
# docs/ANDROID.md "Bundle file list"): a desktop run of the event menu and the
# race, set up as the phone runs it (the bundle's settings.ini and event.ini in
# a scratch data dir, the menu's Continue by GRID_MENU_AUTO, autopilot, muted).
#
#   powershell -NoProfile -File tools\trace_bundle_files.ps1 -Event drift-nurburgring
#       on a scratch mirror of the installation: writes android/events/<event>.files,
#       every file the run read (names only; push_android_data.ps1 bundles them)
#   powershell -NoProfile -File tools\trace_bundle_files.ps1 -Event drift-nurburgring -Stage DIR
#       on a push_android_data.ps1 -Stage folder: lists every file the app read
#       outside the bundle's lists, every file error in its log and every file the
#       event's list names that this run did not read; exit code 1 when there is
#       one. The stage is case-sensitive like Android storage (F921), so a name
#       the code derives in another case than the shipped file is caught even
#       where the code skips a missing optional file silently (the Viper's
#       int_VIP_anims.pssg). Any menu event can be checked on the same stage.
#
# How reads are seen without elevation (Procmon needs it): NTFS here updates a
# file's last-access time on a read when the stored one is more than an hour
# old (DisableLastAccess = 2, system managed, enabled), and not on a metadata
# query. The run's game root gets every last-access time set to 2000-01-01
# first; the files whose time moved during the run were read. The original
# installation is never touched: the mirror (robocopy, about 10 GiB) lives in
# the ignored local/android/trace/mirror and is deleted unless -KeepMirror.
# A window opens for the run; target/release/grid-app.exe must be built.
param(
    [ValidateSet('donington', 'drift-nurburgring', 'washington')]
    [string]$Event = 'donington',
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$Stage = '',
    # Race ticks after the menu's Continue (16 ms each, real time).
    [int]$Ticks = 3000,
    [string]$Exe = (Join-Path (Split-Path $PSScriptRoot) 'target\release\grid-app.exe'),
    [string]$Characteristics = (Join-Path (Split-Path $PSScriptRoot) 'local\ai-driver-characteristics.tsv'),
    [switch]$KeepMirror
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
if (-not (Test-Path -LiteralPath $Exe)) { throw "$Exe missing; build it with cargo build --release -p grid-app" }
$traceDir = Join-Path $root 'local\android\trace'
New-Item -ItemType Directory -Force -Path $traceDir | Out-Null
& git -C $root check-ignore -q $traceDir
if ($LASTEXITCODE -ne 0) { throw "$traceDir is not ignored by git" }
$work = Join-Path $traceDir ("$Event-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
$data = Join-Path $work 'data'
New-Item -ItemType Directory -Force -Path $data | Out-Null

# The game root of the run: the stage's Grid folder, or a mirror of the installation.
if ($Stage) {
    $gridRoot = [IO.Path]::GetFullPath((Join-Path $Stage 'Grid'))
} else {
    $gridRoot = Join-Path $traceDir 'mirror'
    & robocopy $GameRoot $gridRoot /MIR /NFL /NDL /NJH /NJS /NP /R:0 /W:0 | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy of the installation failed ($LASTEXITCODE)" }
}
$old = [datetime]'2000-01-01'
$files = Get-ChildItem -LiteralPath $gridRoot -Recurse -File -Force
foreach ($f in $files) { $f.LastAccessTime = $old }

# The phone's data dir: the bundle's drive profile, the event's own profile
# (a stage's event.ini is only its bundle's starting event, and every menu
# offers Donington and Washington), the game root.
Copy-Item -LiteralPath (Join-Path $root "android\events\$Event.ini") -Destination (Join-Path $data 'event.ini')
if ($Stage) {
    $settings = [IO.File]::ReadAllText((Join-Path $Stage 'settings.ini'))
    $tsv = Join-Path $Stage 'ai-driver-characteristics.tsv'
} else {
    $settings = [IO.File]::ReadAllText((Join-Path $root 'android\app\src\main\assets\grid-import\settings.ini'))
    $tsv = $Characteristics
}
if ($Event -eq 'drift-nurburgring' -and (Test-Path -LiteralPath $tsv)) { Copy-Item -LiteralPath $tsv -Destination $data }
$settings = ($settings -replace "`r`n", "`n").TrimEnd() + "`n[game]`nroot = $($gridRoot -replace '\\', '/')`n"
[IO.File]::WriteAllText((Join-Path $data 'settings.ini'), $settings)

$log = Join-Path $work 'grid-app.log'
$started = Get-Date
$env:GRID_MENU_AUTO = "$Ticks"
$env:GRID_AUTOPILOT = '1'
$exePath = (Resolve-Path -LiteralPath $Exe).Path
try {
    $app = Start-Process -FilePath $exePath -ArgumentList "--menu --data-dir `"$data`"" -WorkingDirectory $root `
        -RedirectStandardOutput $log -RedirectStandardError "$log.err" -PassThru
    $mute = Join-Path (Split-Path (& git -C $root rev-parse --path-format=absolute --git-common-dir)) 'local\mute_grid.ps1'
    # The scripted race ends at its tick count, but its window stays open.
    $limit = $started.AddSeconds(90 + $Ticks * 0.016 * 2)
    while (-not $app.HasExited -and (Get-Date) -lt $limit) {
        if (Test-Path -LiteralPath $mute) { & powershell -NoProfile -ExecutionPolicy Bypass -File $mute *> $null }
        if (Select-String -LiteralPath $log -Pattern 'drive_script_complete' -Quiet) { Start-Sleep -Seconds 3; break }
        Start-Sleep -Seconds 2
    }
} finally {
    Remove-Item Env:GRID_MENU_AUTO, Env:GRID_AUTOPILOT -ErrorAction SilentlyContinue
    # Only this executable's processes (never another checkout's game).
    Get-Process grid-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exePath } | Stop-Process -Force
}
$seconds = [int]((Get-Date) - $started).TotalSeconds
$prefix = $gridRoot.TrimEnd('\').Length + 1
$read = @(Get-ChildItem -LiteralPath $gridRoot -Recurse -File -Force |
    Where-Object { $_.LastAccessTime -gt $started.AddMinutes(-1) } |
    ForEach-Object { $_.FullName.Substring($prefix).Replace('\', '/') } |
    Sort-Object { $_.ToLowerInvariant() })
$errors = @(Get-Content -LiteralPath $log, "$log.err" -ErrorAction SilentlyContinue |
    Where-Object { $_ -match 'No such file|cannot find the (file|path)|os error [23]\b' })
$steps = @(Select-String -LiteralPath $log -Pattern 'drive_event_menu version=1 event=go|drive_script_complete' | ForEach-Object { $_.Line.Substring(0, [Math]::Min(140, $_.Line.Length)) })
Write-Output "trace_bundle_files event=$Event root=$gridRoot ticks=$Ticks seconds=$seconds read=$($read.Count) file_errors=$($errors.Count)"
$steps | ForEach-Object { Write-Output "  $_" }
$errors | Select-Object -Unique | ForEach-Object { Write-Output "  file_error $($_.Substring(0, [Math]::Min(220, $_.Length)))" }
[IO.File]::WriteAllText((Join-Path $work 'read.txt'), (($read | ForEach-Object { "$_`n" }) -join ''))
if (-not ($steps -match 'drive_script_complete')) { throw "the race did not complete; see $log" }

if ($Stage) {
    # Every file read must be one the lists name (so a later build keeps it).
    $listed = @{}
    foreach ($e in @('donington', 'washington', $Event) | Select-Object -Unique) {
        Get-Content -LiteralPath (Join-Path $root "android\events\$e.files") |
            Where-Object { $_ -and -not $_.StartsWith('#') } | ForEach-Object { $listed[$_.ToLowerInvariant()] = $true }
    }
    $unlisted = @($read | Where-Object { -not $listed.ContainsKey($_.ToLowerInvariant()) })
    $unlisted | ForEach-Object { Write-Output "  unlisted $_" }
    # Every file the event's own desktop trace read must be read here too: a
    # file the stage has but the app did not open was looked up under another
    # name (F921). The speech lines after the marker are only played on cue.
    $readKeys = @{}
    $read | ForEach-Object { $readKeys[$_.ToLowerInvariant()] = $true }
    $own = Get-Content -LiteralPath (Join-Path $root "android\events\$Event.files")
    $marker = [Array]::FindIndex([string[]]$own, [Predicate[string]] { param($l) $l.StartsWith('# Speech lines') })
    if ($marker -ge 0) { $own = $own[0..($marker - 1)] }
    $unread = @($own | Where-Object { $_ -and -not $_.StartsWith('#') -and -not $readKeys.ContainsKey($_.ToLowerInvariant()) })
    $unread | ForEach-Object { Write-Output "  unread $_" }
    Write-Output "trace_bundle_files stage=$Stage event=$Event unlisted=$($unlisted.Count) unread=$($unread.Count) file_errors=$($errors.Count) (log: $log)"
    if ($unlisted.Count -or $unread.Count -or $errors.Count) { exit 1 }
    return
}
# Speech streams one file per spoken line, so a run reads only the lines it
# played: every line the ported events can play is listed as well, after the
# marker, even one this run played. The stage check requires every file above
# the marker, and a played line depends on the run's events (a crash cue, F951).
$speech = @(& $exePath --speech-files --data-dir $data | Where-Object { $_ -like 'audio/*' })
if ($LASTEXITCODE -ne 0 -or -not $speech.Count) { throw 'grid-app --speech-files failed' }
$speechKeys = @{}
$speech | ForEach-Object { $speechKeys[$_.ToLowerInvariant()] = $true }
$read = @($read | Where-Object { -not $speechKeys.ContainsKey($_.ToLowerInvariant()) })
if (-not $KeepMirror) { Remove-Item -LiteralPath $gridRoot -Recurse -Force }
$list = Join-Path $root "android\events\$Event.files"
$header = "# Files grid-app read for the $Event bundle in a desktop run of the event menu and a`n" +
    "# $Ticks-tick race (tools/trace_bundle_files.ps1, F608). push_android_data.ps1 bundles them`n" +
    "# and fails when one is missing. Regenerate after a change that reads new game files.`n" +
    "# Paths relative to the GRID folder; names only.`n"
$text = $header + (($read | ForEach-Object { "$_`n" }) -join '') +
    "# Speech lines the ported events can play (grid-app --speech-files), played only on cue:`n" +
    (($speech | ForEach-Object { "$_`n" }) -join '')
[IO.File]::WriteAllText($list, $text)
Write-Output "wrote $list ($($read.Count) files read, $($speech.Count) speech lines)"
