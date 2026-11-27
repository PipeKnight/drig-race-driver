<#
.SYNOPSIS
Rebase onto origin/main, run prepush and push HEAD to main, holding the build lock from the rebase to the push.

.DESCRIPTION
The AGENTS.md push flow in one command (docs/TOOLCHAIN.md "Pushing to main"). Windows PowerShell 5.1.

    powershell -NoProfile -ExecutionPolicy Bypass -File tools\push_main.ps1           # rebase, check, push
    powershell -NoProfile -ExecutionPolicy Bypass -File tools\push_main.ps1 -DryRun   # everything but the push

Why: a gate that waits half an hour in the build-lock queue used to test the base it was rebased
on before the wait; a code commit that another session pushed meanwhile made the pass worthless
and the gate ran again. Here the rebase happens after the lock is taken, and the push
happens before it is given back. Every session's code gate runs under that lock, so while one
gate holds it no other session's code reaches main: the gate tests the newest main and the push
goes through. Only pushes from outside the lock (owner pull requests, Codex, docs-only pushes)
can still move main during a gate; then a round repeats (a docs-only move costs seconds through
the pass cache).

One round:
  1. git fetch; rebase HEAD onto origin/main. A stop on log/ or PROJECT_LOG.md is resolved with
     tools/log_entry.py check --fix and the rebase continues; any other conflict aborts the
     rebase and fails. Then check --fix again; an entry it renumbers is committed with
     --amend, and the old number in HEAD's commit message is replaced by the new one (other
     mentions are printed as notes for the author).
  2. prepush -Plan: when the change needs cargo steps (tier crate and up, no cached pass) and
     the lock is not held yet, take it (hold limit: the prepush tier limit + 10 min) and go back
     to 1, since main may have moved during the wait.
  3. prepush -NoFetch (under the lock when taken; prepush runs nested in it, including grid-soak
     quick, which costs about 2 min of extra hold).
  4. git fetch; when origin/main moved since step 1, give the lock back and start the next
     round. Otherwise git push origin HEAD:main; a rejected push also starts the next round.
At most -MaxRounds rounds. Exit 0 after the push (or, with -DryRun, when it would push).
Commits are amended as the owner (AGENTS.md "Sessions and commits"); hooks run, never skipped.
#>
param(
    # Run everything except the push.
    [switch]$DryRun,
    [int]$MaxRounds = 5,
    # Passed through to prepush.
    [switch]$Full,
    [switch]$NoCache
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'build_lock.ps1')
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Set-Location -LiteralPath $repo
$prepush = Join-Path $PSScriptRoot 'prepush.ps1'
$Owner = @('-c', 'user.name=Evgeniy Lupashin', '-c', 'user.email=34810566+PipeKnight@users.noreply.github.com')

function Invoke-GitRaw {
    # Runs git, returns @{ Code; Out } without throwing.
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try { $o = & git @args 2>&1 | ForEach-Object { "$_" }; $code = $LASTEXITCODE } finally { $ErrorActionPreference = $saved }
    return [pscustomobject]@{ Code = $code; Out = @($o) }
}
function Invoke-Git {
    $r = Invoke-GitRaw @args
    if ($r.Code -ne 0) { throw "git $($args -join ' ') failed ($($r.Code)): $(($r.Out | Select-Object -Last 5) -join ' | ')" }
    return $r.Out
}
function Get-Rev([string]$Ref) { return ((Invoke-Git rev-parse $Ref) | Select-Object -First 1).Trim() }
function Test-Rebasing {
    $gd = ((Invoke-Git rev-parse --git-dir) | Select-Object -First 1).Trim()
    if (-not [IO.Path]::IsPathRooted($gd)) { $gd = Join-Path $repo $gd }
    return (Test-Path -LiteralPath (Join-Path $gd 'rebase-merge')) -or (Test-Path -LiteralPath (Join-Path $gd 'rebase-apply'))
}
function Invoke-LogFix {
    # tools/log_entry.py check --fix; returns its output lines, records renumberings.
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try { $o = @(& py -I tools/log_entry.py check --fix --no-fetch 2>&1 | ForEach-Object { "$_" }); $code = $LASTEXITCODE } finally { $ErrorActionPreference = $saved }
    foreach ($l in $o) {
        if ($l -match '^renumbered ([A-Z])(\d+) -> ([A-Z])(\d+)') { $script:renumbered.Add([pscustomobject]@{ Series = $Matches[1]; Old = [int]$Matches[2]; New = [int]$Matches[4] }) }
        if ($l -match '^(renumbered|renamed|regenerated|  note:|FAIL:)' -and $l -notmatch 'a rebase or merge is in progress') { Write-Host "push_main: log_entry: $l" }
    }
    return [pscustomobject]@{ Code = $code; Out = $o }
}

function Sync-Main {
    # Step 1: fetch, rebase, log fixes. Returns the origin/main commit HEAD now sits on.
    $null = Invoke-Git fetch -q origin
    $base = Get-Rev 'origin/main'
    $ahead = @(Invoke-Git rev-list "$base..HEAD").Count
    if ($ahead -eq 0) { throw 'push_main: HEAD has no commit that origin/main lacks; nothing to push' }
    if ($ahead -gt 1) { Write-Host "push_main: WARNING $ahead commits ahead of origin/main; AGENTS.md asks for one squashed commit" }
    $dirty = @(Invoke-Git status --porcelain --untracked-files=no)
    if ($dirty.Count) { throw "push_main: uncommitted changes to tracked files ($($dirty.Count)); commit or drop them first" }
    $script:renumbered = New-Object System.Collections.Generic.List[object]
    if ((Invoke-GitRaw merge-base --is-ancestor $base HEAD).Code -ne 0) {
        Write-Host "push_main: rebasing onto origin/main $($base.Substring(0, 12))"
        $env:GIT_EDITOR = 'true'
        try {
            $r = Invoke-GitRaw @Owner rebase $base
            for ($i = 0; (Test-Rebasing) -and $i -lt 50; $i++) {
                $unmerged = @(Invoke-Git diff --name-only --diff-filter=U)
                $foreign = @($unmerged | Where-Object { $_ -notlike 'log/*' -and $_ -ne 'PROJECT_LOG.md' })
                if ($foreign.Count) {
                    $null = Invoke-GitRaw rebase --abort
                    throw "push_main: rebase conflict outside log/ in $($foreign -join ', '); rebase by hand, then run again"
                }
                if ($unmerged.Count) {
                    $null = Invoke-LogFix
                    $left = @(Invoke-Git diff --name-only --diff-filter=U)
                    if ($left.Count) {
                        $null = Invoke-GitRaw rebase --abort
                        throw "push_main: log_entry.py check --fix left $($left -join ', ') unmerged; rebase by hand"
                    }
                }
                $r = Invoke-GitRaw @Owner rebase --continue
            }
            if (Test-Rebasing) { $null = Invoke-GitRaw rebase --abort; throw 'push_main: rebase did not finish' }
        } finally { Remove-Item Env:\GIT_EDITOR -ErrorAction SilentlyContinue }
    }
    $fix = Invoke-LogFix
    if ($fix.Code -ne 0) { throw "push_main: tools/log_entry.py check --fix failed: $(($fix.Out | Where-Object { $_ -like 'FAIL*' }) -join ' | ')" }
    $changed = @(Invoke-Git status --porcelain -- log PROJECT_LOG.md)
    $msg = (Invoke-Git log -1 --format=%B) -join "`n"
    $newMsg = $msg
    foreach ($n in $script:renumbered) {
        $newMsg = [regex]::Replace($newMsg, "(?<![A-Za-z0-9])$($n.Series)0*$($n.Old)(?!\d)", "$($n.Series)$($n.New)")
    }
    if ($changed.Count -or $newMsg -ne $msg) {
        if ($changed.Count) { $null = Invoke-Git add -A -- log PROJECT_LOG.md }
        $file = Join-Path ([IO.Path]::GetTempPath()) "push_main-msg-$PID.txt"
        [IO.File]::WriteAllText($file, $newMsg.TrimEnd() + "`n")
        try { $null = Invoke-Git @Owner commit --amend -q -F $file } finally { Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue }
        Write-Host "push_main: amended HEAD ($($changed.Count) log path(s)$(if ($newMsg -ne $msg) { ', message renumbered' }))"
    }
    return $base
}

function Invoke-Prepush([string[]]$Extra) {
    $a = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $prepush, '-NoFetch') + $Extra
    if ($Full) { $a += '-Full' }
    if ($NoCache) { $a += '-NoCache' }
    $saved = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
    try { $o = @(& powershell.exe @a 2>&1 | ForEach-Object { "$_" }); $code = $LASTEXITCODE } finally { $ErrorActionPreference = $saved }
    return [pscustomobject]@{ Code = $code; Out = $o }
}

$lock = $null
$result = 'FAILED'
$started = Get-Date
try {
    for ($round = 1; $round -le $MaxRounds; $round++) {
        $base = Sync-Main
        $plan = Invoke-Prepush @('-Plan')
        if ($plan.Code -ne 0) { $plan.Out | Select-Object -Last 20 | ForEach-Object { Write-Host "    $_" }; throw 'push_main: prepush -Plan failed' }
        $tierLine = @($plan.Out | Where-Object { $_ -like 'prepush: tier *' } | Select-Object -First 1)
        $tier = if ($tierLine -and $tierLine[0] -match '^prepush: tier (\w+)') { $Matches[1] } else { 'full' }
        $cachedPass = @($plan.Out | Where-Object { $_ -like '*cargo steps will be skipped*' }).Count -gt 0
        Write-Host "push_main: round $round, base $($base.Substring(0, 12)), HEAD $((Get-Rev HEAD).Substring(0, 12)), tier $tier$(if ($cachedPass) { ' (cached pass)' })"
        if ($tier -ne 'docs' -and -not $cachedPass -and -not $lock) {
            $hold = $(if ($tier -eq 'crate') { 65 } else { 55 }) + 10
            $lock = Enter-BuildLock -Purpose "push_main: prepush $tier, rebase to push" -HoldMinutes $hold
            # main may have moved while this waited: rebase again before testing.
            $round--; continue
        }
        $check = Invoke-Prepush @()
        $check.Out | Where-Object { $_ -match '^prepush: (tier |\S+ (pass|FAIL)|passed|FAILED|cargo steps)' } | ForEach-Object { Write-Host "  $_" }
        if ($check.Code -ne 0) {
            $check.Out | Select-Object -Last 30 | ForEach-Object { Write-Host "    $_" }
            throw 'push_main: prepush failed; nothing pushed'
        }
        $null = Invoke-Git fetch -q origin
        $now = Get-Rev 'origin/main'
        if ($now -ne $base) {
            Write-Host "push_main: origin/main moved to $($now.Substring(0, 12)) during the check; next round"
            if ($lock) { Exit-BuildLock $lock; $lock = $null }
            continue
        }
        if ($DryRun) { $result = "DRY RUN: would push $((Get-Rev HEAD).Substring(0, 12)) onto $($base.Substring(0, 12))"; break }
        $push = Invoke-GitRaw push origin HEAD:main
        if ($push.Code -eq 0) { $result = "PUSHED $((Get-Rev HEAD).Substring(0, 12)) onto $($base.Substring(0, 12))"; break }
        Write-Host "push_main: push rejected ($(($push.Out | Select-Object -Last 2) -join ' | ')); next round"
        if ($lock) { Exit-BuildLock $lock; $lock = $null }
    }
    if ($result -eq 'FAILED') { throw "push_main: gave up after $MaxRounds rounds; main kept moving" }
} finally {
    if ($lock) { Exit-BuildLock $lock; $lock = $null }
}
Write-Host ("push_main: {0} in {1} s" -f $result, [int]((Get-Date) - $started).TotalSeconds)
exit 0
