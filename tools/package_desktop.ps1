<#
.SYNOPSIS
Package the Windows player release without an archive: DRIG-vVERSION-windows-x86_64.exe (grid-app
without dev-tools), DRIG-vVERSION-LICENSES.txt (every licence notice in one file) and SHA256SUMS.txt (docs/SETTINGS.md).

.DESCRIPTION
    powershell -NoProfile -ExecutionPolicy Bypass -File tools\package_desktop.ps1
    powershell -NoProfile -ExecutionPolicy Bypass -File tools\package_desktop.ps1 -NoBuild

Builds `cargo build --release --locked -p grid-app --bin grid-app --no-default-features` under the
machine-wide build lock with GRID_BUILD_REVISION set to the checkout's revision (12 hex digits,
"-dirty" when the work tree has changes), then writes the release files into the MAIN checkout's
local\release\vVERSION\ (the default -OutDir), whichever worktree runs it. Owner rule of 2026-11-24:
no zip assets; the exe is published directly and the licence notices travel as one plain-text
file. The exe embeds what it needs (font, shaders) and reads only the player's GRID installation.
Code and notices only: never game data. Existing files of the same name are replaced.

SHA256SUMS.txt lists every release file in the folder (.exe, .txt notices, .AppImage, .dmg, .gz),
so other platforms can drop theirs there and rerun with -NoBuild.

Prints a verdict line: package=pass exe=... sha256=... notices=... revision=..., or package=fail reason=...
#>
param(
    [string]$Version,
    [string]$OutDir,
    [switch]$NoBuild,
    [int]$HoldMinutes = 40
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

function Fail([string]$reason) {
    Write-Output "package=fail reason=$reason"
    exit 1
}

if (-not $Version) {
    $line = Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if (-not $line) { Fail 'no workspace version in Cargo.toml' }
    $Version = $line.Matches[0].Groups[1].Value
}
if (-not $OutDir) {
    $common = (& git -C $root rev-parse --path-format=absolute --git-common-dir).Trim()
    $OutDir = Join-Path (Split-Path -Parent $common) "local\release\v$Version"
}
$revision = (& git -C $root rev-parse HEAD).Trim().Substring(0, 12)
$dirty = [bool](& git -C $root status --porcelain --untracked-files=no)
if ($dirty) { $revision += '-dirty' }

$exe = Join-Path $root 'target\release\grid-app.exe'
if (-not $NoBuild) {
    $env:GRID_BUILD_REVISION = $revision
    $buildLog = Join-Path $env:TEMP "drig-package-build-$PID.log"
    # Windows PowerShell turns a native command's stderr lines into errors; with 'Stop' the first
    # "Compiling ..." line would end the script.
    $ErrorActionPreference = 'Continue'
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'tools\build_lock.ps1') -HoldMinutes $HoldMinutes `
        cargo build --release --locked -p grid-app --bin grid-app --no-default-features *> $buildLog
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) { Fail "build exit=$code log=$buildLog" }
}
if (-not (Test-Path -LiteralPath $exe)) { Fail "missing $exe" }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$outExe = Join-Path $OutDir "DRIG-v$Version-windows-x86_64.exe"
Copy-Item -LiteralPath $exe -Destination $outExe -Force

# One notices file: the GPL first, then every bundled library and font notice under a heading.
$notices = @(
    @{ Title = "DRIG $Version (revision $revision): GNU General Public License, version 3"; Path = Join-Path $root 'LICENSE' }
)
Get-ChildItem -LiteralPath (Join-Path $root 'LICENSES') -File | Sort-Object Name | ForEach-Object {
    $notices += @{ Title = "Included library notice: $($_.BaseName)"; Path = $_.FullName }
}
$notices += @{ Title = 'Included font: Fira Mono (SIL Open Font License 1.1)'; Path = Join-Path $root 'crates\grid-app\assets\fonts\FiraMono-LICENSE' }
$text = New-Object Text.StringBuilder
foreach ($n in $notices) {
    if (-not (Test-Path -LiteralPath $n.Path)) { Fail "missing notice $($n.Path)" }
    $rule = '=' * 78
    [void]$text.Append("$rule`n$($n.Title)`n$rule`n`n")
    [void]$text.Append(((Get-Content -LiteralPath $n.Path -Raw) -replace "`r`n", "`n").TrimEnd() + "`n`n")
}
$outNotices = Join-Path $OutDir "DRIG-v$Version-LICENSES.txt"
[IO.File]::WriteAllText($outNotices, $text.ToString().Replace("`n", "`r`n"), (New-Object Text.UTF8Encoding($false)))

# SHA256SUMS.txt lists every release file in the folder (other platforms may add theirs).
$sums = Get-ChildItem -LiteralPath $OutDir -File |
    Where-Object { $_.Name -ne 'SHA256SUMS.txt' -and $_.Extension -in '.exe', '.txt', '.gz', '.AppImage', '.dmg' } |
    Sort-Object Name | ForEach-Object { '{0}  {1}' -f (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant(), $_.Name }
[IO.File]::WriteAllText((Join-Path $OutDir 'SHA256SUMS.txt'), (($sums -join "`n") + "`n"), (New-Object Text.UTF8Encoding($false)))
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $outExe).Hash.ToLowerInvariant()
Write-Output "package=pass exe=$outExe sha256=$hash notices=$outNotices sums=$(@($sums).Count) revision=$revision"
