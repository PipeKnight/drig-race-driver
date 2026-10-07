# Controlled process-only diagnostic. No saved settings or installation edits.
param([int]$Seconds=30)
$ErrorActionPreference='Stop'
if (Get-Process grid -ErrorAction SilentlyContinue) { throw 'GRID is already running; do not alter an existing session.' }
$started=Get-Date
Start-Process -FilePath 'C:\Program Files (x86)\Steam\steam.exe' -ArgumentList '-applaunch','12750' -WindowStyle Hidden
$seen=@{}
$lastSeen=Get-Date
$transient=@()
$deadline=(Get-Date).AddSeconds($Seconds)
do {
    $procs=@(Get-Process grid -ErrorAction SilentlyContinue)
    if($procs.Count -gt 0) { $lastSeen=Get-Date }
    foreach ($proc in $procs) {
        if (-not $seen.ContainsKey($proc.Id)) {
            try {
                $before=$proc.ProcessorAffinity.ToInt64()
                $proc.ProcessorAffinity=[IntPtr]15
                $seen[$proc.Id]=[pscustomobject]@{Pid=$proc.Id;ObservedUtc=[DateTime]::UtcNow.ToString('o');OriginalAffinity=$before;TestAffinity=$proc.ProcessorAffinity.ToInt64()}
            } catch {
                $errorText="PID $($proc.Id): $($_.Exception.Message)"
                if($transient -notcontains $errorText) { $transient += $errorText }
            }
        }
    }
    Start-Sleep -Milliseconds 50
} while((Get-Date) -lt $deadline -and ($seen.Count -eq 0 -or ((Get-Date)-$lastSeen).TotalSeconds -lt 3))
$remaining=@(Get-Process grid -ErrorAction SilentlyContinue)
foreach($proc in $remaining) { if($seen.ContainsKey($proc.Id)) { $proc.ProcessorAffinity=[IntPtr]$seen[$proc.Id].OriginalAffinity } }
$result=[ordered]@{LaunchUtc=$started.ToUniversalTime().ToString('o');Change='Process affinity limited to logical processors 0-3, then restored for surviving process';Observed=@($seen.Values);TransientErrors=$transient;AliveAtEnd=$remaining.Count -gt 0;WindowTitles=@($remaining | Select-Object -ExpandProperty MainWindowTitle);Events=@(Get-WinEvent -FilterHashtable @{LogName='Application';StartTime=$started;Id=1000,1001,1002} -ErrorAction SilentlyContinue | Where-Object { $_.Message -match 'grid.exe' } | Select-Object TimeCreated,Id,Message)}
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $PSScriptRoot '..\local\phase0\affinity-test.json') -Encoding utf8
[pscustomobject]@{LaunchUtc=$result.LaunchUtc;Observed=$result.Observed;AliveAtEnd=$result.AliveAtEnd;CrashMessages=@($result.Events | Where-Object Id -eq 1000 | Select-Object -ExpandProperty Message)} | ConvertTo-Json -Depth 4
