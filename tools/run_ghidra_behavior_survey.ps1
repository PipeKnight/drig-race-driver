param([string[]]$Entries,[string[]]$ReturnContracts,[string[]]$Containing)
$ErrorActionPreference='Stop'
if(([int]($Entries.Count -gt 0)+[int]($ReturnContracts.Count -gt 0)+[int]($Containing.Count -gt 0)) -gt 1){throw 'Choose entries, return contracts or containing addresses'}
if($Containing.Count -gt 64){throw 'At most 64 interior addresses are supported'}
if($ReturnContracts.Count -gt 64){throw 'At most 64 return contracts are supported'}
foreach($taskContract in $ReturnContracts){
 if($taskContract -notmatch '^[0-9a-fA-F]{1,8}:(float|void)$'){throw 'Contracts must be hexadecimal-address:float or :void'}
}
$taskAddressInputs=@()
if($Entries.Count){$taskAddressInputs+=$Entries}
if($Containing.Count){$taskAddressInputs+=$Containing}
foreach($taskEntry in $taskAddressInputs){
 if($taskEntry -notmatch '^(0x)?[0-9a-fA-F]{1,8}$'){throw 'Entries must be quoted hexadecimal addresses'}
}
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 're\GhidraMaster.ps1')
$taskImage=Assert-GridUnpackedImage
$taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$taskOutput=Join-Path $taskRoot "local/behavior/$taskStamp"
try {
 $taskScripts=if($Containing.Count){@('-postScript','GridContaining.java',$taskOutput)+$Containing}elseif($ReturnContracts.Count){@('-postScript','GridReturnContracts.java',$taskOutput)+$ReturnContracts}elseif($Entries.Count){@('-postScript','GridProbe.java',$taskOutput)+$Entries}else{@('-postScript','GridBehaviorSurvey.java',$taskOutput)}
 Invoke-GridHeadless ((Get-GridMasterArguments)+$taskScripts)
 if($LASTEXITCODE -ne 0){throw "Behavior survey failed: $LASTEXITCODE"}
 if($Entries.Count -or $ReturnContracts.Count -or $Containing.Count){
  $taskRequestedEntries=if($Containing.Count){$Containing}elseif($ReturnContracts.Count){$ReturnContracts | ForEach-Object {($_ -split ':')[0]}}else{$Entries}
  foreach($taskEntry in $taskRequestedEntries){
   $taskName=([Convert]::ToUInt64($taskEntry.Replace('0x',''),16)).ToString('x')+'.c.txt'
   $taskResult=Join-Path $taskOutput $taskName
   if(!(Test-Path -LiteralPath $taskResult) -or (Get-Content -LiteralPath $taskResult -Raw).Contains('// FAILED')){throw "Missing/failed decompilation output for $taskEntry"}
  }
 } elseif(!(Test-Path -LiteralPath (Join-Path $taskOutput 'strings.tsv'))){throw 'Missing string survey output'}
 Write-Output "PRIVATE_OUTPUT=$taskOutput"
} finally {}
