param([string]$GameRoot='C:\Program Files (x86)\Steam\steamapps\common\Grid')
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskExecutable=Join-Path $taskRoot 'target/release/grid-app.exe'
foreach($taskProcess in @(Get-Process -Name grid-app -ErrorAction SilentlyContinue)) {
    if($taskProcess.Path -and [IO.Path]::GetFullPath($taskProcess.Path) -eq $taskExecutable) {
        throw 'The prototype is running. Keep that session open; rebuild after it exits.'
    }
}
$taskCargoCommand=Get-Command cargo -ErrorAction SilentlyContinue
$taskRustcCommand=Get-Command rustc -ErrorAction SilentlyContinue
$taskCargo=if($taskCargoCommand){$taskCargoCommand.Source}else{Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'}
$taskRustc=if($taskRustcCommand){$taskRustcCommand.Source}else{Join-Path $env:USERPROFILE '.cargo/bin/rustc.exe'}
if(!(Test-Path -LiteralPath $taskCargo) -or !(Test-Path -LiteralPath $taskRustc)){
    throw 'The existing Rust toolchain was not found on PATH or in the standard user Cargo directory'
}
$taskKeys=@('CARGO_INCREMENTAL','GRID_GAME_ROOT','GRID_CONTACT_DIRECTIONS','GRID_BUILD_REVISION')
$taskPrevious=@{}
foreach($taskKey in $taskKeys){$taskPrevious[$taskKey]=[Environment]::GetEnvironmentVariable($taskKey,'Process')}
Push-Location $taskRoot
try {
    $taskRevision=(& git rev-parse HEAD).Trim()
    if($LASTEXITCODE -ne 0){throw 'Cannot identify source revision'}
    $taskDirty=(@(& git status --porcelain).Count -ne 0)
    if($LASTEXITCODE -ne 0){throw 'Cannot inspect source status'}
    $env:GRID_BUILD_REVISION=$taskRevision.Substring(0,12)+$(if($taskDirty){'-dirty'}else{''})
    $env:CARGO_INCREMENTAL='0'
    $env:GRID_GAME_ROOT=[IO.Path]::GetFullPath($GameRoot)
    $env:GRID_CONTACT_DIRECTIONS=Join-Path $taskRoot 'local/body-contact-directions.bin'
    if(!(Test-Path -LiteralPath $env:GRID_CONTACT_DIRECTIONS)){throw 'Private body sampling file is required for the complete source checks'}
    $taskStamp=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ')
    $taskEvidence=Join-Path $taskRoot "local/playtest/build-$taskStamp"
    New-Item -ItemType Directory -Path $taskEvidence | Out-Null
    & $taskCargo test -p grid-tools --lib --release --offline source_driving -- --ignored *> (Join-Path $taskEvidence 'source-tests.txt')
    if($LASTEXITCODE -ne 0){throw "Source driving checks failed; see $taskEvidence"}
    & $taskCargo test -p grid-app --release --offline -- --ignored *> (Join-Path $taskEvidence 'app-tests.txt')
    if($LASTEXITCODE -ne 0){throw "App checks failed; see $taskEvidence"}
    & $taskCargo build -p grid-app --release --offline *> (Join-Path $taskEvidence 'build.txt')
    if($LASTEXITCODE -ne 0){throw "Prototype build failed; see $taskEvidence"}
    $taskRecord=[ordered]@{
        built_utc=[DateTime]::UtcNow.ToString('o')
        revision=$taskRevision
        dirty=$taskDirty
        display_revision=$env:GRID_BUILD_REVISION
        executable=$taskExecutable
        executable_sha256=(Get-FileHash -LiteralPath $taskExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
        cargo=(& $taskCargo --version)
        rustc=(& $taskRustc --version)
        source_driving_tests='passed'
        bevy_tests='passed'
        game_root=$env:GRID_GAME_ROOT
        contact_directions_sha256=(Get-FileHash -LiteralPath $env:GRID_CONTACT_DIRECTIONS -Algorithm SHA256).Hash.ToLowerInvariant()
        evidence=$taskEvidence
        scope='Source driving and Bevy checks pass; rendering/manual lap require separate evidence.'
    }
    $taskJson=$taskRecord | ConvertTo-Json -Depth 5
    $taskJson | Set-Content -LiteralPath (Join-Path $taskEvidence 'build.json') -Encoding utf8
    $taskJson | Set-Content -LiteralPath (Join-Path $taskRoot 'local/playtest/current-build.json') -Encoding utf8
    Write-Output $taskJson
} finally {
    foreach($taskKey in $taskKeys){[Environment]::SetEnvironmentVariable($taskKey,$taskPrevious[$taskKey],'Process')}
    Pop-Location
}
