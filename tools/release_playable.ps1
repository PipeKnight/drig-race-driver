<#
.SYNOPSIS
The coordinator's playable release: merge origin/main, rebuild the PC prototype, refresh the
portable copy, build both private APKs and draft the log entry (docs/TOOLCHAIN.md "Release").

.DESCRIPTION
Run in the main checkout (it holds Codex's unpushed graphics commits; this script never pushes):

    powershell -NoProfile -ExecutionPolicy Bypass -File tools\release_playable.ps1 -DryRun
    powershell -NoProfile -ExecutionPolicy Bypass -File tools\release_playable.ps1

Preflight (also with -DryRun, which stops after it): the checkout must be a main checkout (not a
worktree; -AllowWorktree for tests), without tracked changes; origin is fetched and
`git merge-tree` must merge HEAD with origin/main without conflicts; the prototype and the
portable copy must not be running; the private inputs must exist.

Steps, each skippable:
  merge      git merge --no-edit origin/main as the owner (no push).              -NoMerge
  prototype  tools\update_prototype.ps1 (source driving + app tests, release build;
             target\release\grid-app.exe, local\playtest\current-build.json).      -NoPrototype
  gate       tools\build_gate.ps1 after building grid-soak (18-24 min); off unless -Gate.
             A failed gate stops the release before the copies.
  portable   target\release\grid-app.exe -> local\portable\GRID-Reassembly\target\release, and
             the settings\*.ini and non-Graphics Play GRID*.cmd launchers that changed or are new.
                                                                                   -NoPortable
  apk        a temporary detached worktree of origin/main in local\release\apk-src-<rev>,
             tools\build_android.ps1 -BundleData -Event drift-nurburgring, and -BundleData -Profile
             with the default bundle (-ProfileEvent), copied to local\android\apk as grid-<rev>-bundled-PRIVATE-drift-nurburgring-
             debug.apk and grid-<rev>-bundled-PRIVATE-profile-debug.apk (with their .json
             manifests); then tools\prune_build_dirs.ps1 -ForWorktree and git worktree remove.
                                                      -NoApk, -NoProfileApk, -KeepApkWorktree
  test order local\android\<APK test order>.txt: the previous APK names and sizes become the new
             ones, the "updated" time and the upper end of the F range move on, and when the
             bundled data did not change the previous revision joins the "no data copy" lists.
             The feature summary stays the coordinator's job (the draft lists the merged
             commits). A copy of the old file goes to the release folder.       -NoTestOrder
  draft      local\release\<time>.md: revisions, merged F range and commits, exe sha256,
             APK sizes and sha256, bundle version, step times; for the coordinator to turn into
             the log entry and commit.

Every cargo build takes the machine-wide build lock (tools\build_lock.ps1) inside the child
scripts. Exit code 0 when every step that ran passed.
#>
param(
    [switch]$DryRun,
    [switch]$NoMerge,
    [switch]$NoPrototype,
    [switch]$Gate,
    [switch]$NoPortable,
    [switch]$NoApk,
    [switch]$NoProfileApk,
    [switch]$NoTestOrder,
    [switch]$KeepApkWorktree,
    # Allow a linked worktree as the checkout (tests on a scratch worktree).
    [switch]$AllowWorktree,
    [string]$Remote = 'origin',
    [string]$Branch = 'main',
    [ValidateSet('donington', 'drift-nurburgring')]
    [string]$Event = 'drift-nurburgring',
    # The bundled event of the -Profile APK (the default bundle, as in the earlier releases).
    [ValidateSet('donington', 'drift-nurburgring')]
    [string]$ProfileEvent = 'donington',
    # Machine-private folder (inputs, outputs); default local\ of this checkout.
    [string]$Local,
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid',
    [string]$CommitName = 'Evgeniy Lupashin',
    [string]$CommitEmail = '34810566+PipeKnight@users.noreply.github.com'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'build_lock.ps1')
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH" }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Set-Location -LiteralPath $root
if (-not $Local) { $Local = Join-Path $root 'local' }
$Local = [IO.Path]::GetFullPath($Local)
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$releaseDir = Join-Path $Local "release\$stamp"
$draftPath = Join-Path $Local "release\$stamp.md"
# Non-ASCII text through escapes: Windows PowerShell 5.1 reads a script without a BOM as ANSI.
function U([string]$s) { return [regex]::Unescape($s) }
$testOrderPath = Join-Path $Local ('android\' + (U '\u041f\u043e\u0440\u044f\u0434\u043e\u043a \u0442\u0435\u0441\u0442\u043e\u0432 APK.txt'))
$portable = Join-Path $Local 'portable\GRID-Reassembly'
$apkDir = Join-Path $Local 'android\apk'
$characteristics = Join-Path $Local 'ai-driver-characteristics.tsv'
$ps = Join-Path $PSHOME 'powershell.exe'

function Invoke-Git {
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try { $o = & git @args 2>&1; $code = $LASTEXITCODE } finally { $ErrorActionPreference = $saved }
    if ($code -ne 0) { throw "git $($args -join ' ') failed ($code): $(($o | Select-Object -Last 3) -join ' | ')" }
    return ,@($o | ForEach-Object { "$_" })
}
# cmd.exe runs the line and does the redirection. The line goes through an environment variable:
# PowerShell 5.1 breaks a native argument with inner quoted paths that contain spaces
# ("C:\Program Files (x86)\...").
function Invoke-CmdLine([string]$Line) {
    $env:GRID_CMDLINE = $Line
    try { & $env:ComSpec /d /c '%GRID_CMDLINE%'; return $LASTEXITCODE } finally { Remove-Item Env:\GRID_CMDLINE -ErrorAction SilentlyContinue }
}
function Invoke-Script([string]$Log, [string]$Script, [string]$Arguments, [string]$WorkDir = $root) {
    Push-Location -LiteralPath $WorkDir
    try { return Invoke-CmdLine "`"$ps`" -NoProfile -ExecutionPolicy Bypass -File `"$Script`" $Arguments > `"$Log`" 2>&1" }
    finally { Pop-Location }
}
function Get-Sha([string]$Path) { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Get-RunningFrom([string]$Exe) {
    $full = [IO.Path]::GetFullPath($Exe)
    return @(Get-Process -Name grid-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [IO.Path]::GetFullPath($_.Path) -eq $full })
}

$times = [ordered]@{}
$notes = New-Object System.Collections.Generic.List[string]
function Step([string]$Name, [scriptblock]$Body) {
    Write-Host "release: $Name ..."
    $t0 = Get-Date
    & $Body
    $times[$Name] = [int]((Get-Date) - $t0).TotalSeconds
    Write-Host "release: $Name done in $($times[$Name]) s"
}

# ---- preflight ---------------------------------------------------------------------------------
$gitDir = (Invoke-Git rev-parse --path-format=absolute --git-dir)[0]
$commonDir = (Invoke-Git rev-parse --path-format=absolute --git-common-dir)[0]
if ($gitDir -ne $commonDir -and -not $AllowWorktree) { throw "release: $root is a linked worktree; run in the main checkout (or pass -AllowWorktree for a test)" }
$tracked = Invoke-Git status --porcelain --untracked-files=no
if ($tracked.Count) { throw "release: tracked changes in $root; commit or set them aside first:`n$($tracked -join "`n")" }
Invoke-Git fetch -q $Remote | Out-Null
$upstream = "$Remote/$Branch"
$preHead = (Invoke-Git rev-parse HEAD)[0]
$upRev = (Invoke-Git rev-parse $upstream)[0]
$saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
$mt = @(& git merge-tree --write-tree --name-only HEAD $upstream 2>&1)
$mtCode = $LASTEXITCODE
$ErrorActionPreference = $saved
if ($mtCode -eq 1) {
    $conflicts = @($mt | Select-Object -Skip 1 | Where-Object { $_ -and $_ -notmatch '^(Auto-merging|CONFLICT) ' })
    $hint = ''
    if ($conflicts -contains 'PROJECT_LOG.md') {
        # Since F532 entries live in log/; Codex's local commits still prepend "## Gnnn" entries.
        $hint = "`nPROJECT_LOG.md conflicts: since F532 the log entries are files in log/. Merge by hand: keep origin/main's PROJECT_LOG.md, move each local '## Gnnn' entry into its own file with ``py -I tools/log_entry.py new --series G`` (then ``check --fix``), commit the merge, and run this script again."
    }
    throw "release: merging $upstream into HEAD would conflict in: $($conflicts -join ', '). Nothing was changed.$hint"
}
if ($mtCode -ne 0) { throw "release: git merge-tree failed ($mtCode): $($mt -join ' | ')" }
$newCommits = (Invoke-Git log --format='%h %s' "$preHead..$upstream")
# A commit's own F number: "F531: ..." or "... (F530)"; earlier playable entries are not merged work.
$fNumbers = @($newCommits | Where-Object { $_ -notmatch '^\S+ F\d+: playable' } | ForEach-Object { if ($_ -match '^\S+ F(\d+):' -or $_ -match '\(F(\d+)\)\s*$') { [int]$Matches[1] } } | Sort-Object -Unique)
$fRange = if ($fNumbers.Count -gt 1) { "F$($fNumbers[0])-F$($fNumbers[-1])" } elseif ($fNumbers.Count) { "F$($fNumbers[0])" } else { 'none' }
if (-not $NoPrototype -and (Get-RunningFrom (Join-Path $root 'target\release\grid-app.exe')).Count) { throw 'release: the prototype (target\release\grid-app.exe) is running; keep it open and release after it exits' }
if (-not $NoPortable -and (Get-RunningFrom (Join-Path $portable 'target\release\grid-app.exe')).Count) { throw 'release: the portable copy is running; release after it exits' }
foreach ($f in @('body-contact-directions.bin', 'body-impact-parameters.bin', 'deformation-parameters.bin', 'ai-driver-characteristics.tsv')) {
    if (-not (Test-Path -LiteralPath (Join-Path $Local $f))) { throw "release: private input $Local\$f is missing" }
}
$plan = @()
if (-not $NoMerge) { $plan += "merge $upstream $($upRev.Substring(0, 12)) into $($preHead.Substring(0, 12)) ($($newCommits.Count) new commits, $fRange)" }
if (-not $NoPrototype) { $plan += 'update_prototype' }
if ($Gate) { $plan += 'build gate' }
if (-not $NoPortable) { $plan += "portable copy in $portable" }
if (-not $NoApk) { $plan += "APKs of $($upRev.Substring(0, 12)): $Event$(if (-not $NoProfileApk) { ' + profile' })" }
if (-not $NoTestOrder -and -not $NoApk) { $plan += 'APK test order' }
$plan += "draft $draftPath"
Write-Host "release: preflight passed in $root"
foreach ($p in $plan) { Write-Host "  - $p" }
foreach ($c in $newCommits) { Write-Host "    $c" }
if ($DryRun) { Write-Host 'release: dry run, nothing changed'; exit 0 }
New-Item -ItemType Directory -Force -Path $releaseDir | Out-Null

# ---- merge -------------------------------------------------------------------------------------
$head = $preHead
if (-not $NoMerge) {
    Step 'merge' {
        Invoke-Git -c "user.name=$CommitName" -c "user.email=$CommitEmail" merge --no-edit -m "merge: $upstream ($fRange) into Codex's local main for the playable rebuild" $upstream | Out-Host
        $script:head = (Invoke-Git rev-parse HEAD)[0]
    }
}
$changedAssets = (Invoke-Git diff --name-only $preHead $head -- settings '*.cmd')

# ---- prototype ---------------------------------------------------------------------------------
$build = $null
if (-not $NoPrototype) {
    Step 'prototype' {
        $code = Invoke-Script (Join-Path $releaseDir 'update_prototype.log') (Join-Path $root 'tools\update_prototype.ps1') "-GameRoot `"$GameRoot`""
        if ($code -ne 0) { throw "release: update_prototype failed ($code); see $releaseDir\update_prototype.log" }
        $script:build = Get-Content -LiteralPath (Join-Path $Local 'playtest\current-build.json') -Raw | ConvertFrom-Json
    }
}
$exe = Join-Path $root 'target\release\grid-app.exe'
$exeSha = if (Test-Path -LiteralPath $exe) { Get-Sha $exe } else { $null }

# ---- gate --------------------------------------------------------------------------------------
$gateLine = 'not run'
if ($Gate) {
    Step 'gate' {
        $code = Invoke-Script (Join-Path $releaseDir 'build_soak.log') (Join-Path $root 'tools\build_lock.ps1') 'cargo build --release -p grid-tools --bin grid-soak'
        if ($code -ne 0) { throw "release: grid-soak build failed; see $releaseDir\build_soak.log" }
        $code = Invoke-Script (Join-Path $releaseDir 'build_gate.log') (Join-Path $root 'tools\build_gate.ps1') ''
        $script:gateLine = (Get-Content -LiteralPath (Join-Path $releaseDir 'build_gate.log') | Where-Object { $_ -match '^gate_end' } | Select-Object -Last 1)
        if ($code -ne 0) { throw "release: build gate failed ($gateLine); see $releaseDir\build_gate.log. Nothing was copied." }
    }
}

# ---- portable ----------------------------------------------------------------------------------
$portableLine = 'not refreshed'
if (-not $NoPortable) {
    Step 'portable' {
        $dest = Join-Path $portable 'target\release'
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        Copy-Item -LiteralPath $exe -Destination (Join-Path $dest 'grid-app.exe') -Force
        $copied = Get-Sha (Join-Path $dest 'grid-app.exe')
        if ($copied -ne $exeSha) { throw 'release: the portable exe differs from target\release\grid-app.exe after the copy' }
        $synced = @()
        $sources = @(Get-ChildItem -LiteralPath (Join-Path $root 'settings') -Filter '*.ini' -File | ForEach-Object { @{ Src = $_.FullName; Rel = "settings\$($_.Name)" } })
        $sources += @(Get-ChildItem -LiteralPath $root -Filter 'Play GRID*.cmd' -File | Where-Object { $_.Name -notlike 'Play GRID Graphics*' } | ForEach-Object { @{ Src = $_.FullName; Rel = $_.Name } })
        foreach ($s in $sources) {
            $d = Join-Path $portable $s.Rel
            if (-not (Test-Path -LiteralPath $d) -or (Get-Sha $d) -ne (Get-Sha $s.Src)) { New-Item -ItemType Directory -Force -Path (Split-Path -Parent $d) | Out-Null; Copy-Item -LiteralPath $s.Src -Destination $d -Force; $synced += $s.Rel }
        }
        $script:portableLine = "exe sha256 $copied; launchers/settings updated: $(if ($synced.Count -gt 8) { "$($synced.Count) files ($(($synced | Select-Object -First 8) -join ', '), ...)" } elseif ($synced.Count) { $synced -join ', ' } else { 'none' })"
    }
}

# ---- APKs --------------------------------------------------------------------------------------
$apks = New-Object System.Collections.Generic.List[object]
$apkRev = $upRev.Substring(0, 12)
if (-not $NoApk) {
    Step 'apk' {
        $wt = Join-Path $Local "release\apk-src-$apkRev"
        if (Test-Path -LiteralPath $wt) { throw "release: $wt exists (a previous run kept it); remove it with prune_build_dirs -ForWorktree and git worktree remove first" }
        Invoke-Git worktree add --detach $wt $upRev | Out-Host
        try {
            # build_android.ps1 names a bundle of a non-default event with -<event>; the profile APK
            # gets -profile after it. The previous releases built the profile APK with the default
            # (Donington) bundle: -ProfileEvent.
            $variants = @(@{ Event = $Event; Args = ''; Suffix = $Event; Built = $(if ($Event -eq 'donington') { '' } else { "-$Event" }) })
            if (-not $NoProfileApk) { $variants += @{ Event = $ProfileEvent; Args = '-Profile'; Suffix = 'profile'; Built = $(if ($ProfileEvent -eq 'donington') { '-profile' } else { "-$ProfileEvent-profile" }) } }
            foreach ($v in $variants) {
                $log = Join-Path $releaseDir "apk-$($v.Suffix).log"
                $t0 = Get-Date
                # The lock is taken here, not only in build_android.ps1: the worktree's copy of that
                # script is origin/main's, which may predate the lock. A child that knows the lock
                # sees GRID_BUILD_LOCK and runs under this one.
                $lock = Enter-BuildLock -Purpose "release_playable APK $($v.Suffix)"
                try { $code = Invoke-Script $log (Join-Path $wt 'tools\build_android.ps1') "-BundleData -Event $($v.Event) -Characteristics `"$characteristics`" -GameRoot `"$GameRoot`" $($v.Args)" $wt }
                finally { Exit-BuildLock $lock }
                if ($code -ne 0) { throw "release: build_android $($v.Args) failed ($code); see $log" }
                $built = Join-Path $wt "local\android\apk\grid-$apkRev-bundled-PRIVATE$($v.Built)-debug.apk"
                if (-not (Test-Path -LiteralPath $built)) { throw "release: build_android did not produce $built" }
                New-Item -ItemType Directory -Force -Path $apkDir | Out-Null
                $name = "grid-$apkRev-bundled-PRIVATE-$($v.Suffix)-debug.apk"
                $out = Join-Path $apkDir $name
                Copy-Item -LiteralPath $built -Destination $out -Force
                Copy-Item -LiteralPath "$built.json" -Destination "$out.json" -Force
                $manifest = Get-Content -LiteralPath "$built.json" -Raw | ConvertFrom-Json
                $apks.Add([pscustomobject]@{ Name = $name; Bytes = (Get-Item -LiteralPath $out).Length; Sha256 = Get-Sha $out
                    Bundle = $manifest.bundled_data.version; Seconds = [int]((Get-Date) - $t0).TotalSeconds; Suffix = $v.Suffix })
            }
        } finally {
            if ($KeepApkWorktree) { $notes.Add("APK worktree kept: $wt") }
            else {
                $code = Invoke-Script (Join-Path $releaseDir 'prune.log') (Join-Path $root 'tools\prune_build_dirs.ps1') "-ForWorktree `"$wt`" -Apply"
                if ($code -ne 0) { $notes.Add("prune_build_dirs -ForWorktree $wt failed ($code); see prune.log") }
                try { Invoke-Git worktree remove --force $wt | Out-Null } catch { $notes.Add("git worktree remove $wt failed: $($_.Exception.Message)") }
            }
        }
    }
}

# ---- APK test order ----------------------------------------------------------------------------
$testOrderLine = 'not updated'
$bundleNote = ''
if (-not $NoTestOrder -and $apks.Count -and (Test-Path -LiteralPath $testOrderPath)) {
    Step 'test-order' {
        $bytes = [IO.File]::ReadAllBytes($testOrderPath)
        $bom = $bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF
        $text = [Text.Encoding]::UTF8.GetString($bytes, $(if ($bom) { 3 } else { 0 }), $bytes.Length - $(if ($bom) { 3 } else { 0 }))
        Copy-Item -LiteralPath $testOrderPath -Destination (Join-Path $releaseDir 'test-order-before.txt')
        $mb = U '\u041c\u0411'
        $old = [regex]::Match($text, "grid-([0-9a-f]{12})-bundled-PRIVATE-$([regex]::Escape($apks[0].Suffix))-debug\.apk")
        $changes = @()
        if ($old.Success -and $old.Groups[1].Value -ne $apkRev) {
            $oldRev = $old.Groups[1].Value
            foreach ($a in $apks) {
                $size = [Math]::Round($a.Bytes / 1e6)
                $pattern = "grid-$oldRev-bundled-PRIVATE-$([regex]::Escape($a.Suffix))-debug\.apk(\s*)\(\d+ $mb"
                $n = [regex]::Matches($text, $pattern).Count
                $text = [regex]::Replace($text, $pattern, { param($m) "$($a.Name)$($m.Groups[1].Value)($size $mb" })
                $changes += "$($a.Suffix): $n name(s)"
            }
            # "updated DD.MM.YYYY at HH:MM. New builds <rev> (Fa-Fb):"
            $updated = U '\u043e\u0431\u043d\u043e\u0432\u043b\u0435\u043d\u043e'
            $at = U '\u0432'
            $now = Get-Date
            $text = [regex]::Replace($text, "$updated \d\d\.\d\d\.\d{4} $at \d\d:\d\d", "$updated $($now.ToString('dd.MM.yyyy')) $at $($now.ToString('HH:mm'))")
            $text = [regex]::Replace($text, "(\b)$oldRev(\s*\(F\d+)" + [char]0x2013 + "F\d+\)", { param($m) "$apkRev$($m.Groups[2].Value)$([char]0x2013)$(if ($fNumbers.Count) { "F$($fNumbers[-1])" } else { 'F?' }))" })
            # Same bundled data as the previous APK: no data copy when installed over it either.
            $prevJson = Join-Path $apkDir "grid-$oldRev-bundled-PRIVATE-$($apks[0].Suffix)-debug.apk.json"
            if (Test-Path -LiteralPath $prevJson) {
                $prevBundle = (Get-Content -LiteralPath $prevJson -Raw | ConvertFrom-Json).bundled_data.version
                if ($prevBundle -eq $apks[0].Bundle) {
                    $builds = U '\u0441\u0431\u043e\u0440\u043a[\u0430\u0438]'
                    $text = [regex]::Replace($text, "($builds )((?:[0-9a-f]{12}, )*[0-9a-f]{12}\b)", { param($m) if ($m.Groups[2].Value -match "$oldRev|$apkRev") { $m.Value } else { "$($m.Groups[1].Value)$oldRev, $($m.Groups[2].Value)" } })
                    $script:bundleNote = "bundled data unchanged since $oldRev (version $($prevBundle.Substring(0, 12))); $oldRev added to the no-copy lists"
                } else {
                    $script:bundleNote = "bundled data CHANGED since $oldRev ($($prevBundle.Substring(0, 12)) -> $($apks[0].Bundle.Substring(0, 12))): installing over an older drift build copies the data again; fix the no-copy lists in the test order by hand"
                }
            }
            $enc = New-Object Text.UTF8Encoding($bom)
            [IO.File]::WriteAllText($testOrderPath, $text, $enc)
            $script:testOrderLine = "previous $oldRev -> $apkRev ($($changes -join ', ')); feature summary not touched"
        } else {
            $script:testOrderLine = 'no previous bundled APK name found; not changed'
        }
    }
}

# ---- draft log entry ---------------------------------------------------------------------------
$md = New-Object System.Collections.Generic.List[string]
$md.Add("# Release draft $stamp")
$md.Add('')
$md.Add("Draft log entry for the coordinator (fill in the F number, the feature summary and commit it).")
$md.Add('')
$md.Add("FNNN Playable $($head.Substring(0, 12)) with $fRange, portable copy and APKs $apkRev ($(Get-Date -Format 'yyyy-MM-dd'), Claude Code, coordinator; tools/release_playable.ps1).")
$md.Add("- Main checkout: $($preHead.Substring(0, 12)) + merge of $upstream $($upRev.Substring(0, 12)) -> $($head.Substring(0, 12)) (not pushed). New commits: $($newCommits.Count), $fRange.")
if ($build) { $md.Add("- PC prototype: $($build.display_revision), grid-app.exe sha256 $($build.executable_sha256), $((Get-Item -LiteralPath $exe).Length) bytes; update_prototype source driving and app tests passed; $($build.rustc).") }
elseif ($exeSha) { $md.Add("- PC prototype not rebuilt; current grid-app.exe sha256 $exeSha.") }
if ($Gate) { $md.Add("- Build gate: $gateLine.") }
$md.Add("- Portable copy: $portableLine.")
foreach ($a in $apks) { $md.Add("- APK $($a.Name): $($a.Bytes) bytes, sha256 $($a.Sha256), bundle $($a.Bundle.Substring(0, 12)), built in $($a.Seconds) s.") }
if ($bundleNote) { $md.Add("- $bundleNote.") }
$md.Add("- APK test order: $testOrderLine.")
if ($changedAssets.Count) { $md.Add("- Launchers/settings changed by the merge: $($changedAssets -join ', ').") }
foreach ($n in $notes) { $md.Add("- Note: $n") }
$md.Add("- Step times (s): $((@($times.Keys | ForEach-Object { "$_ $($times[$_])" })) -join ', '). Logs: local/release/$stamp/.")
$md.Add('')
$md.Add('Merged commits:')
foreach ($c in $newCommits) { $md.Add("- $c") }
$md | Set-Content -LiteralPath $draftPath -Encoding utf8
Write-Host "release: done; draft $draftPath"
$md | ForEach-Object { Write-Host "  $_" }
