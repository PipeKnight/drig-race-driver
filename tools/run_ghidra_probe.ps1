# Decompile four independently located functions (GridProbe.java defaults) or the given -Entries.
# Default: the shared master project (read-only). Its .text equals the runtime image's except 16
# bytes at 0xC883F0, so code reads the same. -RuntimeImage uses the captured runtime image instead
# (local/phase1/runtime/grid-runtime-image.bin: runtime-initialised .rdata/.data and imports),
# imported once without analysis into the cache project local/re-cache/ghidra-runtime.
param([switch]$RuntimeImage, [switch]$Existing, [string[]]$Entries=@())
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 're\GhidraMaster.ps1')
$output=Join-Path $root 'local\phase1\decompilation'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$probe=@('-postScript','GridProbe.java',$output)+$Entries+@('-log',(Join-Path $output 'ghidra.log'),'-scriptlog',(Join-Path $output 'ghidra-script.log'))
if($RuntimeImage) {
    $main=Get-GridMainRoot
    $image=Join-Path $main 'local\phase1\runtime\grid-runtime-image.bin'
    $project=Join-Path $main 'local\re-cache\ghidra-runtime'
    New-Item -ItemType Directory -Path $project -Force | Out-Null
    if(Test-GridProjectLocked $project 'grid-runtime') { throw 'Runtime cache project is in use' }
    $imported=Test-Path -LiteralPath (Join-Path $project 'grid-runtime.gpr')
    $mode=if($imported) { @('-process','grid-runtime-image.bin','-readOnly') } else { @('-import',$image,'-loader','BinaryLoader','-loader-baseAddr','0x00400000','-processor','x86:LE:32:default','-cspec','windows') }
    $arguments=@($project,'grid-runtime')+$mode+@('-noanalysis','-scriptPath',[IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'ghidra')))+$probe
} else {
    $arguments=(Get-GridMasterArguments)+$probe
}
Invoke-GridHeadless $arguments | Out-Null
if($LASTEXITCODE -ne 0) { throw "Ghidra exited with $LASTEXITCODE" }
Write-Output "PRIVATE_OUTPUT=$output"
