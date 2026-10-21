param([Parameter(Mandatory=$true)][string[]]$Addresses)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 're\GhidraMaster.ps1')
$taskImage=Assert-GridUnpackedImage
$taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$taskOutput=Join-Path $taskRoot "local/behavior/$taskStamp"
try {
 $taskArguments=(Get-GridMasterArguments)+@('-postScript','GridAddressRefs.java',$taskOutput)+$Addresses
 Invoke-GridHeadless $taskArguments
 if($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath (Join-Path $taskOutput 'address-refs.tsv'))){throw 'Reference survey failed'}
 Write-Output "PRIVATE_OUTPUT=$taskOutput"
} finally {}
