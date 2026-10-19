<#
.SYNOPSIS
Build a new cargo build-dir seed beside C:\cargo-build\seed and swap it in.

.DESCRIPTION
tools/seed_build_dir.ps1 copies the seed into a new worktree's empty build dir,
so registry dependencies (Bevy and about 400 crates) are Fresh there
(docs/TOOLCHAIN.md, "Build and test cost"). The seed holds the artifacts of
exactly the commands sessions run, built from one clean checkout of main:

  1. cargo build --release                                 (default members)
  2. cargo test --workspace --release --lib --test * --bin grid-app --no-run
  3. cargo clippy --release --workspace --all-targets -- -D warnings
  4. tools/build_android.ps1             (cargo ndk, aarch64, release)
  5. tools/build_android.ps1 -Profile    (feature `profile`: other Bevy hashes)
  6. tools/build_android.ps1 -Clippy     (cargo ndk clippy, the same environment)

Steps 4-6 put their target artifacts under aarch64-linux-android\ (copied only
by seed_build_dir.ps1 -Android, i.e. by build_android.ps1 when needed) and
their host build scripts and proc-macros under release\. They must share one
environment: jni-sys's build script reruns when JAVA_HOME changes, and a rerun
makes every Android crate built before it stale (cargo compares file times).
A Clippy run without JAVA_HOME once spoiled the APK artifacts of a seed.

The build runs into -Out (default C:\cargo-build\seed-next) through
CARGO_BUILD_BUILD_DIR, so the live seed stays valid while it builds. With
-Swap the old seed is renamed to seed-old, the new one to seed, and seed-old is
deleted. seed.json records the revision, rustc, every step's time and the size.

    git -C C:\Dev\Reassembly worktree add --detach C:\Dev\Reassembly\.claude\worktrees\tmp-seed origin/main
    powershell -NoProfile -File tools\build_seed.ps1 -Checkout C:\Dev\Reassembly\.claude\worktrees\tmp-seed -Swap
    (then remove the temporary worktree; its own build dir stays empty)

.PARAMETER Checkout
A clean checkout of the revision to seed from (a temporary worktree of main).

.PARAMETER NoAndroid
Skip steps 4-6.

.PARAMETER AndroidOnly
Redo only steps 4-6, after deleting -Out's aarch64-linux-android.
#>
param(
    [Parameter(Mandatory)] [string]$Checkout,
    [string]$Out = 'C:\cargo-build\seed-next',
    [string]$Live = 'C:\cargo-build\seed',
    [switch]$NoAndroid,
    [switch]$AndroidOnly,
    [switch]$Swap
)
$ErrorActionPreference = 'Stop'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$Checkout = (Resolve-Path -LiteralPath $Checkout).Path
if (@(& git -C $Checkout status --porcelain).Count -ne 0) { throw "$Checkout has local changes; seed from a clean checkout" }
$revision = (& git -C $Checkout rev-parse HEAD).Trim()
if ((Test-Path -LiteralPath $Out) -and (Get-ChildItem -LiteralPath $Out -Force | Select-Object -First 1)) {
    Write-Output "build_seed: $Out exists; continuing into it (cargo reuses what is there)."
}
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$env:CARGO_BUILD_BUILD_DIR = $Out

$steps = New-Object System.Collections.Generic.List[object]
function Step([string]$name, [scriptblock]$body) {
    Write-Output "build_seed: $name"
    $t = Measure-Command { & $body | Out-Host }
    if ($LASTEXITCODE -ne 0) { throw "build_seed: step '$name' failed ($LASTEXITCODE)" }
    $steps.Add([ordered]@{ step = $name; seconds = [int]$t.TotalSeconds })
}

Push-Location $Checkout
try {
    if (-not $AndroidOnly) {
        Step 'cargo build --release' { cargo build --release }
        Step 'cargo test --no-run' { cargo test --workspace --release --lib --test '*' --bin grid-app --no-run }
        Step 'cargo clippy' { cargo clippy --release --workspace --all-targets -- -D warnings }
    }
    if (-not $NoAndroid) {
        $android = Join-Path $Checkout 'tools\build_android.ps1'
        if (-not (Get-Command $android).Parameters.ContainsKey('Clippy')) { throw "$android lacks -Clippy; seed from a newer main" }
        $aarch64 = Join-Path $Out 'aarch64-linux-android'
        if ($AndroidOnly -and (Test-Path -LiteralPath $aarch64)) { cmd /c "rd /s /q `"$aarch64`"" }
        # -NoSeed: never top up the new seed from the old one. (Splatting '-NoSeed'
        # into powershell.exe mangles it in Windows PowerShell 5.1, so no splat.)
        Step 'android apk' { powershell -NoProfile -ExecutionPolicy Bypass -File $android -NoSeed }
        Step 'android apk -Profile' { powershell -NoProfile -ExecutionPolicy Bypass -File $android -Profile -NoSeed }
        Step 'android clippy' { powershell -NoProfile -ExecutionPolicy Bypass -File $android -Clippy -NoSeed }
    }
} finally { Pop-Location }

# Incremental caches are skipped by seed_build_dir.ps1; drop them here too.
Get-ChildItem -LiteralPath $Out -Directory -Recurse -Filter incremental -Force | ForEach-Object { cmd /c "rd /s /q `"$($_.FullName)`"" }

# Drop our own crates. Cargo hashes workspace crates relative to the workspace
# root, so their fingerprints match in every worktree, and then only file times
# decide freshness: a worktree whose edits are older than the seed would keep
# the seed's code. Each unit's files share its hash: .fingerprint\<package>-<hash>,
# build\<package>-<hash>, deps\<target>-<hash>.*.
$workspace = 'grid-formats', 'grid-world', 'grid-tools', 'grid-app', 'grid-probes'
foreach ($profileDir in Get-ChildItem -LiteralPath $Out -Directory -Recurse -Filter .fingerprint -Force | ForEach-Object { $_.Parent.FullName }) {
    foreach ($fp in Get-ChildItem -LiteralPath (Join-Path $profileDir '.fingerprint') -Directory -Force) {
        $m = [regex]::Match($fp.Name, '^(.+)-([0-9a-f]{16})$')
        if (-not $m.Success -or $workspace -notcontains $m.Groups[1].Value) { continue }
        $hash = $m.Groups[2].Value
        Get-ChildItem -LiteralPath (Join-Path $profileDir 'deps') -File -Force -Filter "*-$hash*" -ErrorAction SilentlyContinue | Remove-Item -Force
        Get-ChildItem -LiteralPath (Join-Path $profileDir 'build') -Directory -Force -Filter "*-$hash" -ErrorAction SilentlyContinue | ForEach-Object { cmd /c "rd /s /q `"$($_.FullName)`"" }
        cmd /c "rd /s /q `"$($fp.FullName)`""
    }
    # Binaries are not always named by their fingerprint's hash; without a
    # fingerprint they would rebuild anyway, this only saves space.
    foreach ($target in 'grid_formats', 'grid_world', 'grid_tools', 'grid_app', 'grid_android', 'grid_probes', 'grid_soak', 'grid_opponent_race', 'grid_private_parameters') {
        Get-ChildItem -LiteralPath (Join-Path $profileDir 'deps') -File -Force -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -match "^(lib)?$target-[0-9a-f]{16}" } | Remove-Item -Force
    }
}
$bytes = (Get-ChildItem -LiteralPath $Out -Recurse -File -Force | Measure-Object Length -Sum).Sum
[ordered]@{
    revision = $revision
    built_utc = [DateTime]::UtcNow.ToString('o')
    rustc = (& rustc --version)
    cargo = (& cargo --version)
    android = (-not $NoAndroid)
    steps = $steps
    bytes = $bytes
} | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 -LiteralPath (Join-Path $Out 'seed.json')
Write-Output ("build_seed: {0:N2} GiB in {1}" -f ($bytes / 1GB), $Out)

if ($Swap) {
    $old = "$Live-old"
    if (Test-Path -LiteralPath $old) { cmd /c "rd /s /q `"$old`"" }
    if (Test-Path -LiteralPath $Live) { Rename-Item -LiteralPath $Live -NewName (Split-Path -Leaf $old) }
    Rename-Item -LiteralPath $Out -NewName (Split-Path -Leaf $Live)
    if (Test-Path -LiteralPath $old) { cmd /c "rd /s /q `"$old`"" }
    Write-Output "build_seed: $Live now holds $revision."
}
