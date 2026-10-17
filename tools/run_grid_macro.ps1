param(
 [Parameter(Mandatory=$true)][ValidateSet('inspect','serve','menu_down','menu_up','confirm','pause','throttle','brake','left','right','throttle_left','throttle_right','brake_test')][string]$Action,
 [ValidateRange(30,2000)][int]$HoldMs=150,
 [ValidateSet('Event','Input')][string]$Mode='Event',
 [string]$SessionJson='local\autosave\persistence-restart\session.json',
 [string]$EvidenceDirectory='local\macros'
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$out=[IO.Path]::GetFullPath((Join-Path $root $EvidenceDirectory))
if(-not $out.StartsWith((Join-Path $root 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Macro evidence must remain in local/' }
$session=Get-Content -LiteralPath (Join-Path $root $SessionJson) -Raw | ConvertFrom-Json
$savedStart=[DateTimeOffset]$session.StartedUtc
$exe='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$game=Get-Process -Id $session.Pid -ErrorAction Stop
if($game.ProcessName -ne 'grid' -or $game.Path -ne $exe -or [Math]::Abs(($game.StartTime.ToUniversalTime()-$savedStart.UtcDateTime).TotalSeconds) -gt 2) { throw 'Reference process identity mismatch' }
if((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLower() -ne $session.ExeSHA256) { throw 'Reference executable hash mismatch' }
$ahk=Join-Path $root 'local\tools\autohotkey\AutoHotkey64.exe'
$source=Join-Path $PSScriptRoot 'grid_macros.ahk'
New-Item -ItemType Directory -Path $out -Force | Out-Null
$stamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$log=Join-Path $out ($stamp+'-'+$Action+'.tsv')
$argsList=@('/ErrorStdOut=UTF-8','/CP65001',$source,[string]$game.Id,$Action,[string]$HoldMs,$Mode,$log,$exe)
if($Action -eq 'serve') {
 $existing=Get-CimInstance Win32_Process -Filter "Name='AutoHotkey64.exe'" | Where-Object { $_.ExecutablePath -eq $ahk -and $_.CommandLine -like '*grid_macros.ahk*' -and $_.CommandLine -match '\bserve\b' }
 if($existing) { throw 'GRID macro server already running; do not duplicate hotkey handlers' }
 $quoted=($argsList | ForEach-Object { '"'+$_+'"' }) -join ' '
 $stdout=Join-Path $out ($stamp+'-serve-stdout.txt')
 $stderr=Join-Path $out ($stamp+'-serve-stderr.txt')
 $macro=Start-Process -FilePath $ahk -ArgumentList $quoted -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
 $record=[PSCustomObject]@{Pid=$macro.Id;StartedUtc=$macro.StartTime.ToUniversalTime().ToString('o');GamePid=$game.Id;GameStartedUtc=$game.StartTime.ToUniversalTime().ToString('o');Log=$log;Executable=$ahk;ScriptSHA256=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLower()}
 $record | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out ($stamp+'-server.json')) -Encoding utf8
 $record | ConvertTo-Json
} else {
 $quoted=($argsList | ForEach-Object { '"'+$_+'"' }) -join ' '
 $stdout=Join-Path $out ($stamp+'-'+$Action+'-stdout.txt')
 $stderr=Join-Path $out ($stamp+'-'+$Action+'-stderr.txt')
 $macro=Start-Process -FilePath $ahk -ArgumentList $quoted -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
 if(-not $macro.WaitForExit($(if($Action -eq 'brake_test'){20000}else{10000}))) { throw 'Macro did not finish in time; do not send further input before investigating' }
 $exitCode=$macro.ExitCode
 Get-Content -LiteralPath $stdout,$stderr
 if($exitCode -ne 0) { throw "Macro failed with exit code $exitCode; inspect $log" }
}
