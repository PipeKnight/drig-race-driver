param(
 [Parameter(Mandatory=$true)][string]$SessionJson,
 [Parameter(Mandatory=$true)][string]$EvidenceDirectory,
 [ValidateRange(15,120)][int]$Seconds=90
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$out=[IO.Path]::GetFullPath($EvidenceDirectory)
if(-not $out.StartsWith((Join-Path $root 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Capture must remain in project local/' }
if(Test-Path -LiteralPath $out) { throw 'Evidence directory already exists' }
$session=Get-Content -LiteralPath $SessionJson -Raw | ConvertFrom-Json
$savedStart=[DateTimeOffset]$session.StartedUtc
$exe='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$game=Get-Process -Id $session.Pid -ErrorAction Stop
if($game.ProcessName -ne 'grid' -or $game.Path -ne $exe -or [Math]::Abs(($game.StartTime.ToUniversalTime()-$savedStart.UtcDateTime).TotalSeconds) -gt 2) { throw 'Reference process identity mismatch' }
$hash=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLower()
if($hash -ne $session.ExeSHA256) { throw 'Reference executable hash mismatch' }
if(Get-Process -Name Procmon,Procmon64,Procmon64a -ErrorAction SilentlyContinue) { throw 'An existing Process Monitor instance must be handled before capture' }
$disk=Get-PSDrive -Name ([IO.Path]::GetPathRoot($out).Substring(0,1))
if($disk.Free -lt 5GB) { throw 'Insufficient free disk space for bounded private trace' }
$tracer=Join-Path $root 'local\tools\procmon\Procmon64.exe'
if(-not (Test-Path -LiteralPath $tracer)) { throw 'Installed Process Monitor is missing' }
New-Item -ItemType Directory -Path $out | Out-Null
$settings=Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Codemasters\GRID\hardwaresettings\hardware_settings_config.xml'
Copy-Item -LiteralPath $settings -Destination (Join-Path $out 'hardware-settings-before.xml')
$pml=Join-Path $out 'race-load.private.pml'
$arguments='/AcceptEula /Quiet /Minimized /NoFilter /BackingFile "'+$pml+'" /Runtime '+$Seconds
$started=[DateTime]::UtcNow
$proc=Start-Process -FilePath $tracer -ArgumentList $arguments -WindowStyle Hidden -PassThru
$record=[PSCustomObject]@{
 CaptureRequestedUtc=$started.ToString('o');Seconds=$Seconds;
 ScheduledEndUtc=$started.AddSeconds($Seconds).ToString('o');
 LauncherPid=$proc.Id;GamePid=$game.Id;GameStartedUtc=$game.StartTime.ToUniversalTime().ToString('o');
 GameExeSHA256=$hash;TracerVersion=(Get-Item -LiteralPath $tracer).VersionInfo.FileVersion;
 TracerSHA256=(Get-FileHash -LiteralPath $tracer -Algorithm SHA256).Hash.ToLower();
 BackingFile=$pml;Arguments=$arguments;DiskFreeBefore=$disk.Free;
 Caveat='Bounded all-process capture retained privately; export must be filtered to verified GRID PID. A fresh scene load must happen during the capture window. File creation is not proof of a complete trace.'
}
$record | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $out 'capture-request.json') -Encoding utf8
$record | ConvertTo-Json
