# Read-only, bounded inspection of the already owned private GRID process.
param([Parameter(Mandatory=$true)][string]$SessionJson,[switch]$IncludeGrid)
$ErrorActionPreference='Stop'
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
if((Get-FileHash -LiteralPath $taskPath).Hash.ToLower() -ne 'b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'){throw 'Reference file identity changed'}
if($taskProcess.MainModule.BaseAddress.ToInt64() -ne 0x400000){throw 'Unverified module relocation'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GridReadOnlyCollisionState {
 [DllImport("kernel32.dll",SetLastError=true)] public static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] public static extern bool ReadProcessMemory(IntPtr handle,IntPtr address,byte[] data,UIntPtr length,out UIntPtr read);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
}
'@
$taskHandle=[GridReadOnlyCollisionState]::OpenProcess(0x410,$false,$taskProcessId)
if($taskHandle -eq [IntPtr]::Zero){throw 'Read-only process access failed'}
$taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$taskOut=Join-Path $taskRoot "local/behavior/runtime-collision-$taskStamp.json"
$taskBlocks=[Collections.Generic.List[object]]::new()
$taskGridShapes=[Collections.Generic.List[object]]::new()
function Read-TaskBlock([string]$Name,[long]$Address,[int]$Length){
 if($Address -lt 0x10000 -or $Address -gt 0x7fff0000 -or $Length -lt 1 -or $Length -gt 4096){throw 'Out-of-scope memory request'}
 $taskBytes=[byte[]]::new($Length)
 $taskRead=[UIntPtr]::Zero
 $taskSuccess=[GridReadOnlyCollisionState]::ReadProcessMemory($taskHandle,[IntPtr]::new($Address),$taskBytes,[UIntPtr]::new([uint64]$Length),[ref]$taskRead)
 $taskBlocks.Add([pscustomobject]@{name=$Name;address=$Address.ToString('x');length=$Length;success=$taskSuccess;read=$taskRead.ToUInt64();hex=if($taskSuccess){[Convert]::ToHexString($taskBytes).ToLower()}else{$null}})
 if(!$taskSuccess -or $taskRead.ToUInt64() -ne $Length){return $null}
 return ,$taskBytes
}
try {
 $taskCode=Read-TaskBlock 'triangle_kernel_code' 0xadc290 0x310
 $taskSource=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
 if((Get-FileHash -LiteralPath $taskSource).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Private analysis reference changed'}
 $taskRaw=[IO.File]::ReadAllBytes($taskSource)
 # Verified .text RVA 0x1000, file offset 0x400; current module retains preferred base.
 $taskSourceCode=$taskRaw[(0xadc290-0x400000-0x1000+0x400)..(0xadc290-0x400000-0x1000+0x400+0x30f)]
 if($null -eq $taskCode -or [Convert]::ToHexString($taskCode) -ne [Convert]::ToHexString([byte[]]$taskSourceCode)){throw 'Live numeric code differs from analyzed reference'}
 $taskCount=Read-TaskBlock 'candidate_registry_count' 0xe85b1c 4
 $taskPointers=Read-TaskBlock 'candidate_tile_pointer_table' 0xe84af8 4096
 $taskWidths=Read-TaskBlock 'candidate_tile_width_table' 0xe81af8 4096
 if($IncludeGrid){
  foreach($taskRange in @(@('candidate_collector_code',0x96b440,0x230),@('candidate_coordinates_code',0x96b390,0xa2),@('candidate_subdivision_code',0x9583c0,0xab))){
   $taskLive=Read-TaskBlock $taskRange[0] $taskRange[1] $taskRange[2]
   $taskOffset=$taskRange[1]-0x400000-0x1000+0x400
   $taskExpectedCode=$taskRaw[$taskOffset..($taskOffset+$taskRange[2]-1)]
   if($null -eq $taskLive -or [Convert]::ToHexString($taskLive) -ne [Convert]::ToHexString([byte[]]$taskExpectedCode)){throw 'Live candidate code differs'}
  }
  $taskExponents=Read-TaskBlock 'candidate_tile_exponent_table' 0xe82af8 4096
  $taskRegistry=Read-TaskBlock 'candidate_first_registry_wrapper' 0xe83af8 4
  if($null -eq $taskCount -or $null -eq $taskPointers -or $null -eq $taskWidths -or $null -eq $taskExponents -or $null -eq $taskRegistry){throw 'Required grid header read failed'}
  if([BitConverter]::ToUInt32($taskCount,0) -lt 1){throw 'No active candidate registry'}
  $taskFirstWrapper=[BitConverter]::ToUInt32($taskRegistry,0)
  $taskFirstOuter=Read-TaskBlock 'candidate_bounds_wrapper' $taskFirstWrapper 16
  if($null -eq $taskFirstOuter){throw 'Bounds wrapper absent'}
  $taskFirstShape=Read-TaskBlock 'candidate_bounds_shape' ([BitConverter]::ToUInt32($taskFirstOuter,8)) 0x280
  if($null -eq $taskFirstShape){throw 'Bounds shape absent'}
  $taskFirstHeader=Read-TaskBlock 'candidate_bounds_header' ([BitConverter]::ToUInt32($taskFirstShape,0x244)) 0x80
  if($null -eq $taskFirstHeader){throw 'Bounds header absent'}
  $taskTotalGridBytes=0
  $taskGridWrappers=[Collections.Generic.HashSet[uint32]]::new()
  for($taskIndex=0;$taskIndex -lt 1024;$taskIndex++){
   $taskWidth=[BitConverter]::ToUInt32($taskWidths,$taskIndex*4)
   $taskExponent=[BitConverter]::ToUInt32($taskExponents,$taskIndex*4)
   if($taskExponent -gt 6 -or $taskWidth -ne (1 -shl $taskExponent)){throw "Unsupported grid dimensions at tile $taskIndex"}
   $taskTileLength=[int]($taskWidth*$taskWidth*4)
   $taskTotalGridBytes+=$taskTileLength
   if($taskTotalGridBytes -gt 4194304){throw 'Grid read exceeds four MiB bound'}
   $taskTile=[BitConverter]::ToUInt32($taskPointers,$taskIndex*4)
   for($taskPart=0;$taskPart -lt $taskTileLength;$taskPart+=4096){
    $taskPartLength=[Math]::Min(4096,$taskTileLength-$taskPart)
    $taskReadCells=Read-TaskBlock "grid_tile_$taskIndex-part_$taskPart" ($taskTile+$taskPart) $taskPartLength
    if($null -eq $taskReadCells){throw "Grid cell read failed at tile $taskIndex"}
    for($taskCellWord=0;$taskCellWord -lt $taskPartLength;$taskCellWord+=4){
     $taskCellWrapper=[BitConverter]::ToUInt32($taskReadCells,$taskCellWord)
     if($taskCellWrapper -ne 0){$null=$taskGridWrappers.Add($taskCellWrapper)}
    }
   }
  }
  if($taskGridWrappers.Count -gt 512){throw 'Grid shape limit exceeded'}
  $taskTotalShapeBytes=0
  foreach($taskWrapper in @($taskGridWrappers | Sort-Object)){
   $taskLabel="grid_shape_$($taskWrapper.ToString('x'))"
   $taskOuter=Read-TaskBlock "$taskLabel-wrapper" $taskWrapper 16
   if($null -eq $taskOuter){throw 'Grid shape wrapper absent'}
   $taskShapePointer=[BitConverter]::ToUInt32($taskOuter,8)
   $taskShapeBytes=Read-TaskBlock "$taskLabel-shape" $taskShapePointer 0x280
   if($null -eq $taskShapeBytes -or [BitConverter]::ToUInt32($taskShapeBytes,0x10) -ne 11){throw 'Unsupported grid shape type'}
   $taskHeaderPointer=[BitConverter]::ToUInt32($taskShapeBytes,0x244)
   $taskHeaderBytes=Read-TaskBlock "$taskLabel-header" $taskHeaderPointer 0x80
   if($null -eq $taskHeaderBytes){throw 'Grid shape header absent'}
   $taskTriangles=[BitConverter]::ToUInt32($taskHeaderBytes,0x18)
   $taskVertices=[BitConverter]::ToUInt32($taskHeaderBytes,0x1c)
   $taskMaterials=[BitConverter]::ToUInt32($taskHeaderBytes,0x20)
   if($taskTriangles -lt 1 -or $taskTriangles -gt 32768 -or $taskVertices -lt 1 -or $taskVertices -gt 1024 -or $taskMaterials -lt 1 -or $taskMaterials -gt 8){throw 'Grid packed-shape count outside scope'}
   foreach($taskSection in @(@('vertices',0x24,[int]($taskVertices*8)),@('triangles',0x2c,[int]($taskTriangles*4)))){
    $taskSectionPointer=[BitConverter]::ToUInt32($taskHeaderBytes,$taskSection[1])
    $taskSectionLength=$taskSection[2]
    $taskTotalShapeBytes+=$taskSectionLength
    if($taskTotalShapeBytes -gt 4194304){throw 'Shape payload read exceeds four MiB bound'}
    for($taskPart=0;$taskPart -lt $taskSectionLength;$taskPart+=4096){
     $taskPayload=Read-TaskBlock "$taskLabel-$($taskSection[0])-part_$taskPart" ($taskSectionPointer+$taskPart) ([Math]::Min(4096,$taskSectionLength-$taskPart))
     if($null -eq $taskPayload){throw 'Grid shape payload read failed'}
    }
   }
   $taskGridShapes.Add([pscustomobject]@{wrapper=$taskWrapper.ToString('x');shape=$taskShapePointer.ToString('x');header=$taskHeaderPointer.ToString('x');type_index=11;vertices=$taskVertices;triangles=$taskTriangles;materials=$taskMaterials})
  }
  foreach($taskRecheck in @(@('grid_count_after',0xe85b1c,$taskCount),@('grid_pointers_after',0xe84af8,$taskPointers),@('grid_widths_after',0xe81af8,$taskWidths),@('grid_exponents_after',0xe82af8,$taskExponents))){
   $taskAfter=Read-TaskBlock $taskRecheck[0] $taskRecheck[1] $taskRecheck[2].Length
   if($null -eq $taskAfter -or [Convert]::ToHexString($taskAfter) -ne [Convert]::ToHexString($taskRecheck[2])){throw 'Grid header changed during capture'}
  }
 }
 $null=Read-TaskBlock 'vehicle_manager_cache' 0xe6e834 32
 $taskCandidates=[Collections.Generic.List[object]]::new()
 $taskVisited=[Collections.Generic.HashSet[uint32]]::new()
 if($null -ne $taskPointers){
  for($taskIndex=0;$taskIndex -lt 1024 -and $taskCandidates.Count -lt 8;$taskIndex++){
   $taskTile=[BitConverter]::ToUInt32($taskPointers,$taskIndex*4)
   if($taskTile -lt 0x10000 -or $taskTile -gt 0x7ffe0000){continue}
   if($null -eq $taskWidths){continue}
   $taskSampleWidth=[BitConverter]::ToUInt32($taskWidths,$taskIndex*4)
   if($taskSampleWidth -lt 1 -or $taskSampleWidth -gt 64){continue}
   $taskSampleCount=[Math]::Min(16,$taskSampleWidth*$taskSampleWidth)
   $taskCells=Read-TaskBlock "tile_$taskIndex-first_cells" $taskTile ($taskSampleCount*4)
   if($null -eq $taskCells){continue}
   for($taskCell=0;$taskCell -lt $taskSampleCount -and $taskCandidates.Count -lt 8;$taskCell++){
    $taskWrapper=[BitConverter]::ToUInt32($taskCells,$taskCell*4)
    if($taskWrapper -lt 0x10000 -or $taskWrapper -gt 0x7ffe0000 -or !$taskVisited.Add($taskWrapper)){continue}
    $taskOuter=Read-TaskBlock "wrapper_$($taskWrapper.ToString('x'))" $taskWrapper 16
    if($null -eq $taskOuter){continue}
    $taskShape=[BitConverter]::ToUInt32($taskOuter,8)
    if($taskShape -lt 0x10000 -or $taskShape -gt 0x7ffe0000){continue}
    $taskShapeBytes=Read-TaskBlock "shape_$($taskShape.ToString('x'))" $taskShape 0x280
    if($null -eq $taskShapeBytes){continue}
    $taskType=[BitConverter]::ToUInt32($taskShapeBytes,0x10)
    $taskHeader=[BitConverter]::ToUInt32($taskShapeBytes,0x244)
    $taskCandidates.Add([pscustomobject]@{tile_index=$taskIndex;cell=$taskCell;wrapper=$taskWrapper.ToString('x');shape=$taskShape.ToString('x');type_index=$taskType;header=$taskHeader.ToString('x')})
    if($taskType -eq 11 -and $taskHeader -ge 0x10000 -and $taskHeader -le 0x7ffe0000){
     $taskHeaderBytes=Read-TaskBlock "packed_header_$($taskHeader.ToString('x'))" $taskHeader 0x80
     if($null -ne $taskHeaderBytes){
      foreach($taskSection in @(@('vertices',0x24),@('triangles',0x2c))){
       $taskSectionPointer=[BitConverter]::ToUInt32($taskHeaderBytes,$taskSection[1])
       if($taskSectionPointer -ge 0x10000 -and $taskSectionPointer -le 0x7ffe0000){$null=Read-TaskBlock "$($taskSection[0])_$($taskHeader.ToString('x'))" $taskSectionPointer 64}
      }
     }
    }
   }
  }
 }
 $taskRecord=[pscustomobject]@{utc=[DateTime]::UtcNow.ToString('o');pid=$taskProcessId;created_utc=$taskProcess.StartTime.ToUniversalTime().ToString('o');path=$taskPath;live_numeric_code_exact=$true;full_grid_requested=[bool]$IncludeGrid;grid_header_rechecked=[bool]$IncludeGrid;registry_count=if($null -ne $taskCount){[BitConverter]::ToUInt32($taskCount,0)}else{$null};grid_shapes=$taskGridShapes;candidates=$taskCandidates;blocks=$taskBlocks;read_only=$true;input_sent=$false;process_suspended=$false}
 $taskText=$taskRecord | ConvertTo-Json -Depth 8
 $taskFile=[IO.File]::Open($taskOut,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskWriter=[IO.StreamWriter]::new($taskFile);$taskWriter.Write($taskText);$taskWriter.Flush()}finally{$taskFile.Dispose()}
 [pscustomobject]@{output=$taskOut;registry_count=$taskRecord.registry_count;candidates=$taskCandidates;successful_blocks=($taskBlocks | Where-Object success).Count;blocks=$taskBlocks.Count} | ConvertTo-Json -Depth 5
}finally{[void][GridReadOnlyCollisionState]::CloseHandle($taskHandle)}
