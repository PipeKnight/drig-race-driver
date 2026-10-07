# Read-only hardware, tool and GRID crash evidence. Writes only to the project.
param([string]$OutDir = (Join-Path $PSScriptRoot '..\local\phase0'))
$ErrorActionPreference = 'Stop'
$OutDir = [IO.Path]::GetFullPath($OutDir)
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
function Save-Json($Name, $Value) {
    ConvertTo-Json -InputObject $Value -Depth 8 | Set-Content -LiteralPath (Join-Path $OutDir $Name) -Encoding utf8
}
$machine = [ordered]@{
    CollectedUtc = [DateTime]::UtcNow.ToString('o')
    OS = Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,OSArchitecture
    CPU = @(Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
    GPU = @(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,DriverDate,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate)
    Input = @(Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPClass -in @('HIDClass','Keyboard','Mouse') -and $_.Name -match 'controller|game|wheel|keyboard|mouse|joystick' } | Select-Object Name,PNPClass)
}
Save-Json 'machine.json' $machine
$events = @(Get-WinEvent -FilterHashtable @{LogName='Application';StartTime=(Get-Date).AddDays(-30);Id=1000,1001,1002,33} -ErrorAction SilentlyContinue | Where-Object { $_.Message -match 'grid.exe|Race Driver|\\Grid\\' } | Select-Object TimeCreated,Id,ProviderName,RecordId,Message)
Save-Json 'crash-events.json' $events
$names = 'rg','git','python','python3','py','rustup','rustc','cargo','java','javac','node','npm','rea','ghidraRun','analyzeHeadless','imhex','procmon','procmon64','procdump','x32dbg','x64dbg','apitrace','renderdoccmd','qrenderdoc','blender','dumpbin','llvm-readobj','cl','cmake','7z','windbg','cdb','winget'
$commands = @(foreach ($n in $names) { $c = Get-Command $n -ErrorAction SilentlyContinue; [pscustomobject]@{Name=$n;Paths=@($c | Select-Object -ExpandProperty Source)} })
$keys = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*','HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*'
$registered = @(Get-ItemProperty $keys -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match 'Ghidra|ImHex|Debug|Python|Rust|Git|Java|JDK|Node|Visual Studio|Windows.*SDK|RenderDoc|Blender|EGO|DirectX|Redistributable' } | Select-Object DisplayName,DisplayVersion,InstallLocation)
Save-Json 'tools.json' @{Commands=$commands;Registered=$registered;Note='Python WindowsApps paths are aliases, not proof of a working interpreter. Tools outside PATH and registry require a portable-file search.'}
$game = 'C:\Program Files (x86)\Steam\steamapps\common\Grid'
$exe = Get-Item -LiteralPath (Join-Path $game 'grid.exe')
Save-Json 'exe-version.json' ($exe.VersionInfo | Select-Object FileVersion,ProductVersion,ProductName,FileDescription,OriginalFilename)
$settings = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Codemasters\GRID\hardwaresettings'
New-Item -ItemType Directory -Path (Join-Path $OutDir 'settings-before') -Force | Out-Null
foreach ($name in 'hardware_settings_config.xml','hardware_settings_info.xml') {
    $file = Join-Path $settings $name
    if (Test-Path -LiteralPath $file) { Copy-Item -LiteralPath $file -Destination (Join-Path $OutDir 'settings-before') }
}
$manifest = Get-Content -LiteralPath 'C:\Program Files (x86)\Steam\steamapps\appmanifest_12750.acf' -Raw
# Do not retain the Steam owner's account ID in project evidence.
$manifest = $manifest -replace '(?m)^\s*"LastOwner"[^\r\n]*[\r\n]+', ''
Set-Content -LiteralPath (Join-Path $OutDir 'appmanifest_12750.redacted.acf') -Value $manifest -Encoding utf8
$layers = @(foreach ($key in 'HKCU:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers','HKLM:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers') {
    if (Test-Path $key) {
        $item = Get-ItemProperty $key
        foreach ($property in $item.PSObject.Properties) {
            if ($property.Name -match '(?i)\\grid\\grid\.exe$') { [pscustomobject]@{Key=$key;Path=$property.Name;Flags=$property.Value} }
        }
    }
})
Save-Json 'compatibility-layers.json' $layers
Write-Output "Saved machine, events, tool inventory, version, compatibility layers and existing settings to $OutDir"
