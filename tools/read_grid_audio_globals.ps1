# Bounded read-only parameter-descriptor capture; no game input or code changes.
param([string]$SessionJson,[string]$Output)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskPrivate=(Join-Path $taskRoot 'local')+[IO.Path]::DirectorySeparatorChar
$taskSessionPath=[IO.Path]::GetFullPath((Join-Path $taskRoot $SessionJson))
$taskOutput=[IO.Path]::GetFullPath((Join-Path $taskRoot $Output))
if(!$taskSessionPath.StartsWith($taskPrivate,[StringComparison]::OrdinalIgnoreCase) -or !$taskOutput.StartsWith($taskPrivate,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $taskOutput)){throw 'Private session and new private output required'}
$taskSession=Get-Content -LiteralPath $taskSessionPath -Raw|ConvertFrom-Json
$taskProcess=Get-Process -Id $taskSession.Pid
$taskExpectedPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
if($taskSession.Path -ne $taskExpectedPath -or $taskProcess.MainModule.FileName -ne $taskExpectedPath -or $taskProcess.MainModule.BaseAddress.ToInt64() -ne 0x400000 -or $taskProcess.StartTime.ToUniversalTime().Ticks -ne ([DateTimeOffset]$taskSession.StartedUtc).UtcDateTime.Ticks){throw 'Private GRID process changed'}
if((Get-FileHash -LiteralPath $taskExpectedPath).Hash.ToLower() -ne 'b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'){throw 'Private GRID executable changed'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GridAudioDescriptorRead {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool ReadProcessMemory(IntPtr process,IntPtr address,byte[] bytes,UIntPtr size,out UIntPtr read);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
'@
$taskHandle=[GridAudioDescriptorRead]::OpenProcess(0x410,$false,$taskSession.Pid)
if($taskHandle -eq [IntPtr]::Zero){throw 'Read-only process access failed'}
function Read-TaskAudio([long]$Address,[int]$Length){
 if($Address -lt 0x10000 -or $Address -gt 0x7fff0000 -or $Length -lt 1 -or $Length -gt 128){throw 'Audio descriptor read outside scope'}
 $taskBytes=[byte[]]::new($Length);$taskRead=[UIntPtr]::Zero
 if(![GridAudioDescriptorRead]::ReadProcessMemory($taskHandle,[IntPtr]::new($Address),$taskBytes,[UIntPtr]::new([uint64]$Length),[ref]$taskRead) -or $taskRead.ToUInt64() -ne $Length){throw 'Audio descriptor memory read failed'}
 return ,$taskBytes
}
try {
 $taskSource=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
 if((Get-FileHash -LiteralPath $taskSource).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Analysis reference changed'}
 $taskRaw=[IO.File]::ReadAllBytes($taskSource)
 foreach($taskEntry in @(0x46a7d0,0x46ab80)){
  $taskCode=Read-TaskAudio $taskEntry 32;$taskOffset=$taskEntry-0x400000-0x1000+0x400
  if([Convert]::ToHexString($taskCode) -ne [Convert]::ToHexString([byte[]]$taskRaw[$taskOffset..($taskOffset+31)])){throw 'Live audio parameter code differs'}
 }
 $taskRows=@(foreach($taskGlobal in @(0xe9c180,0xe9c184,0xe9c18c,0xe9c190,0xe9c19c,0xe9c1a0,0xe9c1a4,0xe9c1a8,0xe9c1b0)){
  $taskPointer=[BitConverter]::ToUInt32((Read-TaskAudio $taskGlobal 4),0)
  if(!$taskPointer){[pscustomobject]@{Global=$taskGlobal.ToString('x');Descriptor='0';Initialized=$false};continue}
  $taskBytes=Read-TaskAudio $taskPointer 28;$taskRepeat=Read-TaskAudio $taskPointer 28
  if([Convert]::ToHexString($taskBytes) -ne [Convert]::ToHexString($taskRepeat)){throw 'Audio descriptor changed during capture'}
  [pscustomobject]@{Global=$taskGlobal.ToString('x');Descriptor=$taskPointer.ToString('x');Initialized=$true;Hex=[Convert]::ToHexString($taskBytes).ToLower();NameOrId=([BitConverter]::ToUInt32($taskBytes,0)).ToString('x');OffsetOrCallback=([BitConverter]::ToUInt32($taskBytes,4)).ToString('x');Flags=[BitConverter]::ToUInt32($taskBytes,20);Kind=[BitConverter]::ToUInt32($taskBytes,24);RepeatedHeaderExact=$true}
 })
 $taskRecord=[pscustomobject]@{CapturedUtc=[DateTime]::UtcNow.ToString('o');Session=$taskSession;CodePrefixesExact=$true;Scope='Fixed read-only audio parameter descriptors; no input, injection or whole mixer claim';Descriptors=$taskRows}
 $taskStream=[IO.File]::Open($taskOutput,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskData=[Text.Encoding]::UTF8.GetBytes(($taskRecord|ConvertTo-Json -Depth 6));$taskStream.Write($taskData,0,$taskData.Length)}finally{$taskStream.Dispose()}
 $taskRows|Format-Table Global,Initialized,OffsetOrCallback,Flags,Kind
}finally{[void][GridAudioDescriptorRead]::CloseHandle($taskHandle)}
