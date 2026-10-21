# Dot-source from tools/run_ghidra_*.ps1: runs Ghidra headless scripts on the shared master project
# (<main checkout>/local/ghidra-master, program grid-unpacked.exe) read-only. docs/re/README.md.
# Works from any worktree: the main checkout is found through git's common directory.
$GridGhidraHeadless='C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat'
$GridGhidraJdk='C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre'
$GridReRoot=$PSScriptRoot
$GridUnpackedSha256='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'

function Get-GridMainRoot {
    $common=& git -C $GridReRoot rev-parse --path-format=absolute --git-common-dir 2>$null
    if($LASTEXITCODE -ne 0 -or -not $common){ return [IO.Path]::GetFullPath((Join-Path $GridReRoot '..\..')) }
    return [IO.Path]::GetFullPath((Split-Path -Parent $common))
}

function Get-GridMasterProject { Join-Path (Get-GridMainRoot) 'local\ghidra-master' }

function Assert-GridUnpackedImage {
    $image=Join-Path (Get-GridMainRoot) 'local\compatibility\grid-unpacked.exe'
    if((Get-FileHash -LiteralPath $image -Algorithm SHA256).Hash.ToLower() -ne $GridUnpackedSha256){ throw 'Private PE identity mismatch' }
    return $image
}

function Test-GridProjectLocked([string]$ProjectDir,[string]$Name) {
    # Ghidra keeps <name>.lock and a channel lock on <name>.lock~; files left by a killed process are stale.
    $lock=Join-Path $ProjectDir "$Name.lock"; $channel=Join-Path $ProjectDir "$Name.lock~"
    if(-not (Test-Path -LiteralPath $lock)){ return $false }
    try {
        $stream=[IO.File]::Open($channel,'Open','ReadWrite','ReadWrite')
        try { $stream.Lock(0,1); $stream.Unlock(0,1); return $false } catch { return $true } finally { $stream.Dispose() }
    } catch { return $true }
}

function Wait-GridMasterUnlocked([int]$TimeoutSeconds=900) {
    $project=Get-GridMasterProject
    if(-not (Test-Path -LiteralPath (Join-Path $project 'grid-master.gpr'))){ throw "Master project missing: $project (py -I tools/re/master.py build)" }
    $deadline=(Get-Date).AddSeconds($TimeoutSeconds)
    while(Test-GridProjectLocked $project 'grid-master') {
        if((Get-Date) -gt $deadline){ throw 'Master Ghidra project is still locked by another process' }
        Write-Host 'waiting for grid-master.lock ...'; Start-Sleep -Seconds 10
    }
}

# Arguments for analyzeHeadless: master project, existing program, no analysis, nothing saved.
# Scripts may still create functions or labels in memory for their own run.
function Get-GridMasterArguments {
    Wait-GridMasterUnlocked
    return @((Get-GridMasterProject),'grid-master','-process','grid-unpacked.exe','-noanalysis','-readOnly','-scriptPath',[IO.Path]::GetFullPath((Join-Path $GridReRoot '..\ghidra')))
}

# Runs analyzeHeadless with empty stdin (launch.bat pauses after an error when started through cmd /c)
# and JAVA_HOME set to the validated JDK. Returns the output lines; $LASTEXITCODE is Ghidra's.
function Invoke-GridHeadless([string[]]$Arguments) {
    $previous=$env:JAVA_HOME
    $ErrorActionPreference='Continue'  # Java writes log lines to stderr; they are not failures
    try {
        $env:JAVA_HOME=$GridGhidraJdk
        '' | & $GridGhidraHeadless @Arguments 2>&1 | ForEach-Object { "$_" }
    } finally { $env:JAVA_HOME=$previous }
}
