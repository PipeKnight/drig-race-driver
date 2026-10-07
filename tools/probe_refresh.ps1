# Temporarily change only the saved fullscreen refresh rate; always restore bytes.
param([int]$Rate=60,[int]$Seconds=30,[switch]$Windowed)
$ErrorActionPreference='Stop'
if(Get-Process grid -ErrorAction SilentlyContinue) { throw 'GRID already running' }
$out=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\local\phase0'))
$config=Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Codemasters\GRID\hardwaresettings\hardware_settings_config.xml'
$before=[IO.File]::ReadAllBytes($config)
$testName=if($Windowed) { 'windowed-test' } else { 'refresh-test' }
$backup=Join-Path $out ($testName+'-original.xml')
[IO.File]::WriteAllBytes($backup,$before)
$utf8=[Text.UTF8Encoding]::new($false)
$text=$utf8.GetString($before)
$pattern=if($Windowed) { '(fullscreen=")true(")' } else { '(<refreshRate\s+rate=")\d+("\s*/>)' }
if(([regex]::Matches($text,$pattern)).Count -ne 1) { throw 'Expected exactly one refresh rate' }
$replacement=if($Windowed) { 'false' } else { "$Rate" }
$modified=[regex]::Replace($text,$pattern,{ param($m) $m.Groups[1].Value+$replacement+$m.Groups[2].Value })
$change=if($Windowed) { 'fullscreen true to false only; saved refresh remains 165' } else { "Fullscreen refreshRate only, 165 to $Rate Hz" }
$started=Get-Date
$seen=@{}
$lastSeen=Get-Date
$alive=$false
try {
    [IO.File]::WriteAllText($config,$modified,$utf8)
    Start-Process -FilePath 'C:\Program Files (x86)\Steam\steam.exe' -ArgumentList '-applaunch','12750' -WindowStyle Hidden
    $deadline=(Get-Date).AddSeconds($Seconds)
    do {
        $procs=@(Get-Process grid -ErrorAction SilentlyContinue)
        if($procs.Count -gt 0) { $lastSeen=Get-Date }
        foreach($proc in $procs) { $seen[$proc.Id]=$true }
        Start-Sleep -Milliseconds 100
    } while((Get-Date) -lt $deadline -and ($seen.Count -eq 0 -or ((Get-Date)-$lastSeen).TotalSeconds -lt 3))
    $remaining=@(Get-Process grid -ErrorAction SilentlyContinue)
    $alive=$remaining.Count -gt 0
    # Close only our launched processes before restoring the saved config.
    foreach($proc in $remaining) {
        if($seen.ContainsKey($proc.Id)) {
            $closed=$proc.CloseMainWindow()
            if(-not $proc.WaitForExit(5000)) { throw "GRID PID $($proc.Id) stayed open; settings will still be restored" }
        }
    }
} finally {
    [IO.File]::WriteAllBytes($config,$before)
    $restored=(Get-FileHash -LiteralPath $config -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash
    $result=[ordered]@{LaunchUtc=$started.ToUniversalTime().ToString('o');Change=$change;ObservedPids=@($seen.Keys);AliveAtEnd=$alive;SettingsRestored=$restored;Events=@(Get-WinEvent -FilterHashtable @{LogName='Application';StartTime=$started;Id=1000,1001,1002} -ErrorAction SilentlyContinue | Where-Object { $_.Message -match 'grid.exe' } | Select-Object TimeCreated,Id,Message)}
    $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $out ($testName+'.json')) -Encoding utf8
    [pscustomobject]@{AliveAtEnd=$alive;SettingsRestored=$restored;Pids=@($seen.Keys);CrashMessages=@($result.Events | Where-Object Id -eq 1000 | Select-Object -ExpandProperty Message)} | ConvertTo-Json -Depth 4
}
