# Candidate stores by literal displacement; object identity requires separate proof.
param([Parameter(Mandatory=$true)][string[]]$Offsets,[switch]$AllOperands,[string]$FunctionRange='')
$ErrorActionPreference='Stop'
foreach($taskOffset in $Offsets){
 if($taskOffset -notmatch '^0x[0-9a-f]{1,6}$'){throw 'Offsets must be lower-case hexadecimal strings'}
}
if($FunctionRange -and (-not $AllOperands -or $FunctionRange -notmatch '^[0-9a-f]{6,8}-[0-9a-f]{6,8}$')) { throw 'FunctionRange requires AllOperands and a lower-case hex start-end range' }
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskImage=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
if((Get-FileHash -LiteralPath $taskImage).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Private PE identity mismatch'}
$taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$taskDirectory=Join-Path $taskRoot "local/behavior/$taskStamp"
New-Item -ItemType Directory -Path $taskDirectory -ErrorAction Stop | Out-Null
$taskOutput=Join-Path $taskDirectory $(if($AllOperands){'displacement-refs.tsv'}else{'field-stores.tsv'})
$taskScript=if($AllOperands){'GridDisplacementRefs.java'}else{'GridManagerStores.java'}
$taskPreviousJava=$env:JAVA_HOME
try {
 $env:JAVA_HOME='C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre'
 $taskArguments=@((Join-Path $taskRoot 'local/phase3/ghidra-project'),'grid-unpacked','-process','grid-unpacked.exe','-noanalysis','-scriptPath',(Join-Path $PSScriptRoot 'ghidra'),'-postScript',$taskScript,$taskOutput)+$Offsets
 if($FunctionRange) { $taskArguments+=('functions:'+$FunctionRange) }
 $taskLog=@(& 'C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat' @taskArguments 2>&1)
 $taskExit=$LASTEXITCODE
 $taskLog | Set-Content -LiteralPath (Join-Path $taskDirectory 'headless.log') -Encoding utf8
 $taskLog | Write-Output
 $taskMarker=if($AllOperands){'GRID_DISPLACEMENT_REFS_OK'}else{'GRID_MANAGER_STORES_OK'}
 if($taskExit -ne 0 -or !(Test-Path -LiteralPath $taskOutput) -or -not ($taskLog | Select-String -SimpleMatch $taskMarker) -or ($taskLog | Select-String -SimpleMatch 'REPORT SCRIPT ERROR')){throw 'Field-store survey incomplete; private partial output/log retained'}
 Write-Output "PRIVATE_OUTPUT=$taskOutput"
} finally {$env:JAVA_HOME=$taskPreviousJava}
