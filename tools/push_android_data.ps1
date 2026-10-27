# Copies the owner's own GRID data for a solo Donington drive to the phone
# (P9, docs/ANDROID.md). Development path only; the folder picker is P10.
#
#   powershell -NoProfile -File tools\push_android_data.ps1            # adb push
#   powershell -NoProfile -File tools\push_android_data.ps1 -Stage DIR # copy to DIR (desktop check, or MTP copy)
#
# Destination on the phone: /sdcard/Android/data/org.reassembly.grid/files,
# the app-specific storage that settings::data_dir() returns there:
#   files/Grid/...      the subset of the installation listed below
#   files/settings.ini  the drive profile (written by this script)
# CD01/BP01/DF01 are derived in memory from the pushed grid.exe (F359, P5), so
# no private file is needed. Nothing here is ever put into the repository; only
# the private bundled APK (build_android.ps1 -BundleData) packs the -Stage
# output. Uninstalling the app deletes the folder.
param(
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$Stage = '',
    [string]$Adb = 'C:\Users\debros\Tools\android-sdk\platform-tools\adb.exe',
    [string]$Package = 'org.reassembly.grid',
    # Without effect since P11: the profile has no autopilot any more (the
    # phone controls drive); kept because build_android.ps1 -BundleData passes it.
    [switch]$NoAutopilot,
    # Event profile (android/events/<Event>.ini, written as event.ini): donington
    # (solo Lacetti practice), drift-nurburgring (Drift Battle, 7 AI BMWs) or
    # washington (the menu starts on the Viper race, F615).
    [ValidateSet('donington', 'drift-nurburgring', 'washington')]
    [string]$Event = 'donington',
    # The private AI characteristics table for drift-nurburgring (data-dir relative
    # in the profile); never put into the repository.
    [string]$Characteristics = (Join-Path $PSScriptRoot '..\local\ai-driver-characteristics.tsv')
)
$ErrorActionPreference = 'Stop'

# What a solo Lacetti drive at Donington reads (checked with -Stage, docs/ANDROID.md).
$items = @(
    'grid.exe',              # CD01/BP01/DF01 are derived from it in memory
    'ActionMap',
    'ai',
    'language\language_eng.lng',
    'car_materials.xml',
    'raceload.jpk',
    'surface_materials.xml',
    'system',
    'effects',
    'skidmarks',
    'audio\CMSamples.dic',
    'audio\carsetup.bin',
    'audio\cars',
    'audio\eng_lac.WIP',
    'audio\car_surfaces.WIP',
    'audio\audioVarieties.bin',    # tyre audio (missing before F376)
    'audio\CollisionInfos.bin',
    'audio\audioMaterials.bin',
    'track_sponsors\tag_heuer\sponsor_minor.pssg',
    'track_sponsors\bridgestone\sponsor_event.pssg',
    'cars\lac',
    'Osd',                   # the original HUD scene (docs/RACE_HUD.md section 6)
    'frontend\fonts\microgramma_osd.PSSG',
    'frontend\fonts\swiss_bold_osd_2d.PSSG',
    'frontend\fonts\micro_drift.pssg',
    'frontend\fonts\micro_lap.pssg',
    'frontend\fonts\swiss_time.pssg',
    'tracks\europe\donington',
    # The track's floor lift (Route::additional_ground_clearance), read when
    # body geometry is prepared; without it every race stopped there (F539).
    'database\schema.bin',
    'database\database.bin',
    # The Viper race at Washington route 0 (event menu `race_viper`, F615): the
    # car, its engine banks, the track's own files and route 0 (not routes 1-9).
    'cars\vip',
    'audio\eng_vip.WIP',
    'audio\mec_vip.WIP',
    'tracks\usa\washington\route_0'
)
$items += @(Get-ChildItem -LiteralPath (Join-Path $GameRoot 'tracks\usa\washington') -File |
    ForEach-Object { "tracks\usa\washington\$($_.Name)" })
# Extra data per event (checked with -Stage and a desktop run, docs/ANDROID.md "Events").
if ($Event -eq 'drift-nurburgring') {
    $items += @(
        'database\dlc-database-a0.bin',  # with the base list's two files: the whole
        'database\dlc-database-p0.bin',  # folder as before F539, so the bundle version stays
        'gameconfig',            # drift.xml (drift scoring settings)
        'cars\bmw',
        'audio\eng_bmw.WIP',
        'audio\mec_bmw.WIP',
        'tracks\europe\nurburgring'
    )
}
# Every file a desktop run of the event read (android/events/<event>.files,
# written by tools/trace_bundle_files.ps1, F608): the Donington and Washington
# lists for every bundle (every menu offers those races, F615), then the event's own. The
# lists above stay as the minimum; a traced file outside them is added here,
# and a -Stage run fails when one is missing.
$traced = @(foreach ($name in @('donington', 'washington', $Event) | Select-Object -Unique) {
    Get-Content -LiteralPath (Join-Path $PSScriptRoot "..\android\events\$name.files") |
        Where-Object { $_ -and -not $_.StartsWith('#') } | ForEach-Object { $_.Replace('/', '\') }
}) | Sort-Object -Unique
foreach ($file in $traced) {
    $covered = $items | Where-Object { $file -ieq $_ -or $file.StartsWith("$_\", [StringComparison]::OrdinalIgnoreCase) }
    if (-not $covered) { $items += $file }
}
$eventText = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "..\android\events\$Event.ini")) -replace "`r`n", "`n"

# The drive profile, shared with the code-only APK's import (P10):
# android/app/src/main/assets/grid-import/settings.ini.
$settings = [IO.File]::ReadAllText((Join-Path $PSScriptRoot '..\android\app\src\main\assets\grid-import\settings.ini'))
if ($NoAutopilot) {
    $settings = @($settings -split "`r?`n" | Where-Object { $_ -notmatch '^\[diagnostics\]|^# No touch or tilt|^autopilot' }) -join "`n"
}
# LF whatever the checkout's line endings, so the bundle version is stable.
$settings = $settings -replace "`r`n", "`n"

foreach ($item in $items) {
    if (!(Test-Path -LiteralPath (Join-Path $GameRoot $item))) { throw "Missing in the installation: $item" }
}

if ($Stage) {
    $grid = Join-Path $Stage 'Grid'
    foreach ($item in $items) {
        $target = Join-Path $grid $item
        New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
        Copy-Item -LiteralPath (Join-Path $GameRoot $item) -Destination $target -Recurse -Force
    }
    # No byte-order mark (Set-Content -Encoding utf8 writes one in PowerShell 5).
    [IO.File]::WriteAllText([IO.Path]::GetFullPath((Join-Path $Stage 'settings.ini')), $settings)
    [IO.File]::WriteAllText([IO.Path]::GetFullPath((Join-Path $Stage 'event.ini')), $eventText)
    if ($Event -eq 'drift-nurburgring' -and (Test-Path -LiteralPath $Characteristics)) {
        Copy-Item -LiteralPath $Characteristics -Destination (Join-Path $Stage 'ai-driver-characteristics.tsv') -Force
    }
    # The APK build stops here when a file the event reads is not in the stage.
    $missing = @($traced | Where-Object { -not (Test-Path -LiteralPath (Join-Path $grid $_) -PathType Leaf) })
    if ($missing.Count) { throw "Missing in the stage ($($missing.Count)): $($missing -join ', ')" }
    Write-Output "staged $Stage ($($traced.Count) traced files present)"
    return
}

$dest = "/sdcard/Android/data/$Package/files"
& $Adb shell mkdir -p "$dest/Grid/audio" "$dest/Grid/language" "$dest/Grid/track_sponsors/tag_heuer" "$dest/Grid/track_sponsors/bridgestone" "$dest/Grid/tracks/europe" "$dest/Grid/tracks/usa/washington" "$dest/Grid/cars" "$dest/Grid/frontend/fonts"
if ($LASTEXITCODE -ne 0) { throw 'adb shell mkdir failed (is the phone connected and the app installed?)' }
foreach ($item in $items) {
    $parent = (Split-Path $item) -replace '\\', '/'
    $remote = if ($parent) { "$dest/Grid/$parent/" } else { "$dest/Grid/" }
    & $Adb push --sync (Join-Path $GameRoot $item) $remote
    if ($LASTEXITCODE -ne 0) { throw "adb push failed: $item" }
}
$tmp = Join-Path ([IO.Path]::GetTempPath()) 'grid-android-settings.ini'
[IO.File]::WriteAllText($tmp, $settings)
& $Adb push $tmp "$dest/settings.ini"
if ($LASTEXITCODE -ne 0) { throw 'adb push failed: settings.ini' }
[IO.File]::WriteAllText($tmp, $eventText)
& $Adb push $tmp "$dest/event.ini"
if ($LASTEXITCODE -ne 0) { throw 'adb push failed: event.ini' }
Remove-Item -LiteralPath $tmp
if ($Event -eq 'drift-nurburgring' -and (Test-Path -LiteralPath $Characteristics)) {
    & $Adb push $Characteristics "$dest/ai-driver-characteristics.tsv"
    if ($LASTEXITCODE -ne 0) { throw 'adb push failed: ai-driver-characteristics.tsv' }
}
Write-Output "pushed to $dest"
