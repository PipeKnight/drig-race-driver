# Bounded read-only x86 process-context diagnostic; offsets are runtime hypotheses.
param([Parameter(Mandatory=$true)][string]$SessionJson)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskSessionPath=[IO.Path]::GetFullPath((Join-Path $taskRoot $SessionJson))
if(!$taskSessionPath.StartsWith((Join-Path $taskRoot 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Private session required'}
$taskSession=Get-Content -LiteralPath $taskSessionPath -Raw | ConvertFrom-Json
$taskPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$taskHash='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
$taskProcess=Get-Process -Id $taskSession.Pid
if($taskSession.Path -ne $taskPath -or $taskSession.ExeSHA256 -ne $taskHash -or $taskProcess.Path -ne $taskPath -or $taskProcess.StartTime.ToUniversalTime().Ticks -ne ([DateTimeOffset]$taskSession.StartedUtc).UtcDateTime.Ticks){throw 'Reference identity changed'}
if((Get-FileHash -LiteralPath $taskPath).Hash.ToLower() -ne $taskHash){throw 'Executable changed'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GridReadProcessContext {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool ReadProcessMemory(IntPtr handle,IntPtr address,byte[] data,UIntPtr length,out UIntPtr read);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
 [DllImport("ntdll.dll")] public static extern int NtQueryInformationProcess(IntPtr handle,int kind,out IntPtr information,uint length,out uint returned);
}
'@
$taskHandle=[GridReadProcessContext]::OpenProcess(0x410,$false,$taskProcess.Id)
if($taskHandle -eq [IntPtr]::Zero){throw 'Read-only process access failed'}
function Read-TaskContext([long]$Address,[int]$Length){
 if($Address -lt 0x10000 -or $Address -gt 0x7fff0000 -or $Length -lt 1 -or $Length -gt 4096){throw 'Read outside scope'}
 $taskBytes=[byte[]]::new($Length);$taskRead=[UIntPtr]::Zero
 if(![GridReadProcessContext]::ReadProcessMemory($taskHandle,[IntPtr]::new($Address),$taskBytes,[UIntPtr]::new([uint64]$Length),[ref]$taskRead) -or $taskRead.ToUInt64() -ne $Length){throw 'Context read failed'}
 return ,$taskBytes
}
function Read-TaskString([byte[]]$Header,[int]$Offset){
 $taskLength=[BitConverter]::ToUInt16($Header,$Offset);$taskMaximum=[BitConverter]::ToUInt16($Header,$Offset+2);$taskPointer=[BitConverter]::ToUInt32($Header,$Offset+4)
 if($taskLength -lt 2 -or $taskLength -gt 2048 -or $taskLength%2 -or $taskLength -gt $taskMaximum){throw 'Unexpected Unicode string bounds'}
 return [Text.Encoding]::Unicode.GetString((Read-TaskContext $taskPointer $taskLength))
}
try {
 $taskPeb=[IntPtr]::Zero;$taskReturned=0u
 $taskStatus=[GridReadProcessContext]::NtQueryInformationProcess($taskHandle,26,[ref]$taskPeb,[uint32][IntPtr]::Size,[ref]$taskReturned)
 if($taskStatus -ne 0 -or $taskPeb -eq [IntPtr]::Zero -or $taskReturned -ne [IntPtr]::Size){throw 'WOW64 context query failed'}
 $taskPebBytes=Read-TaskContext $taskPeb.ToInt64() 0x20
 if([BitConverter]::ToUInt32($taskPebBytes,8) -ne 0x400000){throw 'Unverified x86 PEB layout/base'}
 $taskParameters=[BitConverter]::ToUInt32($taskPebBytes,0x10)
 $taskParameterBytes=Read-TaskContext $taskParameters 0x48
 $taskImage=Read-TaskString $taskParameterBytes 0x38
 if($taskImage -ne $taskPath){throw 'Process parameter image does not match known module'}
 $taskCurrent=Read-TaskString $taskParameterBytes 0x24
 $taskCommand=Read-TaskString $taskParameterBytes 0x40
 $taskRecord=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');session=$taskSession;peb32=$taskPeb.ToInt64().ToString('x');image_path=$taskImage;command_line=$taskCommand;current_directory_candidate=$taskCurrent;read_only=$true;input_sent=$false;layout_scope='x86 image/base cross-check; current-directory offset is an internal-layout hypothesis'}
 $taskOut=Join-Path $taskRoot ('local/behavior/process-context-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')+'.json')
 $taskFile=[IO.File]::Open($taskOut,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskWriter=[IO.StreamWriter]::new($taskFile);$taskWriter.Write(($taskRecord | ConvertTo-Json -Depth 5));$taskWriter.Flush()}finally{$taskFile.Dispose()}
 [pscustomobject]@{output=$taskOut;current_directory_candidate=$taskCurrent;image_path=$taskImage;read_only=$true} | ConvertTo-Json
}finally{[void][GridReadProcessContext]::CloseHandle($taskHandle)}
