# Download official upstream packages for isolated compatibility experiments.
$ErrorActionPreference='Stop'
$dest=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\local\tools\memory-fixes'))
New-Item -ItemType Directory -Path $dest -Force | Out-Null
$headers=@{'User-Agent'='GRID-independent-engine-research'}
$gridRelease=Invoke-RestMethod 'https://api.github.com/repos/xatornet/GridMTFix/releases/tags/v0.3' -Headers $headers
$steamRelease=Invoke-RestMethod 'https://api.github.com/repos/atom0s/Steamless/releases/tags/v3.1.0.3' -Headers $headers
$commit=Invoke-RestMethod 'https://api.github.com/repos/xatornet/GridMTFix/commits/Main' -Headers $headers
Invoke-WebRequest ('https://raw.githubusercontent.com/xatornet/GridMTFix/'+$commit.sha+'/GridMTFixv3-Steam.ps1') -OutFile (Join-Path $dest 'upstream-GridMTFixv3-Steam.ps1')
$packages=@(
    @{Name='ntcore';Url='https://ntcore.com/files/4gb_patch.zip';Digest=$null;Version='inspect binary'},
    @{Name='steamless';Asset=($steamRelease.assets | Where-Object name -eq 'Steamless.v3.1.0.3.-.by.atom0s.zip');Version='3.1.0.3'},
    @{Name='gridmtfix';Asset=($gridRelease.assets | Where-Object name -eq 'GridMTFix-Resources.zip');Version='v0.3 resources'}
)
$records=@()
foreach($package in $packages) {
    if($package.Name -ne 'ntcore') {
        if(-not $package.Asset) { throw 'Expected official release asset missing' }
        $package.Url=$package.Asset.browser_download_url
        $package.Digest=$package.Asset.digest
    }
    $zip=Join-Path $dest ($package.Name+'.zip')
    $expanded=Join-Path $dest $package.Name
    if(Test-Path -LiteralPath $expanded) { throw "Already downloaded; inspect existing destination: $expanded" }
    Invoke-WebRequest -Uri $package.Url -OutFile $zip
    $hash=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if($package.Digest -and $package.Digest -ne ('sha256:'+$hash)) { throw 'Release asset digest mismatch' }
    Expand-Archive -LiteralPath $zip -DestinationPath $expanded
    $records += [pscustomobject]@{Name=$package.Name;Version=$package.Version;Url=$package.Url;SHA256=$hash;UpstreamDigest=$package.Digest;DigestMatched=[bool]$package.Digest;Destination=$expanded}
}
[ordered]@{CollectedUtc=[DateTime]::UtcNow.ToString('o');GridMTFixCommit=$commit.sha;Packages=$records} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $dest 'sources.json') -Encoding utf8
Get-Content -LiteralPath (Join-Path $dest 'sources.json')
