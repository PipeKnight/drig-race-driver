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
    [switch]$NoAutopilot
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
    'track_sponsors\tag_heuer\sponsor_minor.pssg',
    'track_sponsors\bridgestone\sponsor_event.pssg',
    'cars\lac',
    'tracks\europe\donington'
)

$settings = @'
# Solo Lacetti practice at Donington on the phone (docs/ANDROID.md).
# The game folder defaults to Grid/ next to this file on Android.
[drive]
enabled = on
[private]
body_contacts = derive
body_events = derive
deformation = derive
[input]
# Phone controls (P11, docs/ANDROID.md "Controls"): touch zones and tilt
# steering; touch = auto turns them on with a touch screen. Tilt full lock at
# tilt_max_angle degrees; the neutral is captured at start and by CENTRE.
touch = auto
tilt = on
tilt_max_angle = 30
'@
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
    Write-Output "staged $Stage"
    return
}

$dest = "/sdcard/Android/data/$Package/files"
& $Adb shell mkdir -p "$dest/Grid/audio" "$dest/Grid/language" "$dest/Grid/track_sponsors/tag_heuer" "$dest/Grid/track_sponsors/bridgestone" "$dest/Grid/tracks/europe" "$dest/Grid/cars"
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
Remove-Item -LiteralPath $tmp
Write-Output "pushed to $dest"
