# Isolated game-copy experiment. Never patches the Steam installation.
param([Parameter(Mandatory=$true)][ValidateSet('baseline','unpacked','laa','vram','mtfix','legacy','utf8-manifest')][string]$Variant,[int]$Seconds=25,[ValidatePattern('^[a-z0-9-]+$')][string]$RunName)
$ErrorActionPreference='Stop'
$reference='C:\Program Files (x86)\Steam\steamapps\common\Grid'
$work='C:\Users\debros\Tools\grid-research\grid-compat'
if([IO.Path]::GetFullPath($work) -ne $work -or $work -eq $reference) { throw 'Invalid isolated target' }
if(Get-Process grid -ErrorAction SilentlyContinue) { throw 'GRID already running; refusing shared-settings experiment' }
$out=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\local\compatibility'))
$packages=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\local\tools\memory-fixes'))
$settings=Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Codemasters\GRID\hardwaresettings'
if(-not $RunName) { $RunName=$Variant }
$backup=Join-Path $out ($RunName+'-settings-before')
if(Test-Path -LiteralPath (Join-Path $out ($RunName+'.json'))) { throw 'Experiment name already recorded; preserve distinct attempts' }
New-Item -ItemType Directory -Path $backup -Force | Out-Null
$saved=@{}
foreach($f in Get-ChildItem -LiteralPath $settings -Filter '*.xml' -File) {
    $saved[$f.Name]=[IO.File]::ReadAllBytes($f.FullName)
    [IO.File]::WriteAllBytes((Join-Path $backup $f.Name),$saved[$f.Name])
}
$seen=@{}
$started=$null
$record=[ordered]@{Variant=$Variant;WorkCopy=$work;SystemACPRegistry=(Get-ItemProperty -LiteralPath 'HKLM:\SYSTEM\CurrentControlSet\Control\Nls\CodePage').ACP;ConfiguredSettingsReset=($Variant -eq 'mtfix');ReferenceExeSHA256=(Get-FileHash -LiteralPath (Join-Path $reference 'grid.exe')).Hash.ToLowerInvariant();Events=@();ProcessObservations=@()}
try {
    Copy-Item -LiteralPath (Join-Path $reference 'grid.exe') -Destination (Join-Path $work 'grid.exe') -Force
    foreach($name in 'hardware_settings_restrictions.xml','hardware_settings_options.xml','hardware_settings_ratings.xml') {
        Copy-Item -LiteralPath (Join-Path $reference ('system\'+$name)) -Destination (Join-Path $work ('system\'+$name)) -Force
    }
    if($Variant -in @('unpacked','laa','vram','mtfix','legacy','utf8-manifest')) {
        $unpacked=Join-Path $out 'grid-unpacked.exe'
        if(-not (Test-Path -LiteralPath $unpacked)) {
            $sl=Start-Process -FilePath (Join-Path $packages 'steamless\Steamless.CLI.exe') -ArgumentList ('"'+(Join-Path $work 'grid.exe')+'"') -WorkingDirectory $work -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $out 'steamless-stdout.txt') -RedirectStandardError (Join-Path $out 'steamless-stderr.txt')
            if(-not $sl.WaitForExit(20000)) { $sl.Kill(); throw 'Steamless did not complete' }
            $sl.WaitForExit()
            if(-not (Test-Path -LiteralPath (Join-Path $work 'grid.exe.unpacked.exe'))) { throw 'Steamless produced no unpacked executable; inspect logs' }
            Copy-Item -LiteralPath (Join-Path $work 'grid.exe.unpacked.exe') -Destination $unpacked
        }
        Copy-Item -LiteralPath $unpacked -Destination (Join-Path $work 'grid.exe') -Force
    }
    if($Variant -in @('laa','mtfix')) {
        $cachedLaa=Join-Path $out 'grid-laa-verified.exe'
        if(Test-Path -LiteralPath $cachedLaa) {
            $prior=Get-Content -LiteralPath (Join-Path $out 'laa.json') -Raw | ConvertFrom-Json
            if((Get-FileHash -LiteralPath $cachedLaa).Hash.ToLowerInvariant() -ne $prior.TestExeSHA256) { throw 'Cached NTCore output hash mismatch' }
            Copy-Item -LiteralPath $cachedLaa -Destination (Join-Path $work 'grid.exe') -Force
            $record.ReusedExactNTCoreOutput=$true
        } else {
        $nt=Start-Process -FilePath (Join-Path $packages 'ntcore\4gb_patch.exe') -ArgumentList ('"'+(Join-Path $work 'grid.exe')+'"') -WorkingDirectory $work -WindowStyle Hidden -PassThru
        $done=$nt.WaitForExit(10000)
        $record.NTCoreExitedWithinTenSeconds=$done
        if(-not $done) { $nt.Kill(); $nt.WaitForExit() }
        }
        $b=[IO.File]::ReadAllBytes((Join-Path $work 'grid.exe'))
        $pe=[BitConverter]::ToInt32($b,0x3c)
        $flags=[BitConverter]::ToUInt16($b,$pe+22)
        if(($flags -band 0x20) -eq 0) { throw 'NTCore tool did not set LAA flag' }
    }
    if($Variant -eq 'vram') {
        $restrictions=Join-Path $work 'system\hardware_settings_restrictions.xml'
        $text=[IO.File]::ReadAllText($restrictions)
        if(([regex]::Matches($text,'<res mem="270" maxWidth="1280" />')).Count -ne 1) { throw 'Expected one original VRAM restriction' }
        $text=$text.Replace('<res mem="270" maxWidth="1280" />','<res mem="4096" maxWidth="7000" />')
        [IO.File]::WriteAllText($restrictions,$text,[Text.UTF8Encoding]::new($false))
    }
    if($Variant -eq 'mtfix') {
        $files=Join-Path $packages 'gridmtfix\Files'
        foreach($f in Get-ChildItem -LiteralPath (Join-Path $files 'system') -File) { Copy-Item -LiteralPath $f.FullName -Destination (Join-Path $work 'system') -Force }
        $cores=(Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
        $gridCores=@(1,2,4,6,8,12,16,20,24,28,32) | Where-Object { $_ -le $cores } | Select-Object -Last 1
        $text=[IO.File]::ReadAllText((Join-Path $files 'Template.xml'))
        $text=$text -replace ('<!-- (?!'+('{0:D3}' -f $gridCores)+').*'),' '
        [IO.File]::WriteAllText((Join-Path $work 'system\hardware_settings_restrictions.xml'),$text,[Text.UTF8Encoding]::new($false))
        $record.WorkerMapCores=$gridCores
        # Apply upstream's reset to the actual Windows Documents location, not a guessed path.
        foreach($f in Get-ChildItem -LiteralPath $settings -Filter '*.xml' -File) { Remove-Item -LiteralPath $f.FullName }
    }
    if($Variant -in @('legacy','utf8-manifest')) {
        $page=if($Variant -eq 'legacy') { 'en-US' } else { 'UTF-8' }
        & 'C:\Users\debros\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe' (Join-Path $PSScriptRoot 'set_copy_codepage.py') --codepage $page
        if($LASTEXITCODE -ne 0) { throw 'Manifest modification failed' }
        $record.RequestedProcessCodepage=$page
    }
    $b=[IO.File]::ReadAllBytes((Join-Path $work 'grid.exe'))
    $pe=[BitConverter]::ToInt32($b,0x3c)
    $record.LAA=[bool]([BitConverter]::ToUInt16($b,$pe+22) -band 0x20)
    $record.TestExeSHA256=(Get-FileHash -LiteralPath (Join-Path $work 'grid.exe')).Hash.ToLowerInvariant()
    $started=Get-Date
    $record.LaunchUtc=$started.ToUniversalTime().ToString('o')
    $proc=Start-Process -FilePath (Join-Path $work 'grid.exe') -WorkingDirectory $work -PassThru
    $lastSeen=Get-Date
    $deadline=(Get-Date).AddSeconds($Seconds)
    do {
        $procs=@(Get-Process grid -ErrorAction SilentlyContinue)
        foreach($p in $procs) {
            if($p.StartTime -ge $started.AddSeconds(-1)) {
                $lastSeen=Get-Date
                if(-not $seen.ContainsKey($p.Id)) { $seen[$p.Id]=[pscustomobject]@{Id=$p.Id;Path=$p.Path;StartedUtc=$p.StartTime.ToUniversalTime().ToString('o')} }
                $record.LastWindowTitle=$p.MainWindowTitle
            }
        }
        Start-Sleep -Milliseconds 100
    } while((Get-Date) -lt $deadline -and ($seen.Count -eq 0 -or ((Get-Date)-$lastSeen).TotalSeconds -lt 3))
    $record.AliveAtDeadline=@(Get-Process grid -ErrorAction SilentlyContinue | Where-Object { $seen.ContainsKey($_.Id) }).Count -gt 0
    $record.MenuVerified=$false
    Start-Sleep -Seconds 2
    $record.Events=@(Get-WinEvent -FilterHashtable @{LogName='Application';StartTime=$started;Id=1000,1001,1002} -ErrorAction SilentlyContinue | Where-Object { $_.Message -match 'grid.exe' } | Select-Object TimeCreated,Id,RecordId,Message)
    $record.ExperimentCompleted=$true
} catch {
    $record.Error=$_.Exception.Message
    $record.ExperimentCompleted=$false
} finally {
    foreach($p in @(Get-Process grid -ErrorAction SilentlyContinue)) {
        if($seen.ContainsKey($p.Id)) { $p.CloseMainWindow() | Out-Null; if(-not $p.WaitForExit(3000)) { $p.Kill(); $p.WaitForExit() } }
    }
    foreach($f in Get-ChildItem -LiteralPath $settings -Filter '*.xml' -File) {
        Copy-Item -LiteralPath $f.FullName -Destination (Join-Path $out ($RunName+'-after-'+$f.Name)) -Force
        if(-not $saved.ContainsKey($f.Name)) { Remove-Item -LiteralPath $f.FullName }
    }
    $restored=$true
    foreach($name in $saved.Keys) {
        [IO.File]::WriteAllBytes((Join-Path $settings $name),$saved[$name])
        if((Get-FileHash -LiteralPath (Join-Path $settings $name)).Hash -ne (Get-FileHash -LiteralPath (Join-Path $backup $name)).Hash) { $restored=$false }
    }
    $record.SettingsRestored=$restored
    $record.ProcessObservations=@($seen.Values)
    $record.ReferenceExeUnchanged=(Get-FileHash -LiteralPath (Join-Path $reference 'grid.exe')).Hash.ToLowerInvariant() -eq $record.ReferenceExeSHA256
    $record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $out ($RunName+'.json')) -Encoding utf8
    [pscustomobject]@{Variant=$Variant;Completed=$record.ExperimentCompleted;Error=$record.Error;LAA=$record.LAA;Alive=$record.AliveAtDeadline;SettingsRestored=$restored;Processes=$record.ProcessObservations;CrashMessages=@($record.Events | Where-Object Id -eq 1000 | Select-Object -ExpandProperty Message)} | ConvertTo-Json -Depth 5
}
