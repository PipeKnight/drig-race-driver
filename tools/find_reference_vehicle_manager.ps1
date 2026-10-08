# Bounded read-only search for an observed exact general-manager vtable.
param([long]$ScanLimitBytes=64MB,[long]$ManagerAddress=0,[switch]$IncludeDynamics,[switch]$IncludeContacts,[switch]$IncludeSteering,[switch]$IncludeDeformation,[switch]$IncludeImpact,
 [string]$SessionJson='local/reference-race/resume-20261007/session.json')
$ErrorActionPreference='Stop'
if($ScanLimitBytes -lt 1 -or $ScanLimitBytes -gt 256MB){throw 'Scan limit must be 1..256 MiB'}
if($IncludeDeformation -and !$IncludeSteering){throw 'Deformation capture requires steering identity checks'}
if($IncludeImpact -and !$IncludeDeformation){throw 'Impact capture requires deformation identity checks'}
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskSessionPath=[IO.Path]::GetFullPath((Join-Path $taskRoot $SessionJson))
if(!$taskSessionPath.StartsWith((Join-Path $taskRoot 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Session must remain under local/'}
$taskSession=Get-Content -LiteralPath $taskSessionPath -Raw | ConvertFrom-Json
$taskProcessId=[int]$taskSession.Pid
$taskProcess=Get-Process -Id $taskProcessId
$taskPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$taskExpected=([DateTimeOffset]$taskSession.StartedUtc).UtcDateTime
if($taskSession.Path -ne $taskPath -or $taskSession.ExeSHA256 -ne 'b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'){throw 'Unverified reference session'}
if($taskProcess.StartTime.ToUniversalTime().Ticks -ne $taskExpected.ToUniversalTime().Ticks -or $taskProcess.MainModule.FileName -ne $taskPath){throw 'Reference process identity changed'}
if((Get-FileHash -LiteralPath $taskPath).Hash.ToLower() -ne 'b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'){throw 'Reference executable identity changed'}
if($taskProcess.MainModule.BaseAddress.ToInt64() -ne 0x400000){throw 'Unverified module relocation'}
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class GridFindReadOnlyManager {
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
 public static int[] Find(byte[] bytes,uint value) {
  var matches=new List<int>();
  for(int i=0;i+4<=bytes.Length;i+=4) {
   if(BitConverter.ToUInt32(bytes,i)==value) {
    matches.Add(i);
    if(matches.Count>256) throw new InvalidOperationException("Candidate limit exceeded");
   }
  }
  return matches.ToArray();
 }
}
'@
$taskHandle=[GridFindReadOnlyManager]::OpenProcess(0x410,$false,$taskProcessId)
if($taskHandle -eq [IntPtr]::Zero){throw 'Read-only process access failed'}
$taskBlocks=[Collections.Generic.List[object]]::new()
function Read-TaskBytes([long]$Address,[int]$Length,[string]$Name){
 if($Address -lt 0x10000 -or $Address -gt 0x7fff0000 -or $Length -lt 1 -or $Length -gt 65536){throw 'Out-of-scope read'}
 $taskBytes=[byte[]]::new($Length);$taskRead=[UIntPtr]::Zero
 $taskSuccess=[GridFindReadOnlyManager]::ReadProcessMemory($taskHandle,[IntPtr]::new($Address),$taskBytes,[UIntPtr]::new([uint64]$Length),[ref]$taskRead)
 if($Name){$taskBlocks.Add([pscustomobject]@{name=$Name;address=$Address.ToString('x');length=$Length;success=$taskSuccess;read=$taskRead.ToUInt64();hex=if($taskSuccess){[Convert]::ToHexString($taskBytes).ToLower()}else{$null}})}
 if(!$taskSuccess -or $taskRead.ToUInt64() -ne $Length){return $null}
 return ,$taskBytes
}
try {
 $taskSource=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
 if((Get-FileHash -LiteralPath $taskSource).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Private analysis reference changed'}
 $taskRaw=[IO.File]::ReadAllBytes($taskSource)
 $taskCode=Read-TaskBytes 0x9ad030 32 'general_vehicle_count_code'
 $taskCodeOffset=0x9ad030-0x400000-0x1000+0x400
 if($null -eq $taskCode -or [Convert]::ToHexString($taskCode) -ne [Convert]::ToHexString([byte[]]$taskRaw[$taskCodeOffset..($taskCodeOffset+31)])){throw 'Live count routine differs'}
 if($IncludeSteering){
  $taskSteeringEntries=@(0x9c5b90,0x9bd7f0,0x9f2f80)
  if($IncludeDeformation){$taskSteeringEntries+=@(0x9bb1e0,0x9a39d0,0x9a26f0,0x9a2110,0x9fec70)}
  if($IncludeImpact){$taskSteeringEntries+=@(0x9d4970,0x9ea6e0,0xa08040,0x997020,0xa69340,0x989bf0,0x944e00,0x9d4210,0xa064a0,0xa06a10)}
  foreach($taskEntry in $taskSteeringEntries){
   $taskSteeringCode=Read-TaskBytes $taskEntry 32 ('steering_code_'+$taskEntry.ToString('x'))
   $taskEntryOffset=$taskEntry-0x400000-0x1000+0x400
   if($null -eq $taskSteeringCode -or [Convert]::ToHexString($taskSteeringCode) -ne [Convert]::ToHexString([byte[]]$taskRaw[$taskEntryOffset..($taskEntryOffset+31)])){throw 'Live steering routine differs'}
  }
 }
 if($IncludeImpact){
  $taskSolverGlobals=Read-TaskBytes 0xe4fc80 0x20 'deformation_solver_globals'
  $taskSolverClip=Read-TaskBytes 0xe500b8 1 'deformation_solver_clip_flag'
  $taskProducerScale=Read-TaskBytes 0xe4fc2c 4 'deformation_producer_corner_scale'
  $taskProducerGlobals=Read-TaskBytes 0xe500c0 0x20 'deformation_producer_globals'
  if($null -eq $taskSolverGlobals -or $null -eq $taskSolverClip -or $null -eq $taskProducerScale -or $null -eq $taskProducerGlobals){throw 'Missing deformation solver/producer globals'}
 }
 $taskScanBytes=0L;$taskRegions=0;$taskAddress=0x10000L
 $taskCandidates=[Collections.Generic.List[long]]::new()
 if($ManagerAddress){
  if($ManagerAddress -lt 0x10000 -or $ManagerAddress -gt 0x7ffe0000){throw 'Manager address outside scope'}
  $taskCandidates.Add($ManagerAddress)
 }
 while(!$ManagerAddress -and $taskAddress -lt 0x7fff0000 -and $taskScanBytes -lt $ScanLimitBytes){
  $taskRegion=[GridFindReadOnlyManager+Region]::new()
  $taskQuery=[GridFindReadOnlyManager]::VirtualQueryEx($taskHandle,[IntPtr]::new($taskAddress),[ref]$taskRegion,[UIntPtr]::new([uint64][Runtime.InteropServices.Marshal]::SizeOf($taskRegion)))
  if($taskQuery -eq [UIntPtr]::Zero){throw 'Memory metadata query failed'}
  $taskBase=$taskRegion.BaseAddress.ToInt64();$taskSize=[long]$taskRegion.RegionSize.ToUInt64()
  if($taskSize -lt 1 -or $taskBase+$taskSize -le $taskAddress){throw 'Invalid region progression'}
  # Only committed private read/write memory; exclude guard, executable and mapped/image regions.
  if($taskRegion.State -eq 0x1000 -and $taskRegion.Type -eq 0x20000 -and $taskRegion.Protect -eq 4){
   $taskRegions++
   for($taskOffset=0L;$taskOffset -lt $taskSize -and $taskScanBytes -lt $ScanLimitBytes;$taskOffset+=65536){
    $taskLength=[int][Math]::Min(65536,[Math]::Min($taskSize-$taskOffset,$ScanLimitBytes-$taskScanBytes))
    $taskBytes=Read-TaskBytes ($taskBase+$taskOffset) $taskLength ''
    $taskScanBytes+=$taskLength
    if($null -ne $taskBytes){foreach($taskMatch in [GridFindReadOnlyManager]::Find($taskBytes,0xd199cc)){$taskCandidates.Add($taskBase+$taskOffset+$taskMatch)}}
    if($taskCandidates.Count -gt 256){throw 'Candidate limit exceeded'}
   }
   if($taskCandidates.Count){break}
  }
  $taskAddress=$taskBase+$taskSize
 }
 $taskManagers=[Collections.Generic.List[object]]::new()
 foreach($taskCandidate in $taskCandidates){
  $taskPrefix=Read-TaskBytes $taskCandidate 0x100 ('manager_candidate_'+$taskCandidate.ToString('x'))
  if($null -eq $taskPrefix){continue}
  if([BitConverter]::ToUInt32($taskPrefix,0) -ne 0xd199cc){throw 'General manager vtable differs'}
  $taskBegin=[BitConverter]::ToUInt32($taskPrefix,0x4c);$taskEnd=[BitConverter]::ToUInt32($taskPrefix,0x50)
  if($taskBegin -lt 0x10000 -or $taskEnd -lt $taskBegin -or ($taskEnd-$taskBegin)%4 -ne 0 -or ($taskEnd-$taskBegin)/4 -gt 64){continue}
  $taskCount=($taskEnd-$taskBegin)/4
  if($taskCount -lt 1){continue}
  $taskVector=Read-TaskBytes $taskBegin ($taskEnd-$taskBegin) ('manager_vector_'+$taskCandidate.ToString('x'))
  if($null -eq $taskVector){continue}
  $taskCars=[Collections.Generic.List[object]]::new()
  for($taskIndex=0;$taskIndex -lt $taskCount;$taskIndex++){
   $taskCar=[BitConverter]::ToUInt32($taskVector,$taskIndex*4)
   $taskHeader=Read-TaskBytes $taskCar 0x100 "car_$taskIndex-header"
   if($null -eq $taskHeader){throw 'General vehicle header failed'}
   $taskBody=[BitConverter]::ToUInt32($taskHeader,0x24)
   $null=Read-TaskBytes $taskBody 0x100 "car_$taskIndex-body_header"
   # Spawn setter 9ef040 passes body+db0 as ECX; a696f0/a69840/a69860
   # follow wrapper+8 into the pose/velocity storage. Keep both views distinct.
   $taskWrapperLink=Read-TaskBytes ($taskBody+0xdb0) 4 "car_$taskIndex-state_wrapper_link"
   if($null -eq $taskWrapperLink){throw 'State wrapper link read failed'}
   $taskWrapper=[BitConverter]::ToUInt32($taskWrapperLink,0)
   $taskWrapperHeader=Read-TaskBytes $taskWrapper 0x20 "car_$taskIndex-state_wrapper_header"
   if($null -eq $taskWrapperHeader){throw 'State wrapper header read failed'}
   $taskState=[BitConverter]::ToUInt32($taskWrapperHeader,8)
   $taskStateFields=Read-TaskBytes ($taskState+0x190) 0xb0 "car_$taskIndex-linked_state_fields"
   if($null -eq $taskStateFields){throw 'Linked state fields read failed'}
   $taskGrid=Read-TaskBytes ($taskCar+0x2e4c) 8 "car_$taskIndex-grid"
   $taskWheels=Read-TaskBytes ($taskBody+0x1620) 4 "car_$taskIndex-wheel_count"
   if($null -eq $taskWheels){throw 'Body wheel-count read failed'}
   $taskWheelCount=[BitConverter]::ToUInt32($taskWheels,0)
   if($taskWheelCount -lt 1 -or $taskWheelCount -gt 8){throw 'Body wheel count outside scope'}
   $taskWheelRecords=Read-TaskBytes ($taskBody+0x1990) ($taskWheelCount*0x150) "car_$taskIndex-wheel_records"
   $null=Read-TaskBytes ($taskBody+0x15d0) 0x90 "car_$taskIndex-frame_fields"
   $null=Read-TaskBytes ($taskBody+0x15a0) 0x30 "car_$taskIndex-mass_inertia_fields"
   $null=Read-TaskBytes ($taskBody+0xed64) 0x40 "car_$taskIndex-dimension_fields"
   $null=Read-TaskBytes ($taskBody+0x1640) 0x350 "car_$taskIndex-parameter_neighborhood"
   $null=Read-TaskBytes ($taskBody+0xe294) 0x24 "car_$taskIndex-source_wheel_geometry"
   $null=Read-TaskBytes ($taskBody+0x14e0) 0x50 "car_$taskIndex-initialized_wheel_geometry"
   if($IncludeSteering){
    if($taskWheelCount -ne 4){throw 'Steering capture currently scoped to four wheels'}
    # Embedded object, source settings, runtime levels/seeds and producer output.
    $null=Read-TaskBytes ($taskBody+0xcdf0) 0x460 "car_$taskIndex-mutable_steering"
    # Keep separately read production copies distinct from the producer output.
    $null=Read-TaskBytes ($taskBody+0x2294) 0x3c "car_$taskIndex-steering_copies"
    # Optional visual-deformation correction used by the wheel-level updater.
    $null=Read-TaskBytes ($taskBody+0xc7ae) 1 "car_$taskIndex-visual_damage_flag"
    $null=Read-TaskBytes ($taskBody+0xc740) 0x10 "car_$taskIndex-visual_damage_correction"
    $null=Read-TaskBytes ($taskBody+0xeefc) 4 "car_$taskIndex-visual_damage_scale"
    if($IncludeDeformation){
     $null=Read-TaskBytes ($taskBody+0xc7ad) 1 "car_$taskIndex-deformation_flag"
     if(!$IncludeContacts){$null=Read-TaskBytes ($taskBody+0x30c) 0xa0 "car_$taskIndex-wheel_radius_samples"}
     $null=Read-TaskBytes ($taskBody+0xe25c) 8 "car_$taskIndex-deformation_radius_thresholds"
     $null=Read-TaskBytes ($taskBody+0x22e0) 4 "car_$taskIndex-deformation_tyre_flags"
     # Sampler ECX is body+2300; preserve descriptor-relative provenance.
     $taskGridDescriptor=Read-TaskBytes ($taskBody+0x2300+0xa414) 0x2c "car_$taskIndex-deformation_grid_descriptor"
     if($null -eq $taskGridDescriptor){throw 'Missing deformation descriptor'}
     $taskGridCount=1
     foreach($taskAxis in 0..2){
      $taskAxisCount=[BitConverter]::ToInt32($taskGridDescriptor,$taskAxis*4)
      if($taskAxisCount -lt 2 -or $taskAxisCount -gt 16){throw 'Unsupported deformation grid dimension'}
      $taskGridCount*=$taskAxisCount
     }
     $taskGridSpacing=[BitConverter]::ToSingle($taskGridDescriptor,0xc)
     if($taskGridCount -gt 400 -or ![Single]::IsFinite($taskGridSpacing) -or $taskGridSpacing -le 0){throw 'Unsupported deformation grid size/spacing'}
     $taskGridNodes=Read-TaskBytes ($taskBody+0x2300+0x2580) ($taskGridCount*0x50) "car_$taskIndex-deformation_grid_nodes"
     if($null -eq $taskGridNodes){throw 'Missing deformation grid node records'}
     # The settling graph addresses the fixed 400-record pool, including
     # observed sentinel self-edges at node399 outside the sampled dimensions.
     if($taskGridCount -lt 400){
      $taskPaddingNodes=Read-TaskBytes ($taskBody+0x2300+0x2580+$taskGridCount*0x50) ((400-$taskGridCount)*0x50) "car_$taskIndex-deformation_grid_padding_nodes"
      if($null -eq $taskPaddingNodes){throw 'Missing deformation node-pool tail'}
     }
     # Descriptor edge graph and state consumed by the settling routines.
     $taskEdgeCountBytes=Read-TaskBytes ($taskBody+0x2300+0xa410) 4 "car_$taskIndex-deformation_edge_count"
     if($null -eq $taskEdgeCountBytes){throw 'Missing deformation edge count'}
     $taskEdgeCount=[BitConverter]::ToInt32($taskEdgeCountBytes,0)
     if($taskEdgeCount -lt 0 -or $taskEdgeCount -gt 1200){throw 'Unsupported deformation edge count'}
     if($taskEdgeCount){
      $taskEdges=Read-TaskBytes ($taskBody+0x2300) ($taskEdgeCount*8) "car_$taskIndex-deformation_edges"
      if($null -eq $taskEdges){throw 'Missing deformation edges'}
      for($taskEdge=0;$taskEdge -lt $taskEdgeCount;$taskEdge++){
       foreach($taskLane in @(0,2)){
        $taskNodeIndex=[BitConverter]::ToInt16($taskEdges,$taskEdge*8+$taskLane)
        if($taskNodeIndex -lt 0 -or $taskNodeIndex -ge 400){throw 'Deformation edge index outside fixed node pool'}
       }
      }
     }
     $taskGridState=Read-TaskBytes ($taskBody+0x2300+0xa4a0) 0x60 "car_$taskIndex-deformation_grid_state"
     if($null -eq $taskGridState){throw 'Missing deformation grid state'}
     $taskDeformationSettings=[BitConverter]::ToUInt32($taskGridState,0x58)
     if($taskDeformationSettings -lt 0x10000 -or $taskDeformationSettings -gt 0x7ffef000){throw 'Deformation settings pointer outside scope'}
     $taskStrainSettings=Read-TaskBytes ($taskDeformationSettings+0x1024) 8 "car_$taskIndex-deformation_strain_settings"
     if($null -eq $taskStrainSettings){throw 'Missing deformation strain settings'}
     if($IncludeImpact){
      $taskImpactSettings=Read-TaskBytes ($taskDeformationSettings+0x102c) 0x24 "car_$taskIndex-deformation_impact_settings"
      $taskImpactState=Read-TaskBytes ($taskBody+0x2300+0xa440) 0x40 "car_$taskIndex-deformation_impact_state"
      $taskNodeFlags=Read-TaskBytes ($taskBody+0x2300+0xa280) 400 "car_$taskIndex-deformation_node_flags"
      $taskProducerState=Read-TaskBytes ($taskBody+0x2300+0xa480) 0x20 "car_$taskIndex-deformation_producer_state"
      $taskOffsetVector=Read-TaskBytes ($taskBody+0x1630) 0x10 "car_$taskIndex-deformation_body_offset"
      $taskLinkedResponse=Read-TaskBytes ($taskState+0x360) 0x10 "car_$taskIndex-deformation_linked_response"
      $taskProducerCallback=Read-TaskBytes ($taskState+0x418) 0xc "car_$taskIndex-deformation_callback_registration"
      if($null -eq $taskImpactSettings -or $null -eq $taskImpactState -or $null -eq $taskNodeFlags -or $null -eq $taskProducerState -or $null -eq $taskOffsetVector -or $null -eq $taskLinkedResponse -or $null -eq $taskProducerCallback){throw 'Missing deformation impact/producer inputs'}
     }
    }
   }
   if($IncludeDynamics -or $IncludeContacts){
    if($taskWheelCount -ne 4){throw 'Dynamics context currently scoped to four wheels'}
    $taskDynamics=Read-TaskBytes ($taskBody+0x3d0) 0x740 "car_$taskIndex-dynamics_context"
    if($null -eq $taskDynamics -or [BitConverter]::ToUInt32($taskDynamics,0x5d4) -ne $taskBody -or [BitConverter]::ToUInt32($taskDynamics,0x5d8) -ne 4){throw 'Dynamics context identity mismatch'}
    $null=Read-TaskBytes ($taskBody+0xb20) 0x80 "car_$taskIndex-prior_velocity_pose"
    $null=Read-TaskBytes ($taskBody+0x3a0) 0x30 "car_$taskIndex-delta_neighborhood"
    if(!$IncludeDeformation){$null=Read-TaskBytes ($taskBody+0xc7ad) 1 "car_$taskIndex-deformation_flag"}
   }
   if($IncludeContacts){
    if($null -eq $taskWheelRecords){throw 'Contact wheel records absent'}
    $null=Read-TaskBytes ($taskBody+0xe78) 0x65c "car_$taskIndex-decoded_tuning"
    $null=Read-TaskBytes ($taskBody+0xcdf0) 0x360 "car_$taskIndex-setup_neighborhood"
    $null=Read-TaskBytes ($taskBody+0x1560) 0x30 "car_$taskIndex-height_angle_fields"
    $null=Read-TaskBytes ($taskBody+0x30c) 0xa0 "car_$taskIndex-wheel_radius_samples"
    $taskMaterialPointers=[Collections.Generic.HashSet[uint32]]::new()
    for($taskWheelIndex=0;$taskWheelIndex -lt 4;$taskWheelIndex++){
     $taskMaterial=[BitConverter]::ToUInt32($taskWheelRecords,$taskWheelIndex*0x150+0xc0)
     if($taskMaterial -eq 0){continue}
     if($taskMaterialPointers.Add($taskMaterial)){
      $null=Read-TaskBytes $taskMaterial 0xb0 "car_$taskIndex-material_$($taskMaterial.ToString('x'))"
     }
    }
    $taskSurfaceTable=[BitConverter]::ToUInt32($taskDynamics,0x608)
    if($taskSurfaceTable){
     $taskSurfaceHeader=Read-TaskBytes $taskSurfaceTable 0x20 "car_$taskIndex-surface_table_header"
     if($null -eq $taskSurfaceHeader){throw 'Surface table header read failed'}
     $taskSurfaceCount=[BitConverter]::ToInt32($taskSurfaceHeader,0xc)
     $taskSurfaceEntries=[BitConverter]::ToUInt32($taskSurfaceHeader,0x14)
     if($taskSurfaceCount -lt 0 -or $taskSurfaceCount -gt 512){throw 'Surface table count outside bounded scope'}
     if($taskSurfaceCount){$null=Read-TaskBytes $taskSurfaceEntries ($taskSurfaceCount*12) "car_$taskIndex-surface_table_entries"}
    }
   }
   $taskCars.Add([pscustomobject]@{index=$taskIndex;vehicle=$taskCar.ToString('x');body=$taskBody.ToString('x');kind=[BitConverter]::ToUInt32($taskHeader,0x6c);grid_index=if($taskGrid){[BitConverter]::ToInt32($taskGrid,0)}else{$null};wheel_count=$taskWheelCount})
  }
  $taskManagers.Add([pscustomobject]@{address=$taskCandidate.ToString('x');count=$taskCount;cars=$taskCars})
 }
 $taskRecord=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');pid=$taskProcessId;path=$taskPath;created_utc=$taskExpected.ToString('o');read_only=$true;input_sent=$false;process_suspended=$false;live_count_code_exact=$true;scan_bytes=$taskScanBytes;scan_limit_bytes=$ScanLimitBytes;last_region_base=if($taskScanBytes){$taskBase.ToString('x')}else{$null};last_region_offset=$taskOffset;regions=$taskRegions;candidate_addresses=@($taskCandidates | ForEach-Object {$_.ToString('x')});managers=$taskManagers;blocks=$taskBlocks}
 $taskOut=Join-Path $taskRoot ('local/behavior/general-vehicle-manager-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')+'.json')
 $taskFile=[IO.File]::Open($taskOut,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskWriter=[IO.StreamWriter]::new($taskFile);$taskWriter.Write(($taskRecord | ConvertTo-Json -Depth 10));$taskWriter.Flush()}finally{$taskFile.Dispose()}
 [pscustomobject]@{output=$taskOut;scan_bytes=$taskScanBytes;regions=$taskRegions;candidates=$taskCandidates.Count;managers=$taskManagers;blocks=$taskBlocks.Count;successful_blocks=($taskBlocks | Where-Object success).Count} | ConvertTo-Json -Depth 8
}catch{
 $taskFailure=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');pid=$taskProcessId;error=$_.Exception.Message;requested_manager=$ManagerAddress.ToString('x');blocks=$taskBlocks;read_only=$true;input_sent=$false;process_suspended=$false}
 $taskFailureOut=Join-Path $taskRoot ('local/behavior/general-vehicle-manager-failed-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')+'.json')
 $taskFailureFile=[IO.File]::Open($taskFailureOut,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskFailureWriter=[IO.StreamWriter]::new($taskFailureFile);$taskFailureWriter.Write(($taskFailure | ConvertTo-Json -Depth 10));$taskFailureWriter.Flush()}finally{$taskFailureFile.Dispose()}
 Write-Output "FAILED_EVIDENCE=$taskFailureOut"
 throw
}finally{[void][GridFindReadOnlyManager]::CloseHandle($taskHandle)}
