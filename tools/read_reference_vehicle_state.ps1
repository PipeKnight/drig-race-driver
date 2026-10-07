# Bounded read-only indexed render-component survey. This is not the body view.
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskProcess=Get-Process -Id 19680
$taskPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$taskExpected=[DateTime]::Parse('2026-10-06T01:04:50.0562006Z',[Globalization.CultureInfo]::InvariantCulture,[Globalization.DateTimeStyles]::RoundtripKind)
if($taskProcess.StartTime.ToUniversalTime().Ticks -ne $taskExpected.ToUniversalTime().Ticks -or $taskProcess.MainModule.FileName -ne $taskPath){throw 'Reference process identity changed'}
if((Get-FileHash -LiteralPath $taskPath).Hash.ToLower() -ne 'b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'){throw 'Reference executable identity changed'}
if($taskProcess.MainModule.BaseAddress.ToInt64() -ne 0x400000){throw 'Unverified module relocation'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GridReadOnlyVehicleState {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool ReadProcessMemory(IntPtr handle,IntPtr address,byte[] data,UIntPtr length,out UIntPtr read);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
'@
$taskHandle=[GridReadOnlyVehicleState]::OpenProcess(0x410,$false,19680)
if($taskHandle -eq [IntPtr]::Zero){throw 'Read-only process access failed'}
$taskBlocks=[Collections.Generic.List[object]]::new()
function Read-TaskBlock([string]$Name,[long]$Address,[int]$Length){
 if($Address -lt 0x10000 -or $Address -gt 0x7fff0000 -or $Length -lt 1 -or $Length -gt 4096){throw 'Out-of-scope memory request'}
 $taskBytes=[byte[]]::new($Length);$taskRead=[UIntPtr]::Zero
 $taskSuccess=[GridReadOnlyVehicleState]::ReadProcessMemory($taskHandle,[IntPtr]::new($Address),$taskBytes,[UIntPtr]::new([uint64]$Length),[ref]$taskRead)
 $taskBlocks.Add([pscustomobject]@{name=$Name;address=$Address.ToString('x');length=$Length;success=$taskSuccess;read=$taskRead.ToUInt64();hex=if($taskSuccess){[Convert]::ToHexString($taskBytes).ToLower()}else{$null}})
 if(!$taskSuccess -or $taskRead.ToUInt64() -ne $Length){return $null}
 return ,$taskBytes
}
try {
 $taskSource=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
 if((Get-FileHash -LiteralPath $taskSource).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Private analysis reference changed'}
 $taskCode=Read-TaskBlock 'vehicle_count_code' 0x9ad030 32
 $taskRaw=[IO.File]::ReadAllBytes($taskSource)
 $taskOffset=0x9ad030-0x400000-0x1000+0x400
 if($null -eq $taskCode -or [Convert]::ToHexString($taskCode) -ne [Convert]::ToHexString([byte[]]$taskRaw[$taskOffset..($taskOffset+31)])){throw 'Live count routine differs'}
 $taskPhysicsCode=Read-TaskBlock 'physics_vehicle_lookup_code' 0x99c050 32
 $taskPhysicsOffset=0x99c050-0x400000-0x1000+0x400
 if($null -eq $taskPhysicsCode -or [Convert]::ToHexString($taskPhysicsCode) -ne [Convert]::ToHexString([byte[]]$taskRaw[$taskPhysicsOffset..($taskPhysicsOffset+31)])){throw 'Live physics lookup routine differs'}
 # Disassembly confirms ECX=0xe59148 at the named plugin lookup call.
 $taskNames=Read-TaskBlock 'plugin_registry_names' 0xe59148 4096
 $taskPointers=Read-TaskBlock 'plugin_registry_pointers' 0xe5a148 512
 $taskCountBytes=Read-TaskBlock 'plugin_registry_count' 0xe5a348 4
 if($null -eq $taskNames -or $null -eq $taskPointers -or $null -eq $taskCountBytes){throw 'Registry read failed'}
 $taskCount=[BitConverter]::ToUInt32($taskCountBytes,0)
 if($taskCount -gt 128){throw 'Registry count outside observed storage'}
 $taskManager=0
 for($taskIndex=0;$taskIndex -lt $taskCount;$taskIndex++){
  $taskName=[Text.Encoding]::ASCII.GetString($taskNames,$taskIndex*32,32).Split([char]0)[0]
  if($taskName -eq 'VehicleManagerPlugin'){$taskManager=[BitConverter]::ToUInt32($taskPointers,$taskIndex*4);break}
 }
 if($taskManager -lt 0x10000 -or $taskManager -gt 0x7ffe0000){throw 'Named vehicle service absent'}
 $taskManagerBytes=Read-TaskBlock 'named_vehicle_manager_prefix' $taskManager 0x100
 if($null -eq $taskManagerBytes){throw 'Vehicle manager read failed'}
 # Verified plugin creation 8f2e70 stores the indexed manager at +7c.
 # 64b100 reads its count at +598; 99c050 indexes its 20-slot array at +12a0.
 $taskPlugin=$taskManager
 $taskManager=[BitConverter]::ToUInt32($taskManagerBytes,0x7c)
 if($taskManager -lt 0x10000 -or $taskManager -gt 0x7ffe0000){throw 'Physics manager pointer outside bounds'}
 $taskPhysicsCountBytes=Read-TaskBlock 'physics_vehicle_count' ($taskManager+0x598) 4
 if($null -eq $taskPhysicsCountBytes){throw 'Physics count read failed'}
 $taskVehicleCount=[BitConverter]::ToUInt32($taskPhysicsCountBytes,0)
 if($taskVehicleCount -gt 20){throw 'Physics count exceeds indexed storage'}
 $taskVehicles=[Collections.Generic.List[object]]::new()
 if($taskVehicleCount -gt 0){
  $taskVehiclePointers=Read-TaskBlock 'physics_vehicle_pointer_slots' ($taskManager+0x12a0) 80
  if($null -eq $taskVehiclePointers){throw 'Vehicle vector read failed'}
  for($taskIndex=0;$taskIndex -lt $taskVehicleCount;$taskIndex++){
   $taskVehicle=[BitConverter]::ToUInt32($taskVehiclePointers,$taskIndex*4)
   if($taskVehicle -lt 0x10000 -or $taskVehicle -gt 0x7ffe0000){throw 'Vehicle pointer outside bounds'}
   $taskHeader=Read-TaskBlock "vehicle_$taskIndex-header" $taskVehicle 0x100
   if($null -eq $taskHeader -or [BitConverter]::ToUInt32($taskHeader,0) -ne 0xd0dea0){throw 'Indexed component class differs'}
   # ba0200's +18 constructor argument is a database component link.
   # The rejected general-car interpretation is preserved in historical captures.
   # Use find_reference_vehicle_manager.ps1 for the verified general-car/body view.
   $taskComponentLink=[BitConverter]::ToUInt32($taskHeader,0x18)
   $taskVehicles.Add([pscustomobject]@{index=$taskIndex;component=$taskVehicle.ToString('x');database_component_link=$taskComponentLink.ToString('x')})
  }
 }
 $taskRecord=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');pid=19680;created_utc=$taskProcess.StartTime.ToUniversalTime().ToString('o');path=$taskPath;live_count_code_exact=$true;live_physics_lookup_code_exact=$true;registry_count=$taskCount;plugin=$taskPlugin.ToString('x');indexed_manager=$taskManager.ToString('x');vehicle_count=$taskVehicleCount;vehicles=$taskVehicles;blocks=$taskBlocks;read_only=$true;input_sent=$false;process_suspended=$false}
 $taskOut=Join-Path $taskRoot ("local/behavior/runtime-vehicles-"+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')+'.json')
 $taskFile=[IO.File]::Open($taskOut,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskWriter=[IO.StreamWriter]::new($taskFile);$taskWriter.Write(($taskRecord | ConvertTo-Json -Depth 8));$taskWriter.Flush()}finally{$taskFile.Dispose()}
 [pscustomobject]@{output=$taskOut;indexed_manager=$taskRecord.indexed_manager;vehicle_count=$taskVehicleCount;vehicles=$taskVehicles;blocks=$taskBlocks.Count;successful_blocks=($taskBlocks | Where-Object success).Count} | ConvertTo-Json -Depth 5
}catch{
 $taskFailure=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');pid=19680;error=$_.Exception.Message;manager=if($taskManager){$taskManager.ToString('x')}else{$null};blocks=$taskBlocks;read_only=$true;input_sent=$false;process_suspended=$false}
 $taskFailureOut=Join-Path $taskRoot ("local/behavior/runtime-vehicles-failed-"+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')+'.json')
 $taskFailureFile=[IO.File]::Open($taskFailureOut,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskFailureWriter=[IO.StreamWriter]::new($taskFailureFile);$taskFailureWriter.Write(($taskFailure | ConvertTo-Json -Depth 8));$taskFailureWriter.Flush()}finally{$taskFailureFile.Dispose()}
 Write-Output "FAILED_EVIDENCE=$taskFailureOut"
 throw
}finally{[void][GridReadOnlyVehicleState]::CloseHandle($taskHandle)}
