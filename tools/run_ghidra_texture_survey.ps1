$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$image=Join-Path $root 'local\compatibility\grid-unpacked.exe'
if((Get-FileHash -LiteralPath $image).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514') {throw 'Private unpacked reference hash mismatch'}
$out=Join-Path $root 'local\scene\texture-survey'
if(Test-Path -LiteralPath $out){throw 'Refusing to replace texture survey evidence'}
New-Item -ItemType Directory -Path $out -Force | Out-Null
$taskOldJava=$env:JAVA_HOME
try {
 $env:JAVA_HOME='C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre'
 & 'C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat' (Join-Path $root 'local\phase3\ghidra-project') 'grid-unpacked' '-process' 'grid-unpacked.exe' '-noanalysis' '-scriptPath' (Join-Path $PSScriptRoot 'ghidra') '-postScript' 'GridTextureSurvey.java' $out '-log' (Join-Path $out 'ghidra.log') '-scriptlog' (Join-Path $out 'script.log')
 if($LASTEXITCODE -ne 0){throw "Texture survey failed: $LASTEXITCODE"}
} finally {$env:JAVA_HOME=$taskOldJava}
