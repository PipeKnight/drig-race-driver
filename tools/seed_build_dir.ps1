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
It does nothing when the build dir already has content. It copies the seed's
desktop artifacts (release build, release tests, release Clippy) and leaves out
the Android ones (aarch64-linux-android, 2-3 GB).

    powershell -NoProfile -File tools\seed_build_dir.ps1 -Android
adds the Android artifacts (both APK variants, cargo ndk Clippy) and the host
build scripts they use, also to a build dir that already has content: it copies
only files the build dir does not have, never overwriting one.
tools/build_android.ps1 runs this before each cargo ndk build.

    powershell -NoProfile -File tools\seed_build_dir.ps1 -Reseed
replaces a build dir that already has content with a fresh copy of the seed:
for a worktree whose dependencies all went stale at once (the switch to
rust-lld marks every crate dirty). It refuses while a cargo holds the build
dir's lock. Only our own crates then rebuild.

.PARAMETER Source
Build dir to copy from. Default: C:\cargo-build\seed, built from main by the
coordinator with tools/build_seed.ps1 (its seed.json names the revision). If it
is missing, the main checkout's build dir reported by `cargo metadata` is used.
A stale seed is harmless: dependencies that changed simply rebuild.

Measured 16.10 (release `cargo build -p grid-app` in a fresh worktree, 6 other
sessions building): unseeded 1607 s, 345 crates compiled; seeded 107 s, 4 crates
compiled (our own), 341 Fresh; the seed copy took 2 s for 2.3 GB.
#>
param([string]$Source, [switch]$Android, [switch]$Reseed)

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
$hasContent = (Test-Path -LiteralPath $dst) -and (Get-ChildItem -LiteralPath $dst -Force | Select-Object -First 1)
if ($hasContent -and $Reseed) {
    # Cargo holds an OS lock on <profile>\.cargo-lock while it builds.
    foreach ($lock in Get-ChildItem -LiteralPath $dst -Recurse -Depth 2 -Filter .cargo-lock -Force -File) {
        try {
            $fs = [IO.File]::Open($lock.FullName, 'Open', 'ReadWrite', 'ReadWrite')
            try { $fs.Lock(0, 1); $fs.Unlock(0, 1) } finally { $fs.Close() }
        } catch { throw "seed_build_dir: $($lock.FullName) is locked; a cargo is building here. Retry when it is done." }
    }
    $old = "$($dst.TrimEnd('\', '/'))-old"
    Rename-Item -LiteralPath $dst -NewName (Split-Path -Leaf $old)
    cmd /c "rd /s /q `"$old`""
    $hasContent = $false
}
if ($hasContent -and -not $Android) {
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
if ((Resolve-Path -LiteralPath $Source).Path -eq (Resolve-Path -LiteralPath $dst -ErrorAction SilentlyContinue).Path) { throw "seed source is this checkout's own build dir" }

# /COPY:DAT and /DCOPY:T keep timestamps, which cargo's fingerprints compare.
# Never copy our own crates' fingerprints: they match in every worktree, and
# then only file times decide, so edits older than the seed would be ignored
# (tools/build_seed.ps1 strips them; this also covers older seeds).
$rc = @('/E', '/COPY:DAT', '/DCOPY:T', '/MT:8', '/R:1', '/W:1', '/NFL', '/NDL', '/NJH', '/NJS', '/NP',
    '/XF', 'seed.json', '/XD', 'incremental', 'grid-formats-*', 'grid-world-*', 'grid-tools-*', 'grid-app-*', 'grid-probes-*')
if ($Android) {
    $from = Join-Path $Source 'aarch64-linux-android'
    if (-not (Test-Path -LiteralPath $from)) {
        Write-Output "seed_build_dir: $Source has no Android artifacts; nothing to do."
        exit 0
    }
    Write-Output "seed_build_dir: adding missing Android and host files from $Source -> $dst"
    $t = Measure-Command {
        # /XC /XN /XO: copy only files the build dir lacks; never replace one of its own.
        foreach ($sub in 'release', 'aarch64-linux-android') {
            robocopy (Join-Path $Source $sub) (Join-Path $dst $sub) @rc /XC /XN /XO | Out-Null
            if ($LASTEXITCODE -ge 8) { throw "robocopy failed with exit code $LASTEXITCODE" }
        }
    }
    Write-Output ("seed_build_dir: done in {0:N0} s" -f $t.TotalSeconds)
    exit 0
}

Write-Output "seed_build_dir: copying $Source -> $dst (incremental caches and Android artifacts skipped)"
$t = Measure-Command {
    # The /XD list in $rc is last, so this adds to it.
    robocopy $Source $dst @rc aarch64-linux-android | Out-Null
}
if ($LASTEXITCODE -ge 8) { throw "robocopy failed with exit code $LASTEXITCODE" }
$gb = (Get-ChildItem -LiteralPath $dst -Recurse -File -Force | Measure-Object Length -Sum).Sum / 1GB
Write-Output ("seed_build_dir: done in {0:N0} s, {1:N1} GB" -f $t.TotalSeconds, $gb)
