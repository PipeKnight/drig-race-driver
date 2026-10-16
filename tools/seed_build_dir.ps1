<#
.SYNOPSIS
Seed this checkout's empty cargo build dir from another checkout's build dir.

.DESCRIPTION
Every worktree has its own build dir, C:/cargo-build/{workspace-path-hash}
(~/.cargo/config.toml, F328), so each new session compiled all dependencies
(Bevy and about 400 crates) from scratch. Cargo's fingerprints for registry
dependencies do not depend on the build dir or the workspace path, so a copy of
another checkout's build dir makes those dependencies Fresh. Only our own
workspace crates (and dependencies whose features or versions differ) rebuild.
See docs/TOOLCHAIN.md, "Build and test cost".

Run it once in a new worktree before the first cargo command:
    powershell -NoProfile -File tools\seed_build_dir.ps1
It does nothing when the build dir already has content.

.PARAMETER Source
Build dir to copy from. Default: C:\cargo-build\seed, a full release build of
main kept by the coordinator. If it is missing, the main checkout's build dir
reported by `cargo metadata` is used. A stale seed is harmless: dependencies
that changed simply rebuild.

Measured 16.10 (release `cargo build -p grid-app` in a fresh worktree, 6 other
sessions building): unseeded 1607 s, 345 crates compiled; seeded 107 s, 4 crates
compiled (our own), 341 Fresh; the seed copy took 2 s for 2.3 GB.
#>
param([string]$Source)

$ErrorActionPreference = 'Stop'

function Get-BuildDir([string]$Dir) {
    Push-Location $Dir
    try {
        $json = cargo metadata --format-version 1 --no-deps
        if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed in $Dir" }
        return ($json | ConvertFrom-Json).build_directory
    } finally { Pop-Location }
}

$here = (git rev-parse --show-toplevel).Trim()
$dst = Get-BuildDir $here
if ((Test-Path -LiteralPath $dst) -and (Get-ChildItem -LiteralPath $dst -Force | Select-Object -First 1)) {
    Write-Output "seed_build_dir: $dst already has content; nothing to do."
    exit 0
}

if (-not $Source -and (Test-Path -LiteralPath 'C:\cargo-build\seed')) { $Source = 'C:\cargo-build\seed' }
if (-not $Source) {
    $common = (git rev-parse --path-format=absolute --git-common-dir).Trim()
    $main = Split-Path -Parent $common
    $Source = Get-BuildDir $main
}
if (-not (Test-Path -LiteralPath $Source)) { throw "seed source $Source does not exist" }
if ((Resolve-Path -LiteralPath $Source).Path -eq $dst) { throw "seed source is this checkout's own build dir" }

Write-Output "seed_build_dir: copying $Source -> $dst (incremental caches skipped)"
$t = Measure-Command {
    # /COPY:DAT and /DCOPY:T keep timestamps, which cargo's fingerprints compare.
    robocopy $Source $dst /E /COPY:DAT /DCOPY:T /MT:8 /R:1 /W:1 /XD incremental /NFL /NDL /NJH /NJS /NP | Out-Null
}
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with exit code $LASTEXITCODE" }
$gb = (Get-ChildItem -LiteralPath $dst -Recurse -File -Force | Measure-Object Length -Sum).Sum / 1GB
Write-Output ("seed_build_dir: done in {0:N0} s, {1:N1} GB" -f $t.TotalSeconds, $gb)
