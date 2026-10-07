# Install authorized native development prerequisites from official sources.
param([Parameter(Mandatory=$true)][ValidateSet('rust','msvc')][string]$Tool)
$ErrorActionPreference='Stop'
$dest=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\local\tools\native-installers'))
New-Item -ItemType Directory -Path $dest -Force | Out-Null
if($Tool -eq 'rust') {
    $url='https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
    $file=Join-Path $dest 'rustup-init.exe'
    Invoke-WebRequest $url -OutFile $file
    $digestResponse=(Invoke-WebRequest ($url+'.sha256')).Content
    if($digestResponse -is [byte[]]) { $digestResponse=[Text.Encoding]::ASCII.GetString($digestResponse) }
    $published=$digestResponse.Trim().Split(' ')[0]
    $hash=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    if($hash -ne $published) { throw 'Rustup official SHA-256 mismatch' }
    [ordered]@{Source=$url;SHA256=$hash;OfficialDigestMatched=$true;Arguments='-y --profile minimal --default-host x86_64-pc-windows-msvc --no-modify-path --component rustfmt,clippy'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dest 'rust-source.json') -Encoding utf8
    $process=Start-Process -FilePath $file -ArgumentList '-y','--profile','minimal','--default-host','x86_64-pc-windows-msvc','--no-modify-path','--component','rustfmt,clippy' -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $dest 'rust-stdout.log') -RedirectStandardError (Join-Path $dest 'rust-stderr.log')
} else {
    $url='https://aka.ms/vs/17/release/vs_buildtools.exe'
    $file=Join-Path $dest 'vs_buildtools.exe'
    Invoke-WebRequest $url -OutFile $file
    $sig=Get-AuthenticodeSignature -LiteralPath $file
    if($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'Microsoft Corporation') { throw 'Microsoft bootstrapper signature invalid' }
    $hash=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    $arguments=@('--quiet','--wait','--norestart','--nocache','--installPath','C:\BuildTools\GRID','--add','Microsoft.VisualStudio.Workload.VCTools','--add','Microsoft.VisualStudio.Component.VC.Tools.x86.x64','--add','Microsoft.VisualStudio.Component.Windows11SDK.26100')
    [ordered]@{Source=$url;SHA256=$hash;Signature=$sig.Status.ToString();Signer=$sig.SignerCertificate.Subject;FileVersion=(Get-Item -LiteralPath $file).VersionInfo.FileVersion;Arguments=$arguments} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dest 'msvc-source.json') -Encoding utf8
    $process=Start-Process -FilePath $file -ArgumentList $arguments -WindowStyle Hidden -PassThru
}
$process.WaitForExit()
[ordered]@{Tool=$Tool;FinishedUtc=[DateTime]::UtcNow.ToString('o');ExitCode=$process.ExitCode;RebootRequested=($process.ExitCode -eq 3010)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dest ($Tool+'-result.json')) -Encoding utf8
Get-Content -LiteralPath (Join-Path $dest ($Tool+'-result.json'))
if($process.ExitCode -notin @(0,3010)) { throw 'Installer failed; inspect preserved logs' }
