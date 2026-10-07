param([switch]$Existing, [switch]$DecompileCandidates, [switch]$SaveSurvey, [switch]$DecompileSavePaths)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$ghidra='C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat'
$jdk='C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre'
$image=Join-Path $root 'local\compatibility\grid-unpacked.exe'
$expected='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
if ((Get-FileHash -LiteralPath $image -Algorithm SHA256).Hash.ToLower() -ne $expected) { throw 'Unpacked PE hash mismatch' }
$project=Join-Path $root 'local\phase3\ghidra-project'
$output=Join-Path $root 'local\phase3\loader-survey'
New-Item -ItemType Directory -Path $project,$output -Force | Out-Null
$oldJava=$env:JAVA_HOME
try {
 $env:JAVA_HOME=$jdk
 $mode=if($Existing) { @('-process','grid-unpacked.exe','-noanalysis') } else { @('-import',$image,'-analysisTimeoutPerFile','120','-max-cpu','4') }
 $extra=if($DecompileCandidates) { @('-postScript','GridProbe.java',(Join-Path $output 'function-candidates'),'0060bef0','006138d0') } else { @() }
 if($SaveSurvey) { $extra += @('-postScript','GridSaveSurvey.java',(Join-Path $root 'local\autosave\static-survey')) }
 if($DecompileSavePaths) { $extra += @('-postScript','GridProbe.java',(Join-Path $root 'local\autosave\static-survey\function-candidates'),'005b93a0','005b9440','005b9680','005b9750','008a1260','00a9e540') }
 & $ghidra $project 'grid-unpacked' @mode '-scriptPath' (Join-Path $PSScriptRoot 'ghidra') '-postScript' 'GridLoaderSurvey.java' $output @extra '-log' (Join-Path $output 'ghidra.log') '-scriptlog' (Join-Path $output 'script.log')
 if($LASTEXITCODE -ne 0) { throw "Ghidra survey exited with $LASTEXITCODE" }
} finally { $env:JAVA_HOME=$oldJava }
