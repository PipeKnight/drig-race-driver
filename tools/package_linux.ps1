# Builds the Linux x86_64 release packages of one commit in WSL2 (docs/LINUX.md).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\package_linux.ps1 -Commit <sha>
#
# 1. checks out exactly -Commit (default HEAD) from this repository's object store into
#    ~/drig-build on the WSL ext4 filesystem (a shallow clean checkout, never /mnt/c);
# 2. runs tools/package_linux.sh of that commit there, under the machine-wide build lock:
#    cargo build --release --locked -p grid-app plus -CargoArgs (default --no-default-features, the
#    player build), then the tar.gz, the AppImage and SHA256SUMS.txt;
# 3. the pinned appimagetool and AppImage runtime are cached (hash-checked) in the main
#    checkout's ignored local/tools/appimage/, the packages go to local/release/v<Version>/.
# The packages contain only our code and licences, never game files.
# Setup of the WSL distribution and its packages: docs/LINUX.md "Build host".
param(
    [string]$Commit = 'HEAD',
    [string]$Version = '0.1.0',
    [string]$Distro = 'Ubuntu-24.04',
    [string]$Out = '',
    [int]$HoldMinutes = 120,
    # The player build (backlog row 132): no developer tooling, as the Windows release.
    [string]$CargoArgs = '--no-default-features'
)
$ErrorActionPreference = 'Stop'

function ConvertTo-WslPath([string]$Path) {
    $full = [System.IO.Path]::GetFullPath($Path)
    if ($full -notmatch '^([A-Za-z]):\\(.*)$') { throw "not a drive path: $full" }
    return '/mnt/' + $Matches[1].ToLower() + '/' + ($Matches[2] -replace '\\', '/')
}

$repo = Split-Path -Parent $PSScriptRoot
$sha = (git -C $repo rev-parse --verify "$Commit^{commit}").Trim()
if ($LASTEXITCODE -ne 0 -or $sha -notmatch '^[0-9a-f]{40}$') { throw "unknown commit $Commit" }
$common = (git -C $repo rev-parse --path-format=absolute --git-common-dir).Trim()
$main = Split-Path -Parent $common
if (-not $Out) { $Out = Join-Path $main "local\release\v$Version" }
$tools = Join-Path $main 'local\tools\appimage'
New-Item -ItemType Directory -Force $Out, $tools | Out-Null

$gitDir = ConvertTo-WslPath $common
$outWsl = ConvertTo-WslPath $Out
$toolsWsl = ConvertTo-WslPath $tools
if ($CargoArgs -match "'") { throw "-CargoArgs must not contain a single quote" }
foreach ($p in @($gitDir, $outWsl, $toolsWsl)) {
    if ($p -match "['\s]") { throw "path with a quote or space is not supported: $p" }
}
# The commands go into a temporary script run with `wsl.exe -e bash FILE`: `wsl.exe -- CMD`
# re-parses CMD in the default shell, which would expand $(...) too early.
$lines = @(
    'set -euo pipefail',
    '[ -f ~/.cargo/env ] && . ~/.cargo/env',
    'mkdir -p ~/drig-build && cd ~/drig-build',
    '[ -d .git ] || git init -q',
    "git -c safe.directory='*' fetch -q --depth 1 '$gitDir' $sha",
    "git checkout -q --force $sha",
    'git clean -fdq',
    "test `"`$(git rev-parse HEAD)`" = $sha",
    "bash tools/package_linux.sh --version '$Version' --out '$outWsl' --tools '$toolsWsl' --cargo-args '$CargoArgs'"
)
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "drig-package-linux-$PID.sh"
[System.IO.File]::WriteAllText($tmp, ($lines -join "`n") + "`n", (New-Object System.Text.UTF8Encoding $false))
$tmpWsl = ConvertTo-WslPath $tmp
if ($tmpWsl -match "['\s]") { throw "temporary path with a quote or space is not supported: $tmpWsl" }

Write-Host "package_linux: $sha -> $Out"
try {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'build_lock.ps1') `
        -HoldMinutes $HoldMinutes wsl.exe -d $Distro --cd '~' -e bash $tmpWsl
    $rc = $LASTEXITCODE
} finally {
    Remove-Item -Force $tmp -ErrorAction SilentlyContinue
}
if ($rc -eq 0) { Write-Host "VERDICT pass $sha out=$Out" } else { Write-Host "VERDICT fail rc=$rc $sha" }
exit $rc
