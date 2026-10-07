# Import a captured runtime image and decompile four independently located functions.
param([switch]$Existing)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$ghidra='C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat'
if(-not(Test-Path -LiteralPath $ghidra)) { $ghidra=Join-Path $root 'local\tools\ghidra\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat' }
$jdk='C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre'
$project=Join-Path $root 'local\phase1\ghidra-project'
$output=Join-Path $root 'local\phase1\decompilation'
$image=Join-Path $root 'local\phase1\runtime\grid-runtime-image.bin'
if(-not(Test-Path -LiteralPath $ghidra)) { throw 'Pinned Ghidra release not installed' }
if(-not(Test-Path -LiteralPath (Join-Path $jdk 'bin\javac.exe'))) { throw 'Validated JDK is missing' }
New-Item -ItemType Directory -Path $project,$output -Force | Out-Null
$oldJava=$env:JAVA_HOME
try {
    $env:JAVA_HOME=$jdk
    $mode=if($Existing) { @('-process','grid-runtime-image.bin') } else { @('-import',$image,'-loader','BinaryLoader','-loader-baseAddr','0x00400000','-processor','x86:LE:32:default','-cspec','windows') }
    & $ghidra $project 'grid-runtime' @mode '-noanalysis' '-scriptPath' (Join-Path $PSScriptRoot 'ghidra') '-postScript' 'GridProbe.java' $output '-log' (Join-Path $root 'local\phase1\ghidra.log') '-scriptlog' (Join-Path $root 'local\phase1\ghidra-script.log')
    if($LASTEXITCODE -ne 0) { throw "Ghidra exited with $LASTEXITCODE" }
} finally {
    $env:JAVA_HOME=$oldJava
}
