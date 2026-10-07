param([Parameter(Mandatory=$true)][string[]]$Addresses)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskImage=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
if((Get-FileHash -LiteralPath $taskImage).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Private PE identity mismatch'}
$taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
$taskOutput=Join-Path $taskRoot "local/behavior/$taskStamp"
$taskPreviousJava=$env:JAVA_HOME
try {
 $env:JAVA_HOME='C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre'
 $taskArguments=@((Join-Path $taskRoot 'local/phase3/ghidra-project'),'grid-unpacked','-process','grid-unpacked.exe','-noanalysis','-scriptPath',(Join-Path $PSScriptRoot 'ghidra'),'-postScript','GridAddressRefs.java',$taskOutput)+$Addresses
 & 'C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat' @taskArguments
 if($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath (Join-Path $taskOutput 'address-refs.tsv'))){throw 'Reference survey failed'}
 Write-Output "PRIVATE_OUTPUT=$taskOutput"
} finally {$env:JAVA_HOME=$taskPreviousJava}
