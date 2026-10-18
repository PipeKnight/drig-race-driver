<#
.SYNOPSIS
Delete cargo build dirs under C:\cargo-build that no live checkout uses.

.DESCRIPTION
Every checkout builds into its own C:/cargo-build/{workspace-path-hash}
(~/.cargo/config.toml, F328). Removing a worktree does not remove that dir, so
build dirs of archived sessions and of temporary worktrees piled up: on
19.10.2026 there were 22 orphans with 82 GiB next to 14 live dirs with 57 GiB
(docs/DISK_USAGE.md, section 9).

Live dirs come from `git worktree list` (the main checkout, child worktrees,
Codex's worktrees and nested ones such as local/lemans/base-src): for each one
`cargo metadata --no-deps --offline` reports its build dir. A hash dir that no
live checkout reports is an orphan. Orphans with a file changed within
-MinIdleHours are kept anyway, because a scratch workspace that is not a git
worktree may still be building there. `seed` and other named dirs (for example
`perf-android`) are never touched.

Dry run by default; -Apply deletes.

    powershell -NoProfile -File tools\prune_build_dirs.ps1
    powershell -NoProfile -File tools\prune_build_dirs.ps1 -Apply

When archiving a session, delete its build dir BEFORE `git worktree remove`
(the path must still resolve):
    powershell -NoProfile -File tools\prune_build_dirs.ps1 -ForWorktree <path> -Apply

.PARAMETER Root
Parent of the build dirs. Default C:\cargo-build.

.PARAMETER MinIdleHours
Keep an orphan whose newest file is younger than this. Default 6.

.PARAMETER ForWorktree
Delete only the build dir of this checkout (it must still exist), whatever its age.
#>
param(
    [string]$Root = 'C:\cargo-build',
    [double]$MinIdleHours = 6,
    [string]$ForWorktree = '',
    [switch]$Apply
)

$ErrorActionPreference = 'Stop'

function Get-BuildDir([string]$Dir) {
    if (-not (Test-Path -LiteralPath (Join-Path $Dir 'Cargo.toml'))) { return $null }
    Push-Location -LiteralPath $Dir
    try {
        $json = cargo metadata --format-version 1 --no-deps --offline 2>$null
        if ($LASTEXITCODE -ne 0) { return $null }
        $b = ($json | ConvertFrom-Json).build_directory
        if (-not $b) { return $null }
        return ($b -replace '/', '\').TrimEnd('\')
    } finally { Pop-Location }
}

function Measure-Dir([string]$Dir) {
    $bytes = [long]0; $newest = [datetime]::MinValue
    foreach ($f in [System.IO.Directory]::EnumerateFiles($Dir, '*', [System.IO.SearchOption]::AllDirectories)) {
        $fi = New-Object System.IO.FileInfo $f
        $bytes += $fi.Length
        if ($fi.LastWriteTime -gt $newest) { $newest = $fi.LastWriteTime }
    }
    [pscustomobject]@{ Bytes = $bytes; Newest = $newest }
}

function Remove-Tree([string]$Dir) {
    # rd is much faster than Remove-Item on tens of thousands of files.
    cmd /c "rd /s /q `"$Dir`""
    if (Test-Path -LiteralPath $Dir) { throw "could not delete $Dir (a process may hold a file)" }
}

if ($ForWorktree) {
    $dir = Get-BuildDir (Resolve-Path -LiteralPath $ForWorktree).Path
    if (-not $dir) { throw "cargo metadata found no build dir for $ForWorktree" }
    if (-not (Test-Path -LiteralPath $dir)) { Write-Output "prune_build_dirs: $dir does not exist; nothing to do."; exit 0 }
    $m = Measure-Dir $dir
    Write-Output ("prune_build_dirs: {0} {1:N2} GiB (build dir of {2})" -f $dir, ($m.Bytes / 1GB), $ForWorktree)
    if ($Apply) { Remove-Tree $dir; Write-Output 'prune_build_dirs: deleted.' } else { Write-Output 'prune_build_dirs: dry run, pass -Apply to delete.' }
    exit 0
}

$top = (git rev-parse --path-format=absolute --git-common-dir).Trim()
$repo = Split-Path -Parent $top
$live = @{}
$lines = git -C $repo worktree list --porcelain
foreach ($line in $lines) {
    if ($line -notlike 'worktree *') { continue }
    $wt = $line.Substring(9) -replace '/', '\'
    if (-not (Test-Path -LiteralPath $wt)) { continue }
    $b = Get-BuildDir $wt
    if ($b) { $live[$b.ToLower()] = $wt; Write-Output "live    $b  <- $wt" }
}

$cutoff = (Get-Date).AddHours(-$MinIdleHours)
$total = [long]0
foreach ($p in Get-ChildItem -LiteralPath $Root -Directory) {
    if ($p.Name -notmatch '^[0-9a-f]{2}$') { continue }   # seed, perf-android, ...
    foreach ($h in Get-ChildItem -LiteralPath $p.FullName -Directory) {
        if ($live.ContainsKey($h.FullName.ToLower())) { continue }
        $m = Measure-Dir $h.FullName
        $gb = $m.Bytes / 1GB
        if ($m.Newest -gt $cutoff) {
            Write-Output ("keep    {0} {1,6:N2} GiB  changed {2:MM-dd HH:mm}, younger than {3} h" -f $h.FullName, $gb, $m.Newest, $MinIdleHours)
            continue
        }
        Write-Output ("orphan  {0} {1,6:N2} GiB  last change {2:MM-dd HH:mm}" -f $h.FullName, $gb, $m.Newest)
        $total += $m.Bytes
        if ($Apply) { Remove-Tree $h.FullName }
    }
    if ($Apply -and -not (Get-ChildItem -LiteralPath $p.FullName -Force | Select-Object -First 1)) { Remove-Item -LiteralPath $p.FullName }
}
$verb = if ($Apply) { 'deleted' } else { 'would delete (dry run, pass -Apply)' }
Write-Output ("prune_build_dirs: {0} {1:N2} GiB of orphaned build dirs." -f $verb, ($total / 1GB))
