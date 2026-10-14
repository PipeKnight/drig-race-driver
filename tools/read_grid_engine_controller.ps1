# Read-only, bounded private GRID audio-object discovery and capture.
# No process writes, suspension, injection, UI actions or unbounded memory dump.
param(
 [Parameter(Mandatory=$true)][string]$SessionJson,
 [Parameter(Mandatory=$true)][string]$Output,
 [long]$ControllerAddress=0,
 [ValidateRange(1,268435456)][long]$ScanLimitBytes=134217728
)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskLocal=(Join-Path $taskRoot 'local')+[IO.Path]::DirectorySeparatorChar
$taskSessionPath=[IO.Path]::GetFullPath((Join-Path $taskRoot $SessionJson))
$taskOutput=[IO.Path]::GetFullPath((Join-Path $taskRoot $Output))
if(!$taskSessionPath.StartsWith($taskLocal,[StringComparison]::OrdinalIgnoreCase) -or !$taskOutput.StartsWith($taskLocal,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $taskOutput)){throw 'Private session and new private output required'}
if($ControllerAddress -and ($ControllerAddress -lt 0x20000 -or $ControllerAddress -gt 0x7ffe0000)){throw 'Controller address outside scope'}
$taskSession=Get-Content -LiteralPath $taskSessionPath -Raw|ConvertFrom-Json
$taskProcess=Get-Process -Id $taskSession.Pid
$taskPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$taskHash='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
if($taskSession.Path -ne $taskPath -or $taskSession.ExeSHA256 -ne $taskHash -or $taskProcess.MainModule.FileName -ne $taskPath -or $taskProcess.MainModule.BaseAddress.ToInt64() -ne 0x400000 -or $taskProcess.StartTime.ToUniversalTime().Ticks -ne ([DateTimeOffset]$taskSession.StartedUtc).UtcDateTime.Ticks){throw 'Private GRID process identity changed'}
if((Get-FileHash -LiteralPath $taskPath).Hash.ToLower() -ne $taskHash){throw 'Private GRID executable changed'}
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class GridEngineReadOnly {
 [StructLayout(LayoutKind.Sequential)] public struct Region {
  public IntPtr BaseAddress, AllocationBase;
  public uint AllocationProtect;
  public UIntPtr RegionSize;
  public uint State, Protect, Type;
 }
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool ReadProcessMemory(IntPtr handle,IntPtr address,byte[] data,UIntPtr length,out UIntPtr read);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern UIntPtr VirtualQueryEx(IntPtr handle,IntPtr address,out Region info,UIntPtr length);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
 public static long[] Find(byte[] bytes,long address,int primaryLength) {
  var matches=new List<long>();
  for(int i=0;i<primaryLength && i+20<=bytes.Length;i+=4) {
   long slot=address+i;
   if(BitConverter.ToUInt32(bytes,i)==slot-0x75e8 && BitConverter.ToUInt32(bytes,i+16)==slot-0x9028) {
    matches.Add(slot-0x118);
    if(matches.Count>64) throw new InvalidOperationException("Candidate limit exceeded");
   }
  }
  return matches.ToArray();
 }
}
'@
$taskHandle=[GridEngineReadOnly]::OpenProcess(0x410,$false,$taskSession.Pid)
if($taskHandle -eq [IntPtr]::Zero){throw 'Read-only process access failed'}
function Read-TaskEngine([long]$Address,[int]$Length){
 if($Address -lt 0x10000 -or $Address+$Length -gt 0x7fff0000 -or $Length -lt 1 -or $Length -gt 65556){throw 'Memory read outside bounded audio scope'}
 $taskBytes=[byte[]]::new($Length);$taskRead=[UIntPtr]::Zero
 if(![GridEngineReadOnly]::ReadProcessMemory($taskHandle,[IntPtr]::new($Address),$taskBytes,[UIntPtr]::new([uint64]$Length),[ref]$taskRead) -or $taskRead.ToUInt64() -ne $Length){return $null}
 return ,$taskBytes
}
function Get-TaskWord([byte[]]$Bytes,[int]$At){[BitConverter]::ToUInt32($Bytes,$At)}
function Get-TaskFloat([byte[]]$Bytes,[int]$At){[BitConverter]::ToSingle($Bytes,$At)}
function Get-TaskBlock([string]$Name,[long]$Address,[int]$Length){
 $taskData=Read-TaskEngine $Address $Length
 if($null -eq $taskData){throw "Unreadable owned audio block: $Name"}
 [pscustomobject]@{Name=$Name;Address=$Address.ToString('x');Length=$Length;Hex=[Convert]::ToHexString($taskData).ToLower()}
}
try {
 $taskSource=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
 if((Get-FileHash -LiteralPath $taskSource).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Analysis reference changed'}
 $taskRaw=[IO.File]::ReadAllBytes($taskSource)
 foreach($taskEntry in @(0x4223d0,0x451770,0x42de60,0xa233d0,0xa25d90,0xa27780)){
  $taskCode=Read-TaskEngine $taskEntry 32;$taskOffset=$taskEntry-0x400000-0x1000+0x400
  if($null -eq $taskCode -or [Convert]::ToHexString($taskCode) -ne [Convert]::ToHexString([byte[]]$taskRaw[$taskOffset..($taskOffset+31)])){throw 'Live audio controller code differs'}
 }
 $taskCandidates=[Collections.Generic.HashSet[long]]::new()
 $taskScanned=0L;$taskRegions=0;$taskAddress=0x10000L;$taskReadFailures=0
 if($ControllerAddress){[void]$taskCandidates.Add($ControllerAddress)}
 while(!$ControllerAddress -and $taskAddress -lt 0x7fff0000 -and $taskScanned -lt $ScanLimitBytes){
  $taskRegion=[GridEngineReadOnly+Region]::new()
  $taskQuery=[GridEngineReadOnly]::VirtualQueryEx($taskHandle,[IntPtr]::new($taskAddress),[ref]$taskRegion,[UIntPtr]::new([uint64][Runtime.InteropServices.Marshal]::SizeOf($taskRegion)))
  if($taskQuery -eq [UIntPtr]::Zero){throw 'Memory metadata query failed'}
  $taskBase=$taskRegion.BaseAddress.ToInt64();$taskSize=[long]$taskRegion.RegionSize.ToUInt64()
  if($taskSize -lt 1 -or $taskBase+$taskSize -le $taskAddress){throw 'Invalid memory region progression'}
  if($taskRegion.State -eq 0x1000 -and $taskRegion.Type -eq 0x20000 -and $taskRegion.Protect -eq 4){
   $taskRegions++
   for($taskAt=0L;$taskAt -lt $taskSize -and $taskScanned -lt $ScanLimitBytes;$taskAt+=65536){
    $taskPrimary=[int][Math]::Min(65536,[Math]::Min($taskSize-$taskAt,$ScanLimitBytes-$taskScanned))
    $taskLength=[int][Math]::Min($taskPrimary+20,$taskSize-$taskAt)
    $taskBytes=Read-TaskEngine ($taskBase+$taskAt) $taskLength
    $taskScanned+=$taskPrimary
    if($null -eq $taskBytes){$taskReadFailures++;continue}
    foreach($taskMatch in [GridEngineReadOnly]::Find($taskBytes,($taskBase+$taskAt),$taskPrimary)){[void]$taskCandidates.Add($taskMatch)}
    if($taskCandidates.Count -gt 64){throw 'Candidate limit exceeded'}
   }
  }
  $taskAddress=$taskBase+$taskSize
 }
 $taskRows=[Collections.Generic.List[object]]::new()
 foreach($taskController in $taskCandidates){
  $taskBegin=[Diagnostics.Stopwatch]::GetTimestamp()
  $taskControl=Read-TaskEngine $taskController 0x340
  if($null -eq $taskControl){continue}
  $taskRootAddress=Get-TaskWord $taskControl 0x128
  if($taskRootAddress+$taskControl.Length -gt 0x7fff0000 -or $taskController -ne $taskRootAddress+0x8f10 -or (Get-TaskWord $taskControl 0x118) -ne $taskRootAddress+0x1a40){continue}
  $taskRootBytes=Read-TaskEngine $taskRootAddress 0xa6b0
  if($null -eq $taskRootBytes){continue}
  $taskComponent=Get-TaskWord $taskControl 0x110
  if($taskComponent -lt 0x10000 -or $taskComponent -ne (Get-TaskWord $taskRootBytes 0x20)){continue}
  $taskComponentBytes=Read-TaskEngine $taskComponent 0x80
  if($null -eq $taskComponentBytes){continue}
  $taskVehicle=Get-TaskWord $taskComponentBytes 0x24
  if($taskVehicle -lt 0x10000){continue}
  $taskVehicleBytes=Read-TaskEngine $taskVehicle 0x2600
  if($null -eq $taskVehicleBytes){continue}
  $taskMaximum=Get-TaskFloat $taskVehicleBytes 0x17a4;$taskIdle=Get-TaskFloat $taskVehicleBytes 0x17a8;$taskSpeed=Get-TaskFloat $taskVehicleBytes 0x1894
  if(![float]::IsFinite($taskMaximum) -or ![float]::IsFinite($taskIdle) -or ![float]::IsFinite($taskSpeed) -or $taskMaximum -le 0 -or $taskIdle -le 0 -or $taskIdle -ge $taskMaximum -or $taskSpeed -lt 0 -or $taskSpeed -gt 2*$taskMaximum){continue}
  $taskThrottleLength=Get-TaskWord $taskControl 0x328;$taskIdleLength=Get-TaskWord $taskControl 0x32c
  if($taskThrottleLength -lt 1 -or $taskThrottleLength -gt 10000000 -or $taskIdleLength -lt 1 -or $taskIdleLength -gt 10000000){continue}
  $taskBlocks=[Collections.Generic.List[object]]::new()
  $taskBlocks.Add([pscustomobject]@{Name='controller';Address=$taskController.ToString('x');Length=0x340;Hex=[Convert]::ToHexString($taskControl).ToLower()})
  $taskBlocks.Add([pscustomobject]@{Name='audio_root';Address=$taskRootAddress.ToString('x');Length=0xa6b0;Hex=[Convert]::ToHexString($taskRootBytes).ToLower()})
  $taskBlocks.Add([pscustomobject]@{Name='vehicle';Address=$taskVehicle.ToString('x');Length=0x2600;Hex=[Convert]::ToHexString($taskVehicleBytes).ToLower()})
  $taskPhysicalWrapper=Get-TaskWord $taskVehicleBytes 0xdb0
  $taskPhysicalWrapperBytes=Read-TaskEngine $taskPhysicalWrapper 0x10
  if($null -eq $taskPhysicalWrapperBytes){throw 'Physical wrapper unreadable'}
  $taskPhysical=Get-TaskWord $taskPhysicalWrapperBytes 8
  $taskBlocks.Add((Get-TaskBlock 'physical_motion' ($taskPhysical+0x210) 32))
  $taskCameraWrapper=Get-TaskWord $taskControl 0x114
  $taskCameraWrapperBytes=Read-TaskEngine $taskCameraWrapper 0x30
  if($null -eq $taskCameraWrapperBytes){throw 'Camera wrapper unreadable'}
  $taskCamera=Get-TaskWord $taskCameraWrapperBytes 0x28
  $taskBlocks.Add((Get-TaskBlock 'camera_mode' ($taskCamera+0xc1c) 4))
  $taskResource=Get-TaskWord $taskRootBytes 0x2c
  $taskBlocks.Add((Get-TaskBlock 'car_audio_resource' $taskResource 0xe0))
  for($taskEmitter=0;$taskEmitter -lt 2;$taskEmitter++){
   $taskEmitterAddress=Get-TaskWord $taskControl (0x12c+$taskEmitter*4)
   $taskEmitterBytes=Read-TaskEngine $taskEmitterAddress 0x1e4
   if($null -eq $taskEmitterBytes){throw 'Emitter unreadable'}
   $taskProfile=Get-TaskWord $taskEmitterBytes 0x1e0
   $taskBlocks.Add((Get-TaskBlock ('emitter_profile_'+$taskEmitter) $taskProfile 0x40))
  }
  for($taskVoice=0;$taskVoice -lt 12;$taskVoice++){
   $taskVoiceAddress=Get-TaskWord $taskControl (0x1ec+$taskVoice*4)
   if($taskVoiceAddress){
    $taskVoiceBlock=Get-TaskBlock ('voice_'+$taskVoice) $taskVoiceAddress 0xe0
    $taskBlocks.Add($taskVoiceBlock)
    $taskVoiceBytes=[Convert]::FromHexString($taskVoiceBlock.Hex)
    $taskSlot=Get-TaskWord $taskVoiceBytes 0x68
    $taskSlotLimit=Get-TaskWord (Read-TaskEngine 0xebf434 4) 0
    if($taskSlot -ne 0xa000001){
     if($taskSlot -ge $taskSlotLimit -or $taskSlotLimit -gt 4096){throw 'Backend slot outside observed bounded table'}
     $taskBlocks.Add((Get-TaskBlock ('backend_voice_'+$taskVoice) (0xe9c7f0+$taskSlot*0x370+0x300) 0x70))
    }
   }
  }
  $taskBlocks.Add((Get-TaskBlock 'backend_globals' 0xebf420 64))
  $taskBlocks.Add((Get-TaskBlock 'group_treble' 0xebee28 36))
  $taskBlocks.Add((Get-TaskBlock 'group_filters' 0xebedf4 36))
  $taskBlocks.Add((Get-TaskBlock 'filter_config' 0xe50900 16))
  $taskRepeat=Read-TaskEngine $taskController 0x340
  $taskRows.Add([pscustomobject]@{Controller=$taskController.ToString('x');Root=$taskRootAddress.ToString('x');Vehicle=$taskVehicle.ToString('x');Player=((Get-TaskWord $taskComponentBytes 0x6c) -eq 0);EngineSpeed=$taskSpeed;Maximum=$taskMaximum;Idle=$taskIdle;ThrottleLength=$taskThrottleLength;IdleLength=$taskIdleLength;QpcBegin=$taskBegin;QpcEnd=[Diagnostics.Stopwatch]::GetTimestamp();ControllerRepeatExact=($null -ne $taskRepeat -and [Convert]::ToHexString($taskControl) -eq [Convert]::ToHexString($taskRepeat));Blocks=$taskBlocks})
 }
 if($ControllerAddress -and $taskRows.Count -ne 1){throw 'Explicit controller failed ownership checks'}
 $taskRecord=[pscustomobject]@{CapturedUtc=[DateTime]::UtcNow.ToString('o');Session=$taskSession;Scope='Bounded read-only audio ownership capture; sequential reads, not an atomic or suspended game snapshot';QpcFrequency=[Diagnostics.Stopwatch]::Frequency;CodePrefixesExact=$true;ScannedBytes=$taskScanned;Regions=$taskRegions;ReadFailures=$taskReadFailures;ScanComplete=($ControllerAddress -eq 0 -and $taskAddress -ge 0x7fff0000);CandidateCount=$taskCandidates.Count;Controllers=$taskRows}
 $taskStream=[IO.File]::Open($taskOutput,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskData=[Text.Encoding]::UTF8.GetBytes(($taskRecord|ConvertTo-Json -Depth 9));$taskStream.Write($taskData,0,$taskData.Length)}finally{$taskStream.Dispose()}
 $taskRows|Select-Object Controller,Player,EngineSpeed,Maximum,Idle,ThrottleLength,IdleLength,ControllerRepeatExact|Format-Table
 "scanned_bytes=$taskScanned candidates=$($taskCandidates.Count) owned=$($taskRows.Count)"
}finally{[void][GridEngineReadOnly]::CloseHandle($taskHandle)}
