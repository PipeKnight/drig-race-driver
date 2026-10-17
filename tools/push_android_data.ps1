# Copies the owner's own GRID data for a solo Donington drive to the phone
# (P9, docs/ANDROID.md). Development path only; the folder picker is P10.
#
#   powershell -NoProfile -File tools\push_android_data.ps1            # adb push
#   powershell -NoProfile -File tools\push_android_data.ps1 -Stage DIR # copy to DIR\Grid (desktop check)
#
# Destination on the phone: /sdcard/Android/data/org.reassembly.grid/files,
# the app-specific storage that settings::data_dir() returns there:
#   files/Grid/...              the subset of the installation listed below
#   files/private/*.bin, *.tsv  the private tables from the main checkout's local/
#   files/settings.ini          the drive profile (written by this script)
# Nothing here is ever put into the repository or the APK. Uninstalling the app
# deletes the folder.
param(
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$Private = 'C:\Dev\Reassembly\local',
    [string]$Stage = '',
    [string]$Adb = 'C:\Users\debros\Tools\android-sdk\platform-tools\adb.exe',
    [string]$Package = 'org.reassembly.grid'
)
$ErrorActionPreference = 'Stop'

# What a solo Lacetti drive at Donington reads (checked with -Stage, docs/ANDROID.md).
$items = @(
    'grid.exe',              # BP01/DF01 are derived from it in memory ([private] derive)
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
    'track_sponsors\tag_heuer\sponsor_minor.pssg',
    'track_sponsors\bridgestone\sponsor_event.pssg',
    'cars\lac',
    'tracks\europe\donington'
)
$privateFiles = @('body-contact-directions.bin')

$settings = @'
# Solo Lacetti practice at Donington on the phone (docs/ANDROID.md).
# [private] paths are relative to this folder (the app's data dir); the game
# folder defaults to Grid/ in it on Android.
[drive]
enabled = on
[private]
body_contacts = private/body-contact-directions.bin
body_events = derive
deformation = derive
[diagnostics]
# No touch or tilt input yet (P11): the autopilot drives the lap.
autopilot = on
'@

foreach ($item in $items) {
    if (!(Test-Path -LiteralPath (Join-Path $GameRoot $item))) { throw "Missing in the installation: $item" }
}
foreach ($file in $privateFiles) {
    if (!(Test-Path -LiteralPath (Join-Path $Private $file))) { throw "Missing private file: $Private\$file" }
}

if ($Stage) {
    $grid = Join-Path $Stage 'Grid'
    foreach ($item in $items) {
        $target = Join-Path $grid $item
        New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
        Copy-Item -LiteralPath (Join-Path $GameRoot $item) -Destination $target -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path (Join-Path $Stage 'private') | Out-Null
    foreach ($file in $privateFiles) {
        Copy-Item -LiteralPath (Join-Path $Private $file) -Destination (Join-Path $Stage "private\$file") -Force
    }
    [IO.File]::WriteAllText([IO.Path]::GetFullPath((Join-Path $Stage 'settings.ini')), $settings)
    Write-Output "staged $Stage"
    return
}

$dest = "/sdcard/Android/data/$Package/files"
& $Adb shell mkdir -p "$dest/Grid/audio" "$dest/Grid/tracks/europe" "$dest/Grid/cars" "$dest/private"
if ($LASTEXITCODE -ne 0) { throw 'adb shell mkdir failed (is the phone connected and the app installed?)' }
foreach ($item in $items) {
    $parent = (Split-Path $item) -replace '\\', '/'
    $remote = if ($parent) { "$dest/Grid/$parent/" } else { "$dest/Grid/" }
    & $Adb push --sync (Join-Path $GameRoot $item) $remote
    if ($LASTEXITCODE -ne 0) { throw "adb push failed: $item" }
}
foreach ($file in $privateFiles) {
    & $Adb push (Join-Path $Private $file) "$dest/private/"
    if ($LASTEXITCODE -ne 0) { throw "adb push failed: $file" }
}
$tmp = Join-Path ([IO.Path]::GetTempPath()) 'grid-android-settings.ini'
[IO.File]::WriteAllText($tmp, $settings)
& $Adb push $tmp "$dest/settings.ini"
if ($LASTEXITCODE -ne 0) { throw 'adb push failed: settings.ini' }
Remove-Item -LiteralPath $tmp
Write-Output "pushed to $dest"
