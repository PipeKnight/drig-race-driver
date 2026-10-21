# Loader/save surveys on the shared master project (read-only; docs/re/README.md).
# -Existing is accepted for old command lines; the master is always used, never re-imported.
param([switch]$Existing, [switch]$DecompileCandidates, [switch]$SaveSurvey, [switch]$DecompileSavePaths)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 're\GhidraMaster.ps1')
$image=Assert-GridUnpackedImage
$output=Join-Path $root 'local\phase3\loader-survey'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$extra=if($DecompileCandidates) { @('-postScript','GridProbe.java',(Join-Path $output 'function-candidates'),'0060bef0','006138d0') } else { @() }
if($SaveSurvey) { $extra += @('-postScript','GridSaveSurvey.java',(Join-Path $root 'local\autosave\static-survey')) }
if($DecompileSavePaths) { $extra += @('-postScript','GridProbe.java',(Join-Path $root 'local\autosave\static-survey\function-candidates'),'005b93a0','005b9440','005b9680','005b9750','008a1260','00a9e540') }
Invoke-GridHeadless ((Get-GridMasterArguments)+@('-postScript','GridLoaderSurvey.java',$output)+$extra+@('-log',(Join-Path $output 'ghidra.log'),'-scriptlog',(Join-Path $output 'script.log'))) | Out-Null
if($LASTEXITCODE -ne 0) { throw "Ghidra survey exited with $LASTEXITCODE" }
