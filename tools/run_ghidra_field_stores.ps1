# Candidate stores by literal displacement; object identity requires separate proof.
param([Parameter(Mandatory=$true)][string[]]$Offsets,[switch]$AllOperands,[string]$FunctionRange='')
$ErrorActionPreference='Stop'
foreach($taskOffset in $Offsets){
 if($taskOffset -notmatch '^0x[0-9a-f]{1,6}$'){throw 'Offsets must be lower-case hexadecimal strings'}
}
if($FunctionRange -and (-not $AllOperands -or $FunctionRange -notmatch '^[0-9a-f]{6,8}-[0-9a-f]{6,8}$')) { throw 'FunctionRange requires AllOperands and a lower-case hex start-end range' }
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 're\GhidraMaster.ps1')
$taskImage=Assert-GridUnpackedImage
$taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$taskDirectory=Join-Path $taskRoot "local/behavior/$taskStamp"
New-Item -ItemType Directory -Path $taskDirectory -ErrorAction Stop | Out-Null
$taskOutput=Join-Path $taskDirectory $(if($AllOperands){'displacement-refs.tsv'}else{'field-stores.tsv'})
$taskScript=if($AllOperands){'GridDisplacementRefs.java'}else{'GridManagerStores.java'}
try {
 $taskArguments=(Get-GridMasterArguments)+@('-postScript',$taskScript,$taskOutput)+$Offsets
 if($FunctionRange) { $taskArguments+=('functions:'+$FunctionRange) }
 $taskLog=@(Invoke-GridHeadless $taskArguments)
 $taskExit=$LASTEXITCODE
 $taskLog | Set-Content -LiteralPath (Join-Path $taskDirectory 'headless.log') -Encoding utf8
 $taskLog | Write-Output
 $taskMarker=if($AllOperands){'GRID_DISPLACEMENT_REFS_OK'}else{'GRID_MANAGER_STORES_OK'}
 if($taskExit -ne 0 -or !(Test-Path -LiteralPath $taskOutput) -or -not ($taskLog | Select-String -SimpleMatch $taskMarker) -or ($taskLog | Select-String -SimpleMatch 'REPORT SCRIPT ERROR')){throw 'Field-store survey incomplete; private partial output/log retained'}
 Write-Output "PRIVATE_OUTPUT=$taskOutput"
} finally {}
