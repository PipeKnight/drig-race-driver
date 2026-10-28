<#
.SYNOPSIS
Pre-push checks, scaled to what the branch changed (docs/TOOLCHAIN.md "Pre-push tiers").

.DESCRIPTION
Classifies every file changed since the merge base with origin/main (committed, uncommitted and
untracked) and runs the checks of the highest tier any file needs. Windows PowerShell 5.1.

    powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1          # check
    powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1 -Plan    # classify only
    powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1 -Full    # everything
    powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1 -NoCache # ignore recorded passes

Tiers (each also runs the ones above it):
  docs       no cargo. git diff --check (conflict markers fail), relative Markdown links on added
             lines must point at existing files (links into local/ are not checked), changed
             .ps1 files must parse, changed .py files must parse, and tools/log_entry.py check
             when that tool exists.
  crate      the changed leaf crates only: their release tests with --include-ignored, and
             release Clippy -D warnings for them.
  workspace  the release test suite of the whole workspace with --include-ignored and the
             workspace release Clippy -D warnings.
  full       workspace + rebuild grid-opponent-race and grid-soak, the identity check against
             tools/identity.tsv and grid-soak quick: the AGENTS.md "Check levels" gate.

Decision table, first matching row wins; a path no row names is full:
  Cargo.lock, Cargo.toml, rust-toolchain*, .cargo/**, crates/*/Cargo.toml, crates/*/build.rs
                                                           full   (dependencies, toolchain, flags)
  crates/grid-world/**                                     full   (the simulation itself)
  crates/grid-tools/src/bin/grid-opponent-race.rs, grid-soak.rs
                                                           full   (the identity and soak tools)
  crates/grid-tools/src/bin/** (other CLIs)                crate grid-tools (leaf binaries)
  crates/grid-tools/**  (library)                          full   (Session, AI, physics glue that
                                                                   the race tool and grid-soak run)
  crates/grid-formats/src/lib.rs                           full
  crates/grid-formats/src/<module>.rs, <module>/**         full if the module is "sim-reachable",
                                                           else workspace (below)
  crates/grid-formats/tests|examples|benches/**            crate grid-formats
  crates/grid-formats/**  (other)                          full
  crates/grid-app/**                                       crate grid-app
  settings/**, android/app/src/main/assets/grid-import/settings.ini
                                                           crate grid-app (its tests read them)
  crates/grid-probes/**                                    crate grid-probes (Clippy only; no tests)
  crates/** (other)                                        full
  tools/**                                                 docs, unless Rust code (not a comment) names the
                                                           file (then workspace); tools/identity.tsv
                                                           is full
  docs/**, log/**, *.md, LICENSE, .gitignore, .gitattributes, *.cmd (launchers), android/**
                                                           docs (launchers: build gate smoke;
                                                           android/: build an APK yourself)

Why grid-app is a crate tier: the dependency graph is grid-formats -> grid-tools -> grid-app,
grid-world -> grid-tools (grid-world has no dependencies; grid-probes depends on formats, world
and tools). grid-opponent-race and grid-soak are grid-tools binaries, so nothing in grid-app or
grid-probes can change an identity hash or a soak result. grid-app's own behaviour is covered by
its tests (here) and the build gate (tools\build_gate.ps1, before every playable).

"Sim-reachable" grid-formats modules are computed from the source at every run: the modules that
grid-tools or grid-world name through grid_formats::..., closed over grid-formats' own
crate::/super:: references in both directions (a module the reachable code uses, and a module
that uses reachable code, since it could add impls to reachable types). Only modules outside
that set (pure presentation readers such as the ETC2 decoder) take the workspace tier.

Order and lock: sanity, Clippy, (full: race/soak build, identity), tests, (full: grid-soak quick).
The cheap checks that fail most come first. The build lock (tools\build_lock.ps1) is taken once
for the Clippy..tests stretch and given back before grid-soak quick or after a failure.

Pass cache: a gate that passed records its code key in C:\cargo-build\prepush-cache (machine-wide).
A later run with the same key, i.e. the same content of every non-documentation path, the same
tier and steps, toolchain and private inputs, runs only sanity and names the run it relies on
(typically after a rebase onto docs/log-only commits or after log_entry.py check --fix). -NoCache
runs everything. Details: Get-CodeKey below and docs/TOOLCHAIN.md "Pre-push tiers".

Logs: -Out (default local\prepush\<time>), one file per step plus summary.md. Exit code 0 when
every check passed (or sanity passed and the cached pass covers the rest).
#>
param(
    # Run the full gate whatever changed.
    [switch]$Full,
    # Print the classification and the planned steps, run nothing.
    [switch]$Plan,
    # Compare with this ref (default origin/main, fetched first unless -NoFetch).
    [string]$Base = 'origin/main',
    [switch]$NoFetch,
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    # Folder with body-contact-directions.bin, body-impact-parameters.bin,
    # deformation-parameters.bin and ai-driver-characteristics.tsv (default: this checkout's
    # local\, else the main checkout's local\).
    [string]$PrivateDir,
    [string]$Out,
    # Run the remaining steps after a failure (default: stop at the first one).
    [switch]$KeepGoing,
    # Run the cargo steps even when the pass cache has a pass for the same code key.
    [switch]$NoCache
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'build_lock.ps1')
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH" }
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Set-Location -LiteralPath $repo
$TierNames = @('docs', 'crate', 'workspace', 'full')

function Invoke-Git {
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try { $o = & git @args 2>$null; $code = $LASTEXITCODE } finally { $ErrorActionPreference = $saved }
    if ($code -ne 0) { throw "git $($args -join ' ') failed ($code)" }
    return $o
}

# ---- what changed ----------------------------------------------------------------------------
if (-not $NoFetch -and $Base -like 'origin/*') {
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    & git fetch -q origin 2>$null
    if ($LASTEXITCODE -ne 0) { Write-Host 'prepush: git fetch failed; using the last fetched origin/main' }
    $ErrorActionPreference = $saved
}
$mergeBase = (Invoke-Git merge-base $Base HEAD | Select-Object -First 1).Trim()
$baseHead = (Invoke-Git rev-parse $Base | Select-Object -First 1).Trim()
$head = (Invoke-Git rev-parse HEAD | Select-Object -First 1).Trim()
$changed = New-Object System.Collections.Generic.List[string]
foreach ($p in @(Invoke-Git diff --no-renames --name-only "$mergeBase" HEAD) + @(Invoke-Git diff --no-renames --name-only HEAD) + @(Invoke-Git ls-files --others --exclude-standard)) {
    if ($p -and -not $changed.Contains($p)) { $changed.Add($p) }
}

# ---- grid-formats modules the simulation can reach --------------------------------------------
function Get-ModuleRefs([string]$Text, [string[]]$Prefixes, [hashtable]$Modules) {
    $found = @{}
    foreach ($prefix in $Prefixes) {
        foreach ($m in [regex]::Matches($Text, "\b$prefix\s*::\s*")) {
            $i = $m.Index + $m.Length
            if ($i -lt $Text.Length -and $Text[$i] -eq '{') {
                $depth = 0; $j = $i
                for (; $j -lt $Text.Length; $j++) {
                    if ($Text[$j] -eq '{') { $depth++ } elseif ($Text[$j] -eq '}') { $depth--; if ($depth -eq 0) { break } }
                }
                $block = $Text.Substring($i, [Math]::Min($j, $Text.Length - 1) - $i + 1)
                foreach ($w in [regex]::Matches($block, '\b[a-z_][a-z0-9_]*\b')) { if ($Modules.ContainsKey($w.Value)) { $found[$w.Value] = $true } }
            } else {
                $w = [regex]::Match($Text.Substring($i, [Math]::Min(80, $Text.Length - $i)), '^[a-z_][a-z0-9_]*')
                if ($w.Success -and $Modules.ContainsKey($w.Value)) { $found[$w.Value] = $true }
            }
        }
    }
    return @($found.Keys)
}

function Get-RustText([string]$Path) {
    if (Test-Path -LiteralPath $Path -PathType Container) {
        return (@(Get-ChildItem -LiteralPath $Path -Recurse -Filter '*.rs' -File | ForEach-Object { [IO.File]::ReadAllText($_.FullName) }) -join "`n")
    }
    if (Test-Path -LiteralPath $Path) { return [IO.File]::ReadAllText($Path) }
    return ''
}

function Get-SimReachableFormats {
    $src = Join-Path $repo 'crates\grid-formats\src'
    $modules = @{}
    foreach ($m in [regex]::Matches([IO.File]::ReadAllText((Join-Path $src 'lib.rs')), '(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([a-z_][a-z0-9_]*)\s*;')) {
        $modules[$m.Groups[1].Value] = $true
    }
    $deps = @{}
    foreach ($name in @($modules.Keys)) {
        $text = (Get-RustText (Join-Path $src "$name.rs")) + "`n" + (Get-RustText (Join-Path $src $name))
        $deps[$name] = @(Get-ModuleRefs $text @('crate', 'super') $modules | Where-Object { $_ -ne $name })
    }
    $users = (Get-RustText (Join-Path $repo 'crates\grid-tools\src')) + "`n" + (Get-RustText (Join-Path $repo 'crates\grid-world\src'))
    $reach = @{}
    foreach ($m in @(Get-ModuleRefs $users @('grid_formats') $modules)) { $reach[$m] = $true }
    do {
        $grew = $false
        foreach ($name in @($modules.Keys)) {
            if ($reach.ContainsKey($name)) { continue }
            $usesReachable = @($deps[$name] | Where-Object { $reach.ContainsKey($_) }).Count -gt 0
            $usedByReachable = @($reach.Keys | Where-Object { $deps[$_] -contains $name }).Count -gt 0
            if ($usesReachable -or $usedByReachable) { $reach[$name] = $true; $grew = $true }
        }
    } while ($grew)
    return [pscustomobject]@{ Modules = @($modules.Keys | Sort-Object); Reachable = $reach }
}

# ---- classification ---------------------------------------------------------------------------
# Lines of Rust code that name one of the strings (a test reading a tools/ file); comment lines
# and Cargo.toml comments do not count (grid-app's Cargo.toml mentions tools/build_android.ps1).
function Get-RustNaming([string[]]$Strings) {
    $patterns = @($Strings | Where-Object { $_ })
    if (-not $patterns.Count) { return @() }
    # The patterns go through a file: hundreds of -e arguments overflow the command line.
    $file = Join-Path ([IO.Path]::GetTempPath()) "prepush-patterns-$PID.txt"
    [IO.File]::WriteAllText($file, (($patterns -join "`n") + "`n"))
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try {
        $out = @(& git grep -n -F -f $file -- 'crates/*.rs' 2>$null); $code = $LASTEXITCODE
    } finally { $ErrorActionPreference = $saved; Remove-Item -LiteralPath $file -ErrorAction SilentlyContinue }
    if ($code -gt 1) { throw "git grep for tools/ names failed ($code)" }
    return @($out | Where-Object { $_ -notmatch '^[^:]+:\d+:\s*//' })
}

$script:formats = $null
function Get-Class([string]$Path) {
    $p = $Path.Replace('\', '/')
    $leaf = Split-Path -Leaf $p
    function New-Class([int]$tier, [string]$crate, [string]$why) { [pscustomobject]@{ Path = $p; Tier = $tier; Crate = $crate; Reason = $why } }
    if ($p -in @('Cargo.lock', 'Cargo.toml') -or $p -like 'rust-toolchain*' -or $p -like '.cargo/*' -or $p -match '^crates/[^/]+/(Cargo\.toml|build\.rs)$') {
        return New-Class 3 '' 'dependencies, toolchain or build flags' }
    if ($p -like 'crates/grid-world/*') { return New-Class 3 '' 'grid-world: the simulation every check runs' }
    if ($p -match '^crates/grid-tools/src/bin/(grid-opponent-race|grid-soak)\.rs$') { return New-Class 3 '' 'the identity/soak tool itself' }
    if ($p -like 'crates/grid-tools/src/bin/*') { return New-Class 1 'grid-tools' 'a grid-tools CLI other than the race and soak tools' }
    if ($p -like 'crates/grid-tools/*') { return New-Class 3 '' 'grid-tools library: run by grid-opponent-race and grid-soak' }
    if ($p -eq 'crates/grid-formats/src/lib.rs') { return New-Class 3 '' 'grid-formats crate root' }
    if ($p -match '^crates/grid-formats/src/([a-z_][a-z0-9_]*)(\.rs|/)') {
        $module = $Matches[1]
        if (-not $script:formats) { $script:formats = Get-SimReachableFormats }
        if ($script:formats.Reachable.ContainsKey($module)) { return New-Class 3 '' "grid-formats::$module is reachable from grid-tools/grid-world" }
        if ($script:formats.Modules -contains $module) { return New-Class 2 '' "grid-formats::$module is not reachable from the simulation; its users still need the workspace tests" }
        return New-Class 3 '' "grid-formats/src file outside the known modules"
    }
    if ($p -match '^crates/grid-formats/(tests|examples|benches)/') { return New-Class 1 'grid-formats' 'grid-formats tests/examples' }
    if ($p -like 'crates/grid-formats/*') { return New-Class 3 '' 'other grid-formats file' }
    if ($p -like 'crates/grid-app/*') { return New-Class 1 'grid-app' 'grid-app: no check of the simulation depends on it' }
    if ($p -like 'settings/*' -or $p -eq 'android/app/src/main/assets/grid-import/settings.ini') { return New-Class 1 'grid-app' 'settings profiles: read by grid-app tests' }
    if ($p -like 'crates/grid-probes/*') { return New-Class 1 'grid-probes' 'research probes (no tests, Clippy only)' }
    if ($p -like 'crates/*') { return New-Class 3 '' 'unknown crate path' }
    if ($p -eq 'tools/identity.tsv') { return New-Class 3 '' 'identity hashes' }
    if ($p -like 'tools/*') {
        $hits = @(Get-RustNaming @($p, $leaf))
        if ($hits.Count) { return New-Class 2 '' "tool named by crate source ($(($hits[0] -split ':')[0]))" }
        return New-Class 0 '' 'tool script, not used by any crate'
    }
    if ($p -like 'android/*') { return New-Class 0 '' 'Android project files: no cargo check covers them; build an APK (tools\build_android.ps1)' }
    if ($p -like '*.cmd') { return New-Class 0 '' 'launcher: covered by the build gate smoke runs, not here' }
    if ($p -like 'docs/*' -or $p -like 'log/*' -or $p -like '*.md' -or $p -in @('LICENSE', '.gitignore', '.gitattributes')) {
        return New-Class 0 '' 'documentation' }
    return New-Class 3 '' 'path not in the decision table'
}

$classes = @($changed | ForEach-Object { Get-Class $_ })
$tier = 0
foreach ($c in $classes) { if ($c.Tier -gt $tier) { $tier = $c.Tier } }
$why = if ($Full) { '-Full' } elseif ($classes.Count -eq 0) { 'no changes against the merge base' } else { (@($classes | Where-Object { $_.Tier -eq $tier } | Select-Object -First 3 | ForEach-Object { "$($_.Path): $($_.Reason)" }) -join '; ') }
if ($Full) { $tier = 3 }
$crates = @($classes | Where-Object { $_.Crate } | ForEach-Object { $_.Crate } | Sort-Object -Unique)

Write-Host "prepush: HEAD $($head.Substring(0, 12)), merge base $($mergeBase.Substring(0, 12)), $Base $($baseHead.Substring(0, 12)), $($changed.Count) changed path(s)"
if ($mergeBase -ne $baseHead) { Write-Host "prepush: WARNING HEAD is not rebased on $Base; these checks do not include its newest commits. Rebase, then run again." }
foreach ($c in $classes) { Write-Host ("  {0,-9} {1}  ({2})" -f $TierNames[$c.Tier], $c.Path, $c.Reason) }
$tierLine = "prepush: tier $($TierNames[$tier])" + $(if ($tier -eq 1) { " [$($crates -join ', ')]" } else { '' }) + " because $why"
Write-Host $tierLine

if (-not $PrivateDir) {
    $PrivateDir = Join-Path $repo 'local'
    if (-not (Test-Path -LiteralPath (Join-Path $PrivateDir 'body-contact-directions.bin'))) {
        $common = (Invoke-Git rev-parse --path-format=absolute --git-common-dir | Select-Object -First 1).Trim()
        $PrivateDir = Join-Path (Split-Path -Parent $common) 'local'
    }
}
$PrivateDir = [IO.Path]::GetFullPath($PrivateDir)
$private = [ordered]@{
    GRID_GAME_ROOT = [IO.Path]::GetFullPath($GameRoot)
    GRID_CONTACT_DIRECTIONS = Join-Path $PrivateDir 'body-contact-directions.bin'
    GRID_BODY_EVENT_PARAMETERS = Join-Path $PrivateDir 'body-impact-parameters.bin'
    GRID_DEFORMATION_PARAMETERS = Join-Path $PrivateDir 'deformation-parameters.bin'
}
$characteristics = Join-Path $PrivateDir 'ai-driver-characteristics.tsv'

# ---- steps -------------------------------------------------------------------------------------
# Cheap, often-failing checks first: Clippy (half of the failures, about 1 min) before the tests
# (7-12 min); in the full tier the identity check (about 1.5 min) before the tests too. Steps
# with Lock run under one hold of the build lock (taken at the first, released before the first
# step without it), so a gate does not queue again between its own steps.
$steps = New-Object System.Collections.Generic.List[object]
function Add-Step([string]$Name, [string]$Kind, [string]$Command, [bool]$Lock = $true) { $steps.Add([pscustomobject]@{ Name = $Name; Kind = $Kind; Command = $Command; Lock = $Lock }) }
Add-Step 'sanity' 'sanity' 'diff --check, Markdown links, script parse, log tool' $false
if ($tier -eq 1) {
    Add-Step 'clippy-crates' 'cargo' ("clippy --release " + (($crates | ForEach-Object { "-p $_" }) -join ' ') + ' --all-targets -- -D warnings')
    foreach ($c in $crates) {
        switch ($c) {
            'grid-app' { Add-Step 'test-grid-app' 'cargo' 'test --release -p grid-app --no-fail-fast -- --include-ignored' }
            'grid-tools' { Add-Step 'test-grid-tools-bins' 'cargo' 'test --release -p grid-tools --bins --no-fail-fast -- --include-ignored' }
            'grid-formats' { Add-Step 'test-grid-formats' 'cargo' 'test --release -p grid-formats --lib --test * --no-fail-fast -- --include-ignored' }
        }
    }
}
if ($tier -ge 2) { Add-Step 'clippy-workspace' 'cargo' 'clippy --release --workspace --all-targets -- -D warnings' }
if ($tier -ge 3) {
    Add-Step 'build-race-soak' 'cargo' 'build --release -p grid-tools --bin grid-opponent-race --bin grid-soak'
    Add-Step 'identity' 'identity' 'grid-opponent-race solo, 12-car, 12-car DF+AI vs tools/identity.tsv'
}
if ($tier -ge 2) { Add-Step 'test-workspace' 'cargo' 'test --workspace --release --lib --test * --bin grid-app --no-fail-fast -- --include-ignored' }
# grid-soak quick builds nothing and runs for minutes: outside the lock.
if ($tier -ge 3) { Add-Step 'soak-quick' 'soak' 'grid-soak quick' $false }

# ---- pass cache --------------------------------------------------------------------------------
# A gate that passed records the key of everything its cargo steps depend on; a later run with the
# same key (typically after a rebase onto docs- or log-only commits, or after log_entry.py check
# --fix renumbered an entry) runs only the sanity step. The key covers the content of every path
# outside documentation (all of crates/, settings, Cargo.*, toolchain, .cargo, tools/identity.tsv
# and tools files Rust code names, any unknown path) in the working tree, not just the diff, so a
# pass never carries over to different upstream code; plus the tier, the step commands, this
# script, rustc -vV, ~/.cargo/config.toml and the private inputs (path, size, time). Machine-wide
# (C:\cargo-build\prepush-cache), so a pass in one checkout covers the same code in another.
$CacheRoot = 'C:\cargo-build\prepush-cache'
function Test-KeyPath([string]$p, [hashtable]$NamedTools) {
    if ($p -like 'crates/*' -or $p -like 'settings/*' -or $p -eq 'android/app/src/main/assets/grid-import/settings.ini') { return $true }
    if ($p -like 'tools/*') { return ($p -eq 'tools/identity.tsv' -or $NamedTools.ContainsKey($p)) }
    if ($p -like 'docs/*' -or $p -like 'log/*' -or $p -like 'android/*' -or $p -like '*.md' -or $p -like '*.cmd' -or $p -in @('LICENSE', '.gitignore')) { return $false }
    return $true
}
function Get-CodeKey {
    # The working tree (tracked, modified and untracked files) as a tree object, through a copy
    # of the index so the real one is untouched.
    $index = [IO.Path]::GetFullPath((Invoke-Git rev-parse --git-path index | Select-Object -First 1).Trim())
    $tmp = Join-Path ([IO.Path]::GetTempPath()) "prepush-index-$PID"
    Copy-Item -LiteralPath $index -Destination $tmp -Force
    $env:GIT_INDEX_FILE = $tmp
    try { $null = Invoke-Git add -A; $tree = (Invoke-Git write-tree | Select-Object -First 1).Trim() }
    finally { Remove-Item Env:\GIT_INDEX_FILE -ErrorAction SilentlyContinue; Remove-Item -LiteralPath $tmp, "$tmp.lock" -Force -ErrorAction SilentlyContinue }
    $entries = @(Invoke-Git ls-tree -r --full-tree $tree)
    $tools = @($entries | ForEach-Object { ($_ -split "`t", 2)[1] } | Where-Object { $_ -like 'tools/*' })
    $named = @{}
    $hitText = (@(Get-RustNaming @($tools + @($tools | ForEach-Object { Split-Path -Leaf $_ }) | Sort-Object -Unique)) -join "`n")
    foreach ($t in $tools) { if ($hitText.Contains($t) -or $hitText.Contains((Split-Path -Leaf $t))) { $named[$t] = $true } }
    $text = New-Object System.Text.StringBuilder
    [void]$text.AppendLine("prepush-key 1")
    [void]$text.AppendLine("tier $($TierNames[$tier]) [$($crates -join ',')]")
    foreach ($s in $steps) { [void]$text.AppendLine("step $($s.Name) $($s.Command)") }
    [void]$text.AppendLine("script $((Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash)")
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    [void]$text.AppendLine("rustc $((& rustc -vV 2>$null) -join ' ')")
    $ErrorActionPreference = $saved
    $config = Join-Path $env:USERPROFILE '.cargo\config.toml'
    if (Test-Path -LiteralPath $config) { [void]$text.AppendLine("cargo-config $((Get-FileHash -LiteralPath $config -Algorithm SHA256).Hash)") }
    foreach ($k in $private.Keys) { [void]$text.AppendLine("env $k=$($private[$k])") }
    foreach ($f in @($private.GRID_CONTACT_DIRECTIONS, $private.GRID_BODY_EVENT_PARAMETERS, $private.GRID_DEFORMATION_PARAMETERS, $characteristics)) {
        $i = Get-Item -LiteralPath $f -ErrorAction SilentlyContinue
        [void]$text.AppendLine("private $f $(if ($i) { "$($i.Length) $($i.LastWriteTimeUtc.Ticks)" } else { 'missing' })")
    }
    $kept = 0
    foreach ($e in $entries) {
        if (Test-KeyPath ($e -split "`t", 2)[1] $named) { [void]$text.AppendLine($e); $kept++ }
    }
    $sha = [Security.Cryptography.SHA256]::Create()
    $hash = -join ($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text.ToString())) | ForEach-Object { $_.ToString('x2') })
    return [pscustomobject]@{ Key = $hash; Paths = $kept; Tree = $tree }
}
function Get-CachedPass([string]$Key) {
    $file = Join-Path $CacheRoot "$Key.json"
    if (-not (Test-Path -LiteralPath $file)) { return $null }
    try { $r = Get-Content -Raw -LiteralPath $file | ConvertFrom-Json } catch { return $null }
    if ($r.key -ne $Key -or $r.tier -ne $TierNames[$tier]) { return $null }
    return $r
}

foreach ($s in $steps) { Write-Host ("  step {0,-20} {1}" -f $s.Name, $s.Command) }

$codeKey = $null; $cached = $null
if ($tier -ge 1) {
    $k0 = Get-Date
    $codeKey = Get-CodeKey
    if (-not $NoCache) { $cached = Get-CachedPass $codeKey.Key }
    Write-Host ("prepush: code key {0} ({1} paths, {2:N1} s){3}" -f $codeKey.Key.Substring(0, 16), $codeKey.Paths, ((Get-Date) - $k0).TotalSeconds,
        $(if ($cached) { "; passed tier $($cached.tier) at $($cached.finished_local) in $($cached.out) (HEAD $($cached.head.Substring(0, 12))): cargo steps will be skipped" } elseif ($NoCache) { '; -NoCache' } else { '; no recorded pass' }))
}
if ($Plan) { exit 0 }

# ---- environment -------------------------------------------------------------------------------
if (-not $Out) { $Out = Join-Path $repo ('local\prepush\' + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
$Out = [IO.Path]::GetFullPath($Out)
New-Item -ItemType Directory -Force -Path $Out | Out-Null
if ($tier -ge 1) {
    foreach ($k in $private.Keys) {
        if (-not (Test-Path -LiteralPath $private[$k])) { throw "prepush: $k input $($private[$k]) is missing (-PrivateDir, -GameRoot)" }
        Set-Item -Path "Env:\$k" -Value $private[$k]
    }
    Remove-Item Env:\GRID_AI_CHARACTERISTICS -ErrorAction SilentlyContinue
    $env:CARGO_INCREMENTAL = '0'
}
$targetDir = $null
function Get-TargetDir {
    if (-not $script:targetDir) {
        $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        $json = & cargo metadata --format-version 1 --no-deps --offline 2>$null
        $ErrorActionPreference = $saved
        $script:targetDir = ($json | ConvertFrom-Json).target_directory
    }
    return $script:targetDir
}

# cmd.exe does the redirection (PowerShell 5.1 turns redirected native stderr into errors). The
# line goes through an environment variable: PowerShell 5.1 breaks a native argument with inner
# quoted paths that contain spaces ("C:\Program Files (x86)\...").
function Invoke-CmdLine([string]$Line) {
    $env:GRID_CMDLINE = $Line
    try { & $env:ComSpec /d /c '%GRID_CMDLINE%'; return $LASTEXITCODE } finally { Remove-Item Env:\GRID_CMDLINE -ErrorAction SilentlyContinue }
}
function Invoke-Logged([string]$Log, [string]$Exe, [string]$Arguments) {
    return Invoke-CmdLine "`"$Exe`" $Arguments > `"$Log`" 2>&1"
}

function Get-Python {
    if ($env:GRID_PYTHON -and (Test-Path -LiteralPath $env:GRID_PYTHON)) { return $env:GRID_PYTHON }
    # The py launcher (Python Install Manager), as tools/log_entry.py documents; then a python
    # that is not the Microsoft Store stub; then the Codex runtime's interpreter.
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try {
        foreach ($c in @(Get-Command py -ErrorAction SilentlyContinue) + @(Get-Command python, python3 -ErrorAction SilentlyContinue | Where-Object { $_.Source -notlike '*\WindowsApps\*' })) {
            $null = & $c.Source --version 2>&1
            if ($LASTEXITCODE -eq 0) { return $c.Source }
        }
    } finally { $ErrorActionPreference = $saved }
    $codex = Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
    if (Test-Path -LiteralPath $codex) { return $codex }
    return $null
}

function Invoke-Sanity([string]$Log) {
    $problems = New-Object System.Collections.Generic.List[string]
    $notes = New-Object System.Collections.Generic.List[string]
    # Conflict markers fail; whitespace is only reported.
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    # The repository stores CRLF text, so a CR at the end of a line is not trailing whitespace.
    $check = @(& git -c core.whitespace=cr-at-eol diff --check $mergeBase 2>$null)
    $ErrorActionPreference = $saved
    # Only git's "path:line: message" lines; the "+..." lines after them are the content.
    foreach ($l in $check) {
        if ($l -notmatch '^[^+\s].*:\d+: ') { continue }
        if ($l -match ':\d+: leftover conflict marker$') { $problems.Add("diff --check: $l") } else { $notes.Add("whitespace: $l") }
    }
    # Relative Markdown links on added lines.
    $untracked = @(Invoke-Git ls-files --others --exclude-standard)
    foreach ($c in $classes) {
        $file = Join-Path $repo $c.Path
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { continue }
        if ($c.Path -like '*.md') {
            if ($untracked -contains $c.Path) { $added = @(Get-Content -LiteralPath $file -Encoding UTF8) }
            else { $added = @(Invoke-Git diff -U0 --no-color $mergeBase -- $c.Path | Where-Object { $_ -like '+*' -and $_ -notlike '+++*' } | ForEach-Object { $_.Substring(1) }) }
            $dir = Split-Path -Parent $file
            foreach ($line in $added) {
                $targets = @([regex]::Matches($line, '\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)') | ForEach-Object { $_.Groups[1].Value })
                $ref = [regex]::Match($line, '^\s*\[[^\]]+\]:\s*(\S+)'); if ($ref.Success) { $targets += $ref.Groups[1].Value }
                foreach ($t in $targets) {
                    if ($t -match '^[a-zA-Z][a-zA-Z0-9+.-]*:' -or $t.StartsWith('#')) { continue }
                    $t = [Uri]::UnescapeDataString(($t -split '#')[0]) -replace ':\d+(-\d+)?$', ''
                    if (-not $t -or $t -match '(^|/)local/') { continue }
                    $resolved = if ($t.StartsWith('/')) { Join-Path $repo $t.TrimStart('/') } else { Join-Path $dir $t }
                    if (-not (Test-Path -LiteralPath $resolved)) { $problems.Add("$($c.Path): broken link ($t)") }
                }
            }
        }
        if ($c.Path -like '*.ps1') {
            $tokens = $null; $errors = $null
            [void][System.Management.Automation.Language.Parser]::ParseFile($file, [ref]$tokens, [ref]$errors)
            foreach ($e in @($errors)) { $problems.Add("$($c.Path):$($e.Extent.StartLineNumber): $($e.Message)") }
        }
    }
    $python = Get-Python
    $py = @($classes | Where-Object { $_.Path -like '*.py' -and (Test-Path -LiteralPath (Join-Path $repo $_.Path) -PathType Leaf) })
    if ($py.Count -and -not $python) { $notes.Add('no Python found (GRID_PYTHON); .py files not parsed') }
    foreach ($c in $py) {
        $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        $msg = & $python -I -c 'import ast,sys; ast.parse(open(sys.argv[1],''rb'').read(), sys.argv[1])' (Join-Path $repo $c.Path) 2>&1
        $ErrorActionPreference = $saved
        if ($LASTEXITCODE -ne 0) { $problems.Add("$($c.Path): $($msg | Select-Object -Last 1)") }
    }
    $logTool = Join-Path $repo 'tools\log_entry.py'
    if (Test-Path -LiteralPath $logTool) {
        if (-not $python) { $notes.Add('tools/log_entry.py present but no Python found') }
        else {
            $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
            $msg = & $python -I $logTool check 2>&1
            $code = $LASTEXITCODE
            $ErrorActionPreference = $saved
            $notes.Add("log_entry.py check: exit $code")
            if ($code -ne 0) { $problems.Add("tools/log_entry.py check failed: $(($msg | Select-Object -Last 5) -join ' | ')") }
        }
    }
    $lines = @($notes) + @($problems)
    if (-not $lines.Count) { $lines = @('sanity: no problems') }
    $lines | Set-Content -LiteralPath $Log -Encoding utf8
    foreach ($n in @($notes | Select-Object -First 10)) { Write-Host "  note: $n" }
    if ($notes.Count -gt 10) { Write-Host "  note: ... $($notes.Count - 10) more in $Log" }
    foreach ($p in $problems) { Write-Host "  FAIL: $p" }
    return [int]($problems.Count -ne 0)
}

function Invoke-Identity([string]$Log) {
    $table = Join-Path $repo 'tools\identity.tsv'
    $race = Join-Path (Get-TargetDir) 'release\grid-opponent-race.exe'
    $lines = New-Object System.Collections.Generic.List[string]
    $failed = 0
    foreach ($row in @(Get-Content -LiteralPath $table | Where-Object { $_ -and -not $_.StartsWith('#') } | Select-Object -Skip 1)) {
        $f = $row.Split("`t")
        $name = $f[0]; $raceArgs = $f[1]; $mode = $f[2]; $expected = $f[3].Trim()
        if ($mode -eq 'DF+AI') { $env:GRID_DEFORMATION_PARAMETERS = $private.GRID_DEFORMATION_PARAMETERS; $env:GRID_AI_CHARACTERISTICS = $characteristics }
        else { Remove-Item Env:\GRID_DEFORMATION_PARAMETERS, Env:\GRID_AI_CHARACTERISTICS -ErrorAction SilentlyContinue }
        $stdout = Join-Path $Out "identity-$name.txt"
        $t0 = Get-Date
        $code = Invoke-CmdLine "`"$race`" `"$($private.GRID_GAME_ROOT)`" `"$($private.GRID_CONTACT_DIRECTIONS)`" `"$($private.GRID_BODY_EVENT_PARAMETERS)`" $raceArgs > `"$stdout`" 2> `"$stdout.stderr`""
        $hash = (Get-FileHash -LiteralPath $stdout -Algorithm SHA256).Hash.ToLowerInvariant()
        $status = if ($code -ne 0) { "exit $code" } elseif ($hash -eq $expected) { 'unchanged' } else { "CHANGED (tools/identity.tsv has $expected)" }
        if ($status -ne 'unchanged') { $failed++ }
        $line = "identity $name $hash $status $([int]((Get-Date) - $t0).TotalSeconds) s"
        $lines.Add($line); Write-Host "  $line"
    }
    $env:GRID_DEFORMATION_PARAMETERS = $private.GRID_DEFORMATION_PARAMETERS
    Remove-Item Env:\GRID_AI_CHARACTERISTICS -ErrorAction SilentlyContinue
    if ($failed) { $lines.Add('A changed hash fails the check. If the change is intended, put the new hashes in tools/identity.tsv in the same commit and name the behaviour change in the log entry.') }
    $lines | Set-Content -LiteralPath $Log -Encoding utf8
    return [int]($failed -ne 0)
}

$results = New-Object System.Collections.Generic.List[object]
$started = Get-Date
$cargo = (Get-Command cargo).Source
$failedAny = $false
$n = 0
$gateLock = $null
$cachedNote = if ($cached) { "cached: tier $($cached.tier) passed with the same code key at $($cached.finished_local), run $($cached.out)" } else { $null }
try {
foreach ($s in $steps) {
    $n++
    $log = Join-Path $Out ('{0:D2}-{1}.log' -f $n, $s.Name)
    if ($cached -and $s.Kind -ne 'sanity') { $results.Add([pscustomobject]@{ Name = $s.Name; Status = 'skipped (cached pass)'; Seconds = 0; Waited = 0 }); continue }
    if ($failedAny -and -not $KeepGoing) { $results.Add([pscustomobject]@{ Name = $s.Name; Status = 'skipped'; Seconds = 0; Waited = 0 }); continue }
    $t0 = Get-Date; $waited = 0; $code = 1
    if ($gateLock -and -not $s.Lock) { Exit-BuildLock $gateLock; $gateLock = $null }
    if ($s.Lock -and -not $gateLock) {
        $gateLock = Enter-BuildLock -Purpose "prepush $($TierNames[$tier]) ($(@($steps | Where-Object { $_.Lock } | ForEach-Object { $_.Name }) -join ', '))"
        $waited = [int]($gateLock.Acquired - $t0).TotalSeconds
    }
    Write-Host "prepush: $($s.Name) ..."
    switch ($s.Kind) {
        'sanity' { $code = Invoke-Sanity $log }
        'cargo' { $code = Invoke-Logged $log $cargo $s.Command }
        'identity' { $code = Invoke-Identity $log }
        'soak' {
            $env:GRID_AI_CHARACTERISTICS = $characteristics
            $soak = Join-Path (Get-TargetDir) 'release\grid-soak.exe'
            $code = Invoke-Logged $log $soak "quick --out `"$(Join-Path $Out 'soak')`""
            Remove-Item Env:\GRID_AI_CHARACTERISTICS -ErrorAction SilentlyContinue
        }
    }
    $secs = [int]((Get-Date) - $t0).TotalSeconds
    $status = if ($code -eq 0) { 'pass' } else { 'FAIL' }
    if ($code -ne 0) {
        $failedAny = $true
        Write-Host "prepush: $($s.Name) FAILED (exit $code); last lines of $log"
        if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Tail 30 | ForEach-Object { Write-Host "    $_" } }
    }
    if ($s.Kind -eq 'cargo' -and (Test-Path -LiteralPath $log)) {
        $summary = @(Get-Content -LiteralPath $log | Where-Object { $_ -match '^test result:' })
        if ($summary.Count) {
            $passed = 0; $fails = 0
            foreach ($l in $summary) { if ($l -match '(\d+) passed; (\d+) failed') { $passed += [int]$Matches[1]; $fails += [int]$Matches[2] } }
            $status += " ($passed passed, $fails failed)"
        }
    }
    if ($s.Kind -eq 'soak' -and (Test-Path -LiteralPath $log)) {
        $last = Get-Content -LiteralPath $log | Where-Object { $_ -match 'soak_(end|summary)|passed|failed' } | Select-Object -Last 1
        if ($last) { $status += " ($($last.Trim()))" }
    }
    $results.Add([pscustomobject]@{ Name = $s.Name; Status = $status; Seconds = $secs; Waited = $waited })
    Write-Host ("prepush: {0} {1} in {2} s{3}" -f $s.Name, $status, $secs, $(if ($waited) { " ($waited s waiting for the build lock)" } else { '' }))
    # A failed gate gives the lock back at once (the remaining steps are skipped).
    if ($failedAny -and -not $KeepGoing -and $gateLock) { Exit-BuildLock $gateLock; $gateLock = $null }
}
} finally { if ($gateLock) { Exit-BuildLock $gateLock; $gateLock = $null } }
$total = [int]((Get-Date) - $started).TotalSeconds
if ($cachedNote) { Write-Host "prepush: cargo steps skipped, $cachedNote" }
# Record the pass, unless files changed while the checks ran (the key is taken again).
$recorded = $null
if ($codeKey -and -not $failedAny -and -not $cached) {
    $again = Get-CodeKey
    if ($again.Key -eq $codeKey.Key) {
        New-Item -ItemType Directory -Force -Path $CacheRoot | Out-Null
        $record = [ordered]@{
            key = $codeKey.Key; tier = $TierNames[$tier]; crates = @($crates); tree = $codeKey.Tree
            head = $head; merge_base = $mergeBase; base = $Base; base_head = $baseHead
            worktree = $repo; out = $Out; total_s = $total
            finished_local = (Get-Date -Format 'yyyy-MM-dd HH:mm'); finished_utc = [DateTime]::UtcNow.ToString('o')
            steps = @($results | ForEach-Object { "$($_.Name) $($_.Status) $($_.Seconds) s" })
        }
        $file = Join-Path $CacheRoot "$($codeKey.Key).json"
        [IO.File]::WriteAllText("$file.$PID.tmp", ($record | ConvertTo-Json -Depth 4))
        Move-Item -LiteralPath "$file.$PID.tmp" -Destination $file -Force
        $recorded = $file
        # Records are small; drop those older than 30 days.
        Get-ChildItem -LiteralPath $CacheRoot -Filter '*.json' -File | Where-Object { $_.LastWriteTime -lt (Get-Date).AddDays(-30) } | Remove-Item -Force -ErrorAction SilentlyContinue
    } else { Write-Host 'prepush: files changed while the checks ran; pass not recorded' }
}
$md = New-Object System.Collections.Generic.List[string]
$md.Add("# prepush $(Get-Date -Format 'yyyy-MM-dd HH:mm')")
$md.Add('')
$md.Add("- HEAD $head, merge base $mergeBase, $Base $baseHead")
$md.Add("- $tierLine")
$md.Add("- total $total s")
if ($codeKey) { $md.Add("- code key $($codeKey.Key) ($($codeKey.Paths) paths)" + $(if ($recorded) { ", pass recorded in $recorded" } else { '' })) }
if ($cachedNote) {
    $md.Add("- cargo steps not run: the same code key passed tier $($cached.tier) at $($cached.finished_local) in $($cached.out)")
    $md.Add("  (HEAD $($cached.head), $($cached.base) $($cached.base_head)); only sanity ran here")
}
$md.Add('')
$md.Add('| step | status | seconds | lock wait |'); $md.Add('|---|---|---|---|')
foreach ($r in $results) { $md.Add("| $($r.Name) | $($r.Status) | $($r.Seconds) | $($r.Waited) |") }
$md.Add(''); $md.Add('Changed paths:'); foreach ($c in $classes) { $md.Add("- $($TierNames[$c.Tier]) ``$($c.Path)`` ($($c.Reason))") }
$md | Set-Content -LiteralPath (Join-Path $Out 'summary.md') -Encoding utf8
Write-Host "prepush: $(if ($failedAny) { 'FAILED' } elseif ($cached) { 'passed (sanity; cargo steps by the cached pass)' } else { 'passed' }), tier $($TierNames[$tier]), $total s; logs in $Out"
exit $(if ($failedAny) { 1 } else { 0 })
