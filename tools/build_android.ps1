# Builds the Android debug APK of the prototype (P9, docs/ANDROID.md).
#
#   powershell -NoProfile -File tools\build_android.ps1              # code only
#   powershell -NoProfile -File tools\build_android.ps1 -BundleData  # PRIVATE: with game data
#
# 1. cargo ndk builds libgrid_android.so (release profile, arm64-v8a, API 31)
#    into android/app/build/rust-jniLibs;
# 2. the pinned Gradle runs assembleDebug in android/;
# 3. the APK is copied to local/android/apk/ with a manifest of the source
#    revision, tool versions and SHA-256 hashes.
# The default APK holds only our code; the data is pushed separately
# (tools/push_android_data.ps1).
#
# -BundleData packs the subset push_android_data.ps1 selects from the owner's
# own installation into the APK (assets/grid-data, with bundle.txt listing the
# files and a version hash). LauncherActivity copies it to the app's data
# folder on the first start. That APK contains proprietary game files: it is
# for the owner's own phone only, is named grid-<rev>-bundled-PRIVATE-debug.apk
# and stays in the ignored local/android/apk/; the staged assets stay in the
# ignored local/android/bundle/. Never commit or share either.
#
# Tool locations are the pinned installs of docs/TOOLCHAIN.md "Android
# toolchain"; each can be overridden by the matching environment variable.
param(
    [string]$Tools = 'C:\Users\debros\Tools',
    [switch]$BundleData,
    [string]$GameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Grid'
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))

function Pick([string]$name, [string]$default) {
    $value = [Environment]::GetEnvironmentVariable($name, 'Process')
    if ($value) { return $value }
    return $default
}
$sdk = Pick 'ANDROID_HOME' (Join-Path $Tools 'android-sdk')
$ndk = Pick 'ANDROID_NDK_HOME' (Join-Path $sdk 'ndk\30.0.16248370')
$jdk = Pick 'JAVA_HOME' (Join-Path $Tools 'jdk\jdk-21.0.12.1+1')
$gradle = Pick 'GRID_GRADLE' (Join-Path $Tools 'gradle\gradle-9.8.1\bin\gradle.bat')
$cargoNdkBin = Join-Path $Tools 'cargo-ndk-4.1.2\bin'
foreach ($path in @($sdk, $ndk, $jdk, $gradle, $cargoNdkBin)) {
    if (!(Test-Path -LiteralPath $path)) { throw "Missing Android tool: $path (docs/TOOLCHAIN.md)" }
}
$env:ANDROID_HOME = $sdk
$env:ANDROID_NDK_HOME = $ndk
$env:JAVA_HOME = $jdk
$env:GRADLE_USER_HOME = Pick 'GRADLE_USER_HOME' (Join-Path $Tools 'gradle-home')
$env:PATH = "$cargoNdkBin;$env:USERPROFILE\.cargo\bin;$jdk\bin;$env:PATH"

# Stages the private asset folder and writes grid-data/bundle.txt:
#   grid-bundle 1
#   version <sha256 over every file's hash, size, mode and path>
#   <size>\t<replace|keep>\t<path relative to the app's data folder>
# settings.ini is "keep": written only when missing, so the owner's edits stay.
function Stage-Bundle([string]$bundleRoot) {
    & git -C $root check-ignore -q $bundleRoot
    if ($LASTEXITCODE -ne 0) { throw "$bundleRoot is not ignored by git; refusing to stage game data there" }
    $data = Join-Path $bundleRoot 'grid-data'
    if (Test-Path -LiteralPath $bundleRoot) { Remove-Item -LiteralPath $bundleRoot -Recurse -Force }
    & powershell -NoProfile -File (Join-Path $root 'tools\push_android_data.ps1') -GameRoot $GameRoot -Stage $data | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'staging the game data failed' }
    $prefix = $data.TrimEnd('\').Length + 1
    $files = Get-ChildItem -LiteralPath $data -Recurse -File -Force |
        ForEach-Object { [pscustomobject]@{ Path = $_.FullName.Substring($prefix).Replace('\', '/'); File = $_ } } |
        Sort-Object -Property Path -CaseSensitive
    $lines = New-Object System.Collections.Generic.List[string]
    $hashed = New-Object System.Text.StringBuilder
    $bytes = [long]0
    foreach ($entry in $files) {
        $mode = if ($entry.Path -eq 'settings.ini') { 'keep' } else { 'replace' }
        $size = $entry.File.Length
        $bytes += $size
        $lines.Add("$size`t$mode`t$($entry.Path)")
        $sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $entry.File.FullName).Hash.ToLower()
        [void]$hashed.Append("$sha`t$size`t$mode`t$($entry.Path)`n")
    }
    $sha256 = [Security.Cryptography.SHA256]::Create()
    $version = -join ($sha256.ComputeHash([Text.Encoding]::UTF8.GetBytes($hashed.ToString())) | ForEach-Object { $_.ToString('x2') })
    $text = "grid-bundle 1`nversion $version`n" + (($lines -join "`n") + "`n")
    [IO.File]::WriteAllText((Join-Path $data 'bundle.txt'), $text)
    return [ordered]@{ version = $version; files = $lines.Count; bytes = $bytes }
}

Push-Location $root
try {
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify source revision' }
    $dirty = (@(& git status --porcelain).Count -ne 0)
    $env:GRID_BUILD_REVISION = $revision.Substring(0, 12) + $(if ($dirty) { '-dirty' } else { '' })
    $gradleArgs = @('-p', (Join-Path $root 'android'), '--no-daemon', '--console=plain')
    $bundle = $null
    if ($BundleData) {
        $bundleRoot = Join-Path $root 'local\android\bundle\assets'
        $bundle = Stage-Bundle $bundleRoot
        $gradleArgs += "-PgridBundleAssets=$bundleRoot"
    }
    $jni = Join-Path $root 'android\app\build\rust-jniLibs'
    if (Test-Path -LiteralPath $jni) { Remove-Item -LiteralPath $jni -Recurse -Force }
    $started = Get-Date
    & cargo ndk -t arm64-v8a -P 31 -o $jni build --release --lib -p grid-app
    if ($LASTEXITCODE -ne 0) { throw 'cargo ndk build failed' }
    $so = Join-Path $jni 'arm64-v8a\libgrid_android.so'
    if (!(Test-Path -LiteralPath $so)) { throw "cargo ndk did not produce $so" }
    # AGP's incremental debug packaging rewrites the old APK in place and can
    # leave dead space (a 172 MB file holding 90 MB of entries); start fresh.
    $apkDir = Join-Path $root 'android\app\build\outputs\apk'
    if (Test-Path -LiteralPath $apkDir) { Remove-Item -LiteralPath $apkDir -Recurse -Force }
    & $gradle @gradleArgs assembleDebug
    if ($LASTEXITCODE -ne 0) { throw 'Gradle assembleDebug failed' }
    $apk = Join-Path $root 'android\app\build\outputs\apk\debug\app-debug.apk'
    $outDir = Join-Path $root 'local\android\apk'
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    $name = if ($bundle) { "grid-$($env:GRID_BUILD_REVISION)-bundled-PRIVATE-debug.apk" } else { "grid-$($env:GRID_BUILD_REVISION)-debug.apk" }
    $out = Join-Path $outDir $name
    Copy-Item -LiteralPath $apk -Destination $out -Force
    $manifest = [ordered]@{
        revision = $env:GRID_BUILD_REVISION
        built_utc = [DateTime]::UtcNow.ToString('o')
        seconds = [int]((Get-Date) - $started).TotalSeconds
        apk = $out
        apk_bytes = (Get-Item -LiteralPath $out).Length
        apk_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $out).Hash.ToLower()
        so_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $so).Hash.ToLower()
        so_bytes = (Get-Item -LiteralPath $so).Length
        bundled_data = $bundle
        rustc = (& rustc --version)
        ndk = $ndk
        gradle = $gradle
        jdk = $jdk
    }
    $manifest | ConvertTo-Json | Set-Content -Encoding utf8 -LiteralPath "$out.json"
    $manifest | ConvertTo-Json
} finally {
    Pop-Location
}
