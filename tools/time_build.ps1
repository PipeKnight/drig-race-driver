<#
.SYNOPSIS
Time one build or test command and record the machine load while it runs.

.DESCRIPTION
Other sessions build at the same time, so a wall-clock number means little
without the load beside it (docs/TOOLCHAIN.md, "Build and test cost"). This
runs the command, samples every 2 s how many rustc/cargo/link/clippy-driver
processes OTHER than its own run (by parent tree), and the total CPU load, and
appends one JSON line to -Log:

    {"label", "command", "seconds", "exit", "load_start", "load_mean",
     "load_max", "cpu_mean", "free_gib_min", "top_memory", "started", "cwd"}

top_memory: the five process names with the most private memory (GB, summed
over processes of one name) at the sample with the least free RAM; the owner's
apps use this PC too.

load_* count foreign rustc + link processes (the compile/link workers); cargo
processes are only drivers and are not counted.

    powershell -NoProfile -File tools\time_build.ps1 -Label lld-build -Log local\build-speed\times.jsonl -Run "cargo build --release"

-Run is one command line for cmd.exe, so cargo's flags are not taken for
PowerShell parameters.
#>
param(
    [Parameter(Mandatory)] [string]$Label,
    [Parameter(Mandatory)] [string]$Log,
    [Parameter(Mandatory)] [string]$Run
)
$ErrorActionPreference = 'Stop'

function Get-Descendants([int]$Root, $All) {
    $ids = New-Object 'System.Collections.Generic.HashSet[int]'
    [void]$ids.Add($Root)
    $grew = $true
    while ($grew) {
        $grew = $false
        foreach ($p in $All) {
            if ($ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)) { $grew = $true }
        }
    }
    return $ids
}

function Sample([int]$Own) {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name
    $mine = if ($Own) { Get-Descendants $Own $all } else { New-Object 'System.Collections.Generic.HashSet[int]' }
    $foreign = @($all | Where-Object { $_.Name -in @('rustc.exe', 'link.exe', 'rust-lld.exe', 'lld-link.exe') -and -not $mine.Contains([int]$_.ProcessId) }).Count
    $os = Get-CimInstance Win32_OperatingSystem -Property FreePhysicalMemory
    $cpu = (Get-CimInstance Win32_Processor -Property LoadPercentage | Measure-Object LoadPercentage -Average).Average
    $top = (Get-Process | Group-Object Name | ForEach-Object { [pscustomobject]@{ N = $_.Name; G = ($_.Group | Measure-Object PrivateMemorySize64 -Sum).Sum / 1GB } } |
        Sort-Object G -Descending | Select-Object -First 5 | ForEach-Object { "{0} {1:N1}" -f $_.N, $_.G }) -join ', '
    [pscustomobject]@{ Foreign = $foreign; Cpu = $cpu; FreeGiB = $os.FreePhysicalMemory / 1MB; Top = $top }
}

$first = Sample 0
$started = Get-Date
$proc = Start-Process -FilePath $env:ComSpec -ArgumentList '/d', '/s', '/c', "`"$Run`"" -NoNewWindow -PassThru
$handle = $proc.Handle  # keeps ExitCode readable after exit
$samples = New-Object System.Collections.Generic.List[object]
while (-not $proc.WaitForExit(2000)) { $samples.Add((Sample $proc.Id)) }
$seconds = ((Get-Date) - $started).TotalSeconds
if ($samples.Count -eq 0) { $samples.Add($first) }
$rec = [ordered]@{
    label = $Label
    command = $Run
    seconds = [math]::Round($seconds, 1)
    exit = $proc.ExitCode
    load_start = $first.Foreign
    load_mean = [math]::Round(($samples | Measure-Object Foreign -Average).Average, 1)
    load_max = ($samples | Measure-Object Foreign -Maximum).Maximum
    cpu_mean = [math]::Round(($samples | Measure-Object Cpu -Average).Average, 0)
    free_gib_min = [math]::Round(($samples | Measure-Object FreeGiB -Minimum).Minimum, 1)
    top_memory = ($samples | Sort-Object FreeGiB | Select-Object -First 1).Top
    started = $started.ToString('s')
    cwd = (Get-Location).Path
}
$dir = Split-Path -Parent $Log
if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
Add-Content -LiteralPath $Log -Encoding utf8 -Value ($rec | ConvertTo-Json -Compress)
Write-Output ("time_build: {0} {1:N1} s exit {2}; foreign rustc/link start {3}, mean {4}, max {5}; cpu {6}%; free RAM min {7} GiB ({8})" -f $Label, $seconds, $proc.ExitCode, $rec.load_start, $rec.load_mean, $rec.load_max, $rec.cpu_mean, $rec.free_gib_min, $rec.top_memory)
exit $proc.ExitCode
