# Download official pinned research packages into ignored project-local storage.
param([Parameter(Mandatory=$true)][ValidateSet('ghidra','x64dbg','procmon','autohotkey')][string]$Tool)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\local\tools'))
$packages=@{
    ghidra=@{Url='https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_12.1.4_build/ghidra_12.1.4_PUBLIC_20260921.zip';Version='12.1.4';SHA256='ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db'}
    x64dbg=@{Url='https://github.com/x64dbg/x64dbg/releases/download/2026.05.27/snapshot_2026-05-27_12-11.zip';Version='2026.05.27';SHA256='d41966dfc5b435a372798245300ca0ab7bb8e48bdbf48512c6fb20fcca427697'}
    procmon=@{Url='https://download.sysinternals.com/files/ProcessMonitor.zip';Version='verify extracted binary';SHA256=$null}
    autohotkey=@{Url='https://github.com/AutoHotkey/AutoHotkey/releases/download/v2.0.29/AutoHotkey_2.0.29.zip';Version='2.0.29';SHA256='b2d0200724a6b6ad22c965c939c5e5a2c64a35d1ccb455a3ca3f8ce415c5a296'}
}
$package=$packages[$Tool]
$zip=Join-Path $root ($Tool+'.zip')
$dest=Join-Path $root $Tool
if(Test-Path -LiteralPath $dest) { throw "Destination already exists: $dest. Inspect it instead of overwriting." }
New-Item -ItemType Directory -Path $root -Force | Out-Null
Invoke-WebRequest -Uri $package.Url -OutFile $zip
$hash=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
if($package.SHA256 -and $hash -ne $package.SHA256) { throw "Official release digest mismatch for $Tool" }
Expand-Archive -LiteralPath $zip -DestinationPath $dest
$signed=@()
if($Tool -eq 'procmon') {
    foreach($file in Get-ChildItem -LiteralPath $dest -Filter '*.exe') {
        $sig=Get-AuthenticodeSignature -LiteralPath $file.FullName
        if($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'Microsoft Corporation') { throw "Signature verification failed: $($file.Name)" }
        $signed += [pscustomobject]@{Name=$file.Name;Version=$file.VersionInfo.FileVersion;Status=$sig.Status.ToString();Signer=$sig.SignerCertificate.Subject}
    }
}
$record=[ordered]@{Tool=$Tool;Version=$package.Version;Source=$package.Url;CollectedUtc=[DateTime]::UtcNow.ToString('o');ZipSHA256=$hash;OfficialReleaseDigestMatched=[bool]$package.SHA256;Signatures=$signed;Destination=$dest}
$record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root ($Tool+'-source.json')) -Encoding utf8
$record | ConvertTo-Json -Depth 5
