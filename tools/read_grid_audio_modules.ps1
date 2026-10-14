# Resolve actual mapped files, including WOW64 redirection, without touching the game.
param([string]$SessionJson,[string]$Output)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskLocal=(Join-Path $taskRoot 'local')+[IO.Path]::DirectorySeparatorChar
$taskSessionPath=[IO.Path]::GetFullPath((Join-Path $taskRoot $SessionJson))
$taskOutput=[IO.Path]::GetFullPath((Join-Path $taskRoot $Output))
if(!$taskSessionPath.StartsWith($taskLocal,[StringComparison]::OrdinalIgnoreCase) -or !$taskOutput.StartsWith($taskLocal,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $taskOutput)){throw 'Private session and new private output required'}
$taskSession=Get-Content -LiteralPath $taskSessionPath -Raw|ConvertFrom-Json
$taskProcess=Get-Process -Id $taskSession.Pid
$taskPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
if($taskSession.Path -ne $taskPath -or $taskProcess.MainModule.FileName -ne $taskPath -or $taskProcess.StartTime.ToUniversalTime().Ticks -ne ([DateTimeOffset]$taskSession.StartedUtc).UtcDateTime.Ticks){throw 'Reference process changed'}
if((Get-FileHash -LiteralPath $taskPath).Hash.ToLower() -ne 'b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'){throw 'Private reference executable changed'}
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class GridAudioMappedFiles {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern uint K32GetMappedFileNameW(IntPtr process,IntPtr address,StringBuilder path,uint count);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern uint QueryDosDeviceW(string drive,StringBuilder path,uint count);
}
'@
$taskHandle=[GridAudioMappedFiles]::OpenProcess(0x410,$false,$taskSession.Pid)
if($taskHandle -eq [IntPtr]::Zero){throw 'Read-only process access failed'}
try {
 $taskDrives=@{}
 foreach($taskDrive in [IO.DriveInfo]::GetDrives()){
  $taskName=$taskDrive.Name.Substring(0,2);$taskBuffer=[Text.StringBuilder]::new(32768)
  if([GridAudioMappedFiles]::QueryDosDeviceW($taskName,$taskBuffer,32768)){$taskDrives[$taskBuffer.ToString()]=$taskName}
 }
 $taskRows=@(foreach($taskModule in $taskProcess.Modules){
  if($taskModule.ModuleName -notmatch '^(DSOUND|OpenAL32|wrap_oal|binkw32)\.dll$'){continue}
  $taskBuffer=[Text.StringBuilder]::new(32768)
  if(![GridAudioMappedFiles]::K32GetMappedFileNameW($taskHandle,$taskModule.BaseAddress,$taskBuffer,32768)){throw 'Mapped filename query failed'}
  $taskDevice=$taskBuffer.ToString();$taskResolved=$null
  foreach($taskPrefix in $taskDrives.Keys){if($taskDevice.StartsWith($taskPrefix+'\',[StringComparison]::OrdinalIgnoreCase)){$taskResolved=$taskDrives[$taskPrefix]+$taskDevice.Substring($taskPrefix.Length);break}}
  if(!$taskResolved){throw 'Mapped filename has no DOS drive mapping'}
  $taskInfo=Get-Item -LiteralPath $taskResolved
  $taskBytes=[IO.File]::ReadAllBytes($taskResolved);$taskPe=[BitConverter]::ToInt32($taskBytes,0x3c)
  $taskMachine=[BitConverter]::ToUInt16($taskBytes,$taskPe+4)
  if($taskMachine -ne 0x14c){throw '32-bit GRID has unexpected audio module architecture'}
  [pscustomobject]@{Name=$taskModule.ModuleName;Base=$taskModule.BaseAddress.ToInt64().ToString('x');ReportedPath=$taskModule.FileName;MappedDevicePath=$taskDevice;ActualFilePath=$taskResolved;FileVersion=$taskInfo.VersionInfo.FileVersion;Bytes=$taskInfo.Length;Machine=$taskMachine.ToString('x');SHA256=(Get-FileHash -LiteralPath $taskResolved).Hash.ToLower()}
 })
 [pscustomobject]@{Scope='Read-only backing-file metadata; loaded modules do not establish which audio backend is active';CapturedUtc=[DateTime]::UtcNow.ToString('o');Session=$taskSession;Modules=$taskRows}|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $taskOutput -Encoding utf8
 $taskRows|Format-Table Name,FileVersion,Bytes,ActualFilePath
}finally{[void][GridAudioMappedFiles]::CloseHandle($taskHandle)}
