param(
 [string]$GameRoot='C:\Program Files (x86)\Steam\steamapps\common\Grid',
 [string]$DesktopDlaaConfig,
 [switch]$DryRun
)
$ErrorActionPreference='Stop'
# Deterministic local hashing: PS5 may inherit a PS7 Utility module without Get-FileHash.
function Get-TaskSha([string]$Path){
    $taskStream=[IO.File]::OpenRead($Path)
    $taskHash=[Security.Cryptography.SHA256]::Create()
    try{return [BitConverter]::ToString($taskHash.ComputeHash($taskStream)).Replace('-','').ToLowerInvariant()}
    finally{$taskHash.Dispose();$taskStream.Dispose()}
}
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskExecutable=Join-Path $taskRoot 'target/release/grid-app.exe'
foreach($taskProcess in @(Get-Process -Name grid-app -ErrorAction SilentlyContinue)) {
    if($taskProcess.Path -and [IO.Path]::GetFullPath($taskProcess.Path) -eq $taskExecutable) {
        throw 'The prototype is running. Keep that session open; rebuild after it exits.'
    }
}
# Optional desktop-only SDK paths, never an executable setup script.
$taskDlaa=$null
$taskFeatures=@()
if($DesktopDlaaConfig){
    # PS5 Path.IsPathRooted also accepts drive-relative/root-relative paths.
    function Resolve-TaskAbsolutePath([string]$Value){
        if($Value -notmatch '^(?:[A-Za-z]:[\\/]|[\\/]{2}[^\\/?\.][^\\/]*[\\/][^\\/]+(?:[\\/]|$))'){throw 'DLAA paths must be fully absolute Windows drive or UNC paths'}
        return [IO.Path]::GetFullPath($Value)
    }
    $taskConfigPath=Resolve-TaskAbsolutePath $DesktopDlaaConfig
    $taskConfigBytes=[IO.File]::ReadAllBytes($taskConfigPath)
    $taskHasher=[Security.Cryptography.SHA256]::Create()
    try{$taskConfigSha=[BitConverter]::ToString($taskHasher.ComputeHash($taskConfigBytes)).Replace('-','').ToLowerInvariant()}finally{$taskHasher.Dispose()}
    $taskConfig=([Text.Encoding]::UTF8.GetString($taskConfigBytes).TrimStart([char]0xFEFF)) | ConvertFrom-Json
    foreach($taskKey in @('sdk_root','vulkan_root','libclang_directory','license_text')){
        $taskValue=[string]$taskConfig.$taskKey
        $taskConfig.$taskKey=Resolve-TaskAbsolutePath $taskValue
    }
    if([string]::IsNullOrWhiteSpace([string]$taskConfig.bindgen_extra_clang_args)){throw 'DLAA bindgen Windows include arguments are required'}
    $taskDll=Join-Path $taskConfig.sdk_root 'lib/Windows_x86_64/rel/nvngx_dlss.dll'
    foreach($taskFile in @($taskDll,(Join-Path $taskConfig.sdk_root 'include/nvsdk_ngx.h'),(Join-Path $taskConfig.sdk_root 'lib/Windows_x86_64/x64/nvsdk_ngx_d.lib'),(Join-Path $taskConfig.vulkan_root 'Include/vulkan/vulkan.h'),(Join-Path $taskConfig.libclang_directory 'libclang.dll'),$taskConfig.license_text)){
        if(-not (Test-Path -LiteralPath $taskFile -PathType Leaf)){throw "DLAA prerequisite missing: $taskFile"}
    }
    $taskDllSha='8707e53b26c68c606b98bf31c223485ff30d310a261b1a36d48b2eaabc1507ec'
    if((Get-TaskSha $taskDll) -ne $taskDllSha){throw 'DLAA release DLL is not the pinned SDK310.5.3 runtime'}
    $taskLicenseSha=[string]$taskConfig.license_sha256
    if($taskLicenseSha -notmatch '^[0-9a-fA-F]{64}$' -or (Get-Item -LiteralPath $taskConfig.license_text).Length -eq 0 -or (Get-TaskSha $taskConfig.license_text) -ne $taskLicenseSha){throw 'DLAA license text is empty or differs from the explicit hash'}
    $taskFeatures=@('dlaa','paint-specular')
    $taskDlaa=@{config=$taskConfig;config_path=$taskConfigPath;config_sha256=$taskConfigSha;dll_source=$taskDll;dll_sha256=$taskDllSha;license_sha256=$taskLicenseSha.ToLowerInvariant()}
}
$taskFeatureArguments=if($taskDlaa){' --features dlaa,paint-specular'}else{''}
if($DryRun){
    [ordered]@{dry_run=$true;features=@($taskFeatures);commands=@('test -p grid-tools --lib --release --offline source_driving -- --ignored','test -p grid-app --release --offline -- --ignored')+$(if($taskDlaa){@('clippy -p grid-app --release --offline --features dlaa,paint-specular --all-targets -- -D warnings','test -p grid-app --release --offline --features dlaa,paint-specular -- --include-ignored')}else{@()})+@('build -p grid-app --release --offline'+$taskFeatureArguments);executable=$taskExecutable;runtime_copies=@(if($taskDlaa){@(@{source=$taskDlaa.dll_source;destination=(Join-Path $taskRoot 'target/release/nvngx_dlss.dll');sha256=$taskDlaa.dll_sha256},@{source=$taskDlaa.config.license_text;destination=(Join-Path $taskRoot 'target/release/DLSS-LICENSE.txt');sha256=$taskDlaa.license_sha256})}else{@()});application_launches=0;artifact_writes=0} | ConvertTo-Json -Depth 6
    return
}
$taskCargoCommand=Get-Command cargo -ErrorAction SilentlyContinue
$taskRustcCommand=Get-Command rustc -ErrorAction SilentlyContinue
$taskCargo=if($taskCargoCommand){$taskCargoCommand.Source}else{Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'}
$taskRustc=if($taskRustcCommand){$taskRustcCommand.Source}else{Join-Path $env:USERPROFILE '.cargo/bin/rustc.exe'}
if(!(Test-Path -LiteralPath $taskCargo) -or !(Test-Path -LiteralPath $taskRustc)){
    throw 'The existing Rust toolchain was not found on PATH or in the standard user Cargo directory'
}
$taskKeys=@('DLSS_SDK','VULKAN_SDK','LIBCLANG_PATH','BINDGEN_EXTRA_CLANG_ARGS','PATH','CARGO_INCREMENTAL','GRID_GAME_ROOT','GRID_CONTACT_DIRECTIONS','GRID_BODY_EVENT_PARAMETERS','GRID_DEFORMATION_PARAMETERS','GRID_BUILD_REVISION')
$taskPrevious=@{}
foreach($taskKey in $taskKeys){$taskPrevious[$taskKey]=[Environment]::GetEnvironmentVariable($taskKey,'Process')}
# One cargo build at a time on this PC (tools/build_lock.ps1).
. (Join-Path $PSScriptRoot 'build_lock.ps1')
$taskLock=$null
Push-Location $taskRoot
try {
    if($taskDlaa){
        $env:DLSS_SDK=$taskDlaa.config.sdk_root
        $env:VULKAN_SDK=$taskDlaa.config.vulkan_root
        $env:LIBCLANG_PATH=$taskDlaa.config.libclang_directory
        $env:PATH=$env:LIBCLANG_PATH+';'+$env:PATH
        $env:BINDGEN_EXTRA_CLANG_ARGS=$taskDlaa.config.bindgen_extra_clang_args
    }
    $taskRevision=(& git rev-parse HEAD).Trim()
    if($LASTEXITCODE -ne 0){throw 'Cannot identify source revision'}
    $taskDirty=(@(& git status --porcelain).Count -ne 0)
    if($LASTEXITCODE -ne 0){throw 'Cannot inspect source status'}
    $env:GRID_BUILD_REVISION=$taskRevision.Substring(0,12)+$(if($taskDirty){'-dirty'}else{''})
    $env:CARGO_INCREMENTAL='0'
    $env:GRID_GAME_ROOT=[IO.Path]::GetFullPath($GameRoot)
    $env:GRID_CONTACT_DIRECTIONS=Join-Path $taskRoot 'local/body-contact-directions.bin'
    $env:GRID_BODY_EVENT_PARAMETERS=Join-Path $taskRoot 'local/body-impact-parameters.bin'
    $env:GRID_DEFORMATION_PARAMETERS=Join-Path $taskRoot 'local/deformation-parameters.bin'
    if(!(Test-Path -LiteralPath $env:GRID_CONTACT_DIRECTIONS)){throw 'Private body sampling file is required for the complete source checks'}
    if(!(Test-Path -LiteralPath $env:GRID_BODY_EVENT_PARAMETERS)){throw 'Private BP01 file is required; prepare it with grid-private-parameters (or tools/extract_body_impact_parameters.py)'}
    if(!(Test-Path -LiteralPath $env:GRID_DEFORMATION_PARAMETERS)){throw 'Private DF01 file is required; prepare it with grid-private-parameters (or tools/extract_deformation_parameters.py)'}
    $taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
    $taskEvidence=Join-Path $taskRoot "local/playtest/build-$taskStamp"
    New-Item -ItemType Directory -Path $taskEvidence | Out-Null
    # cmd.exe does the redirection: in Windows PowerShell 5.1, `*>` with
    # $ErrorActionPreference='Stop' turns cargo's first stderr line ("Compiling ...")
    # into a terminating NativeCommandError.
    function Invoke-TaskCargo([string]$taskLog, [string]$taskArguments) {
        & $env:ComSpec /d /s /c "`"`"$taskCargo`" $taskArguments > `"$taskLog`" 2>&1`""
        return $LASTEXITCODE
    }
    $taskLock=Enter-BuildLock -Purpose 'update_prototype'
    if((Invoke-TaskCargo (Join-Path $taskEvidence 'source-tests.txt') 'test -p grid-tools --lib --release --offline source_driving -- --ignored') -ne 0){throw "Source driving checks failed; see $taskEvidence"}
    if((Invoke-TaskCargo (Join-Path $taskEvidence 'app-tests.txt') 'test -p grid-app --release --offline -- --ignored') -ne 0){throw "App checks failed; see $taskEvidence"}
    if($taskDlaa){
        if((Invoke-TaskCargo (Join-Path $taskEvidence 'feature-clippy.txt') 'clippy -p grid-app --release --offline --features dlaa,paint-specular --all-targets -- -D warnings') -ne 0){throw "DLAA Clippy failed; see $taskEvidence"}
        if((Invoke-TaskCargo (Join-Path $taskEvidence 'feature-app-tests.txt') 'test -p grid-app --release --offline --features dlaa,paint-specular -- --include-ignored') -ne 0){throw "DLAA app checks failed; see $taskEvidence"}
    }
    if((Invoke-TaskCargo (Join-Path $taskEvidence 'build.txt') ('build -p grid-app --release --offline'+$taskFeatureArguments)) -ne 0){throw "Prototype build failed; see $taskEvidence"}
    $taskRuntime=@()
    if($taskDlaa){
        foreach($taskCopy in @(@{source=$taskDlaa.dll_source;name='nvngx_dlss.dll';sha256=$taskDlaa.dll_sha256},@{source=$taskDlaa.config.license_text;name='DLSS-LICENSE.txt';sha256=$taskDlaa.license_sha256})){
            if((Get-TaskSha $taskCopy.source) -ne $taskCopy.sha256){throw 'DLAA source runtime/license changed before copy'}
            $taskDestination=Join-Path (Split-Path -Parent $taskExecutable) $taskCopy.name
            if(Test-Path -LiteralPath $taskDestination){
                if((Get-TaskSha $taskDestination) -ne $taskCopy.sha256){throw "Existing DLAA runtime/license differs: $taskDestination"}
            }else{Copy-Item -LiteralPath $taskCopy.source -Destination $taskDestination -ErrorAction Stop}
            if((Get-TaskSha $taskDestination) -ne $taskCopy.sha256){throw 'DLAA runtime/license copy verification failed'}
            $taskRuntime+=@{file=$taskCopy.name;sha256=$taskCopy.sha256}
        }
        & $env:ComSpec /d /s /c "`"`"$taskExecutable`" --aa dlaa --help > `"$(Join-Path $taskEvidence 'dlaa-support.txt')`" 2>&1`""
        if($LASTEXITCODE -ne 0){throw "Built executable refused DLAA support; see $taskEvidence"}
    }
    if($taskDlaa -and (Get-TaskSha $taskDlaa.config_path) -ne $taskDlaa.config_sha256){throw 'DLAA config changed before build metadata publication'}
    $taskRecord=[ordered]@{
        built_utc=[DateTime]::UtcNow.ToString('o')
        revision=$taskRevision
        dirty=$taskDirty
        display_revision=$env:GRID_BUILD_REVISION
        desktop_features=@($taskFeatures)
        desktop_dlaa_config_sha256=$(if($taskDlaa){$taskDlaa.config_sha256}else{$null})
        desktop_runtime=@($taskRuntime)
        native_runtime_verified=$false
        executable=$taskExecutable
        executable_sha256=(Get-TaskSha $taskExecutable)
        cargo=(& $taskCargo --version)
        rustc=(& $taskRustc --version)
        source_driving_tests='passed'
        bevy_tests='passed'
        game_root=$env:GRID_GAME_ROOT
        contact_directions_sha256=(Get-TaskSha $env:GRID_CONTACT_DIRECTIONS)
        body_event_parameters_sha256=(Get-TaskSha $env:GRID_BODY_EVENT_PARAMETERS)
        deformation_parameters_sha256=(Get-TaskSha $env:GRID_DEFORMATION_PARAMETERS)
        evidence=$taskEvidence
        scope='Source driving and Bevy checks pass; rendering/manual lap require separate evidence.'
    }
    $taskJson=$taskRecord | ConvertTo-Json -Depth 5
    $taskJson | Set-Content -LiteralPath (Join-Path $taskEvidence 'build.json') -Encoding utf8
    $taskJson | Set-Content -LiteralPath (Join-Path $taskRoot 'local/playtest/current-build.json') -Encoding utf8
    Write-Output $taskJson
} finally {
    Exit-BuildLock $taskLock
    foreach($taskKey in $taskKeys){[Environment]::SetEnvironmentVariable($taskKey,$taskPrevious[$taskKey],'Process')}
    Pop-Location
}
