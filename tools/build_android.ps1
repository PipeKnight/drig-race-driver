# Builds the Android debug APK of the prototype (P9, docs/ANDROID.md).
#
#   powershell -NoProfile -File tools\build_android.ps1
#
# 1. cargo ndk builds libgrid_android.so (release profile, arm64-v8a, API 31)
#    into android/app/build/rust-jniLibs;
# 2. the pinned Gradle runs assembleDebug in android/;
# 3. the APK is copied to local/android/apk/ with a manifest of the source
#    revision, tool versions and SHA-256 hashes.
# The APK holds only our code. No game file, private table or derived cache is
# packaged; the data is pushed separately (tools/push_android_data.ps1).
# Tool locations are the pinned installs of docs/TOOLCHAIN.md "Android
# toolchain"; each can be overridden by the matching environment variable.
param([string]$Tools = 'C:\Users\debros\Tools')
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

Push-Location $root
try {
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot identify source revision' }
    $dirty = (@(& git status --porcelain).Count -ne 0)
    $env:GRID_BUILD_REVISION = $revision.Substring(0, 12) + $(if ($dirty) { '-dirty' } else { '' })
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
    & $gradle -p (Join-Path $root 'android') --no-daemon --console=plain assembleDebug
    if ($LASTEXITCODE -ne 0) { throw 'Gradle assembleDebug failed' }
    $apk = Join-Path $root 'android\app\build\outputs\apk\debug\app-debug.apk'
    $outDir = Join-Path $root 'local\android\apk'
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    $name = "grid-$($env:GRID_BUILD_REVISION)-debug.apk"
    $out = Join-Path $outDir $name
    Copy-Item -LiteralPath $apk -Destination $out -Force
    $manifest = [ordered]@{
        revision = $env:GRID_BUILD_REVISION
        built_utc = [DateTime]::UtcNow.ToString('o')
        seconds = [int]((Get-Date) - $started).TotalSeconds
        apk = $out
        apk_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $out).Hash.ToLower()
        so_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $so).Hash.ToLower()
        so_bytes = (Get-Item -LiteralPath $so).Length
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
