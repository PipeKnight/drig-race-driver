<#
.SYNOPSIS
  Report disk usage of the project folder: cargo target dirs, worktrees and local/ captures.

.DESCRIPTION
  Read-only. Enumerates files with .NET (much faster than du under Git Bash on this
  machine) and prints sizes in GB/MB. With -Children <dir> it lists each immediate
  child of that directory with its size, file count, largest file and dominant
  file extensions, and can write the table as TSV for docs/DISK_USAGE.md.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File tools\disk_report.ps1
  powershell -ExecutionPolicy Bypass -File tools\disk_report.ps1 -Children local\behavior -Tsv local\disk\behavior.tsv
#>
param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot),
    [string]$Children = '',
    [string]$Tsv = '',
    [int]$Top = 40
)

$ErrorActionPreference = 'Stop'

function Measure-Tree([string]$Path) {
    $bytes = [long]0; $count = 0; $largest = $null; $largestLen = [long]0
    $byExt = @{}
    $item = Get-Item -LiteralPath $Path -Force
    if (-not $item.PSIsContainer) {
        return [pscustomobject]@{ Bytes = $item.Length; Files = 1; Largest = $item.Name; LargestBytes = $item.Length; Ext = @{ ($item.Extension.ToLower()) = $item.Length } }
    }
    if ('System.IO.EnumerationOptions' -as [type]) {
        $opts = New-Object System.IO.EnumerationOptions
        $opts.RecurseSubdirectories = $true
        $opts.IgnoreInaccessible = $true
        $opts.AttributesToSkip = [System.IO.FileAttributes]::ReparsePoint
        $files = [System.IO.Directory]::EnumerateFiles($Path, '*', $opts)
    } else {
        # Windows PowerShell 5.1 (.NET Framework) has no EnumerationOptions overload.
        $files = [System.IO.Directory]::EnumerateFiles($Path, '*', [System.IO.SearchOption]::AllDirectories)
    }
    foreach ($f in $files) {
        $fi = New-Object System.IO.FileInfo $f
        $len = $fi.Length
        $bytes += $len; $count++
        if ($len -gt $largestLen) { $largestLen = $len; $largest = $f.Substring($Path.Length).TrimStart('\') }
        $ext = $fi.Extension.ToLower(); if (-not $ext) { $ext = '(none)' }
        if ($byExt.ContainsKey($ext)) { $byExt[$ext] += $len } else { $byExt[$ext] = $len }
    }
    [pscustomobject]@{ Bytes = $bytes; Files = $count; Largest = $largest; LargestBytes = $largestLen; Ext = $byExt }
}

function Format-GB([long]$b) { '{0,8:N2} GB' -f ($b / 1GB) }

function Top-Ext($ext, [int]$n = 3) {
    ($ext.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First $n |
        ForEach-Object { '{0}={1:N0}MB' -f $_.Key, ($_.Value / 1MB) }) -join ' '
}

Set-Location -LiteralPath $Root

if ($Children) {
    $base = (Resolve-Path -LiteralPath $Children).Path
    $rows = foreach ($c in Get-ChildItem -LiteralPath $base -Force) {
        $m = Measure-Tree $c.FullName
        [pscustomobject]@{
            Name = $c.Name; Bytes = $m.Bytes; Files = $m.Files
            Modified = $c.LastWriteTime.ToString('yyyy-MM-dd')
            Largest = $m.Largest; LargestBytes = $m.LargestBytes; Ext = (Top-Ext $m.Ext)
        }
    }
    $rows = $rows | Sort-Object Bytes -Descending
    $total = ($rows | Measure-Object Bytes -Sum).Sum
    "{0}: {1} in {2} entries" -f $Children, (Format-GB $total), @($rows).Count
    $rows | Select-Object -First $Top | ForEach-Object {
        '{0} {1,7} files  {2}  {3}' -f (Format-GB $_.Bytes), $_.Files, $_.Name, $_.Ext
    }
    if ($Tsv) {
        $dir = Split-Path -Parent $Tsv
        if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force $dir | Out-Null }
        "name`tbytes`tfiles`tmodified`tlargest`tlargest_bytes`text" | Set-Content -Encoding utf8 $Tsv
        $rows | ForEach-Object { "{0}`t{1}`t{2}`t{3}`t{4}`t{5}`t{6}" -f $_.Name, $_.Bytes, $_.Files, $_.Modified, $_.Largest, $_.LargestBytes, $_.Ext } |
            Add-Content -Encoding utf8 $Tsv
        "TSV written to $Tsv"
    }
    return
}

# Overview: the usual heavy hitters.
$targets = @('target', 'local', 'local\behavior')
$targets += Get-ChildItem -LiteralPath '.claude\worktrees' -Directory -ErrorAction SilentlyContinue |
    ForEach-Object { Join-Path '.claude\worktrees' $_.Name; Join-Path (Join-Path '.claude\worktrees' $_.Name) 'target' }
$sum = [long]0
foreach ($t in $targets) {
    if (-not (Test-Path -LiteralPath $t)) { continue }
    $m = Measure-Tree (Resolve-Path -LiteralPath $t).Path
    '{0} {1,8} files  {2}' -f (Format-GB $m.Bytes), $m.Files, $t
}
$all = Measure-Tree $Root
'{0} {1,8} files  (whole project folder)' -f (Format-GB $all.Bytes), $all.Files
