$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 're\GhidraMaster.ps1')
$image=Assert-GridUnpackedImage
$out=Join-Path $root 'local\scene\texture-survey'
if(Test-Path -LiteralPath $out){throw 'Refusing to replace texture survey evidence'}
New-Item -ItemType Directory -Path $out -Force | Out-Null
Invoke-GridHeadless ((Get-GridMasterArguments)+@('-postScript','GridTextureSurvey.java',$out,'-log',(Join-Path $out 'ghidra.log'),'-scriptlog',(Join-Path $out 'script.log'))) | Out-Null
if($LASTEXITCODE -ne 0){throw "Texture survey failed: $LASTEXITCODE"}
