$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$out=Join-Path $root 'local\autosave\codepage-probes'
New-Item -ItemType Directory -Path $out -Force | Out-Null
$compiler='C:\Users\debros\.rustup\toolchains\1.99.0-x86_64-pc-windows-msvc\bin\rustc.exe'
$results=@()
foreach($page in @('en-US','uk-UA','UTF-8')) {
 $manifest=Join-Path $out ($page+'.manifest')
 $exe=Join-Path $out ('documents-'+$page+'.exe')
 $xml='<?xml version="1.0" encoding="UTF-8" standalone="yes"?><assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0"><assemblyIdentity version="1.0.0.0" processorArchitecture="amd64" name="GRID.Research.Documents" type="win32"/><application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings><activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">'+$page+'</activeCodePage></windowsSettings></application></assembly>'
 [IO.File]::WriteAllText($manifest,$xml,[Text.UTF8Encoding]::new($false))
 & $compiler '--edition=2024' (Join-Path $PSScriptRoot 'windows\document_codepages.rs') '-o' $exe '-C' 'link-arg=/MANIFEST:EMBED' '-C' ('link-arg=/MANIFESTINPUT:'+ $manifest)
 if($LASTEXITCODE -ne 0) { throw "Probe compile failed for $page" }
 $output=& $exe
 if($LASTEXITCODE -ne 0) { throw "Probe execution failed for $page" }
 $output | Set-Content -LiteralPath (Join-Path $out ($page+'.txt')) -Encoding utf8
 $results += [PSCustomObject]@{RequestedCodepage=$page;Output=$output;ExeSHA256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLower()}
}
$results | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $out 'results.json') -Encoding utf8
$results | ConvertTo-Json -Depth 6
