# One guarded recovery of an observed suspended thread in the owned private game.
param([Parameter(Mandatory=$true)][string]$SessionJson,
 [Parameter(Mandatory=$true)][string]$ThreadEvidence,
 [Parameter(Mandatory=$true)][int]$ThreadId)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
function Get-TaskPrivatePath([string]$Value){
 $taskValue=[IO.Path]::GetFullPath((Join-Path $taskRoot $Value))
 if(!$taskValue.StartsWith((Join-Path $taskRoot 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Private evidence only'}
 return $taskValue
}
$taskSession=Get-Content -LiteralPath (Get-TaskPrivatePath $SessionJson) -Raw | ConvertFrom-Json
$taskEvidence=Get-Content -LiteralPath (Get-TaskPrivatePath $ThreadEvidence) -Raw | ConvertFrom-Json
$taskGame='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$taskSha='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
$taskProcess=Get-Process -Id $taskSession.Pid
if($taskSession.Path -ne $taskGame -or $taskSession.ExeSHA256 -ne $taskSha -or $taskProcess.MainModule.FileName -ne $taskGame -or $taskProcess.StartTime.ToUniversalTime().Ticks -ne ([DateTimeOffset]$taskSession.StartedUtc).UtcDateTime.Ticks -or (Get-FileHash -LiteralPath $taskGame).Hash.ToLower() -ne $taskSha){throw 'Reference identity changed'}
if($taskEvidence.pid -ne $taskProcess.Id -or @($taskEvidence.threads | Where-Object { $_.id -eq $ThreadId -and $_.state -eq 'Wait' -and $_.wait -eq 'Suspended' }).Count -ne 1){throw 'No unique recorded suspended thread'}
$taskThread=@($taskProcess.Threads | Where-Object Id -eq $ThreadId)
if($taskThread.Count -ne 1 -or $taskThread[0].ThreadState.ToString() -ne 'Wait' -or $taskThread[0].WaitReason.ToString() -ne 'Suspended'){throw 'Thread no longer observed suspended'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GridRecoverThread {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenThread(uint access,bool inherit,uint id);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern uint GetProcessIdOfThread(IntPtr handle);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern uint ResumeThread(IntPtr handle);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
'@
$taskHandle=[GridRecoverThread]::OpenThread(0x802,$false,[uint32]$ThreadId)
if($taskHandle -eq [IntPtr]::Zero){throw 'Thread open failed'}
try {
 if([GridRecoverThread]::GetProcessIdOfThread($taskHandle) -ne $taskProcess.Id){throw 'Thread owner changed'}
 $taskPrevious=[GridRecoverThread]::ResumeThread($taskHandle)
 $taskRecord=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');session=$taskSession;thread_id=$ThreadId;operation='one ResumeThread call';previous_suspend_count=$taskPrevious;success=$taskPrevious -ne [uint32]::MaxValue;last_error=if($taskPrevious -eq [uint32]::MaxValue){[Runtime.InteropServices.Marshal]::GetLastWin32Error()}else{$null};input_sent=$false}
 $taskOut=Get-TaskPrivatePath ('local/behavior/thread-recovery-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')+'.json')
 $taskFile=[IO.File]::Open($taskOut,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write)
 try{$taskWriter=[IO.StreamWriter]::new($taskFile);$taskWriter.Write(($taskRecord | ConvertTo-Json -Depth 5));$taskWriter.Flush()}finally{$taskFile.Dispose()}
 $taskRecord | ConvertTo-Json -Depth 5
 Write-Output "PRIVATE_OUTPUT=$taskOut"
 if($taskPrevious -eq [uint32]::MaxValue){throw 'Resume failed; evidence retained'}
}finally{[void][GridRecoverThread]::CloseHandle($taskHandle)}
