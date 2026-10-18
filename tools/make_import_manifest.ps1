# Writes the code-only APK's import manifest (P10, docs/ANDROID.md
# "Importing the game data"): the list of the owner's GRID files the phone
# needs, with their sizes and SHA-256, so the app can check and copy a GRID
# folder the user picks. Only names, sizes and hashes go into the repository,
# never file contents.
#
#   powershell -NoProfile -File tools\make_import_manifest.ps1
#
# The file set is tools/push_android_data.ps1's (the same subset the bundled
# APK carries), staged into the ignored local/android/import-stage and
# deleted afterwards. Output: android/app/src/main/assets/grid-import/import.txt
#   grid-import 1
#   version <sha256 of the lines below>
#   <size>\t<sha256>\t<path relative to the GRID folder>
param(
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$stage = Join-Path $root 'local\android\import-stage'
& git -C $root check-ignore -q $stage
if ($LASTEXITCODE -ne 0) { throw "$stage is not ignored by git; refusing to stage game data there" }
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
& powershell -NoProfile -File (Join-Path $PSScriptRoot 'push_android_data.ps1') -GameRoot $GameRoot -Stage $stage -NoAutopilot | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'staging the game data failed' }
try {
    $grid = Join-Path $stage 'Grid'
    $prefix = $grid.TrimEnd('\').Length + 1
    $files = Get-ChildItem -LiteralPath $grid -Recurse -File -Force |
        ForEach-Object { [pscustomobject]@{ Path = $_.FullName.Substring($prefix).Replace('\', '/'); File = $_ } } |
        Sort-Object -Property Path -CaseSensitive
    $lines = New-Object System.Text.StringBuilder
    $bytes = [long]0
    foreach ($entry in $files) {
        $sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $entry.File.FullName).Hash.ToLower()
        $bytes += $entry.File.Length
        [void]$lines.Append("$($entry.File.Length)`t$sha`t$($entry.Path)`n")
    }
    $sha256 = [Security.Cryptography.SHA256]::Create()
    $version = -join ($sha256.ComputeHash([Text.Encoding]::UTF8.GetBytes($lines.ToString())) | ForEach-Object { $_.ToString('x2') })
    $out = Join-Path $root 'android\app\src\main\assets\grid-import\import.txt'
    [IO.File]::WriteAllText($out, "grid-import 1`nversion $version`n" + $lines.ToString())
    Write-Output "import manifest: $($files.Count) files, $bytes bytes, version $version -> $out"
} finally {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
