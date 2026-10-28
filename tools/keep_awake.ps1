<#
.SYNOPSIS
Temporarily prevent automatic system sleep with a renewable, expiring Windows power request.
.DESCRIPTION
Run -Minutes 90 to start or renew this checkout's lease; -Status inspects it, -Stop releases it.
The screen may still turn off. No power-plan settings, wake timers or installed services change.
Workers are hidden, hold an exclusive lock, and release their request on expiry or process exit.
Renew before long work; a lease is not proof that the agent or a build is still working.
#>
[CmdletBinding()]
param(
    [ValidateRange(0.02,240)][double]$Minutes=90,
    [switch]$Status,
    [switch]$Stop,
    [switch]$Worker
)
$ErrorActionPreference='Stop'
$repo = Split-Path $PSScriptRoot -Parent
$stateDir = Join-Path $repo 'local/keep-awake'
$leasePath = Join-Path $stateDir 'lease.json'
$workerPath = Join-Path $stateDir 'worker.json'
New-Item -ItemType Directory -Path $stateDir -Force | Out-Null
function Read-State($Path) {
    if (Test-Path -LiteralPath $Path) { Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json }
}
function Write-Lease([DateTime]$Until) {
    $temp = Join-Path $stateDir ("lease-$PID.tmp")
    @{expires_utc=$Until.ToUniversalTime().ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath $temp
    Move-Item -LiteralPath $temp -Destination $leasePath -Force
}
function Active-Worker {
    $s = Read-State $workerPath
    if (!$s) {return $false}
    $p = Get-Process -Id $s.pid -ErrorAction SilentlyContinue
    return ($p -and $p.StartTime.ToUniversalTime().ToString('o') -eq $s.started_utc)
}
if ($Status) {
    [pscustomobject]@{worker_alive=[bool](Active-Worker);lease=(Read-State $leasePath);worker=(Read-State $workerPath)} | ConvertTo-Json -Depth 5
    exit 0
}
if ($Stop) {
    Write-Lease ([DateTime]::UtcNow)
    Write-Output 'Sleep lease cancelled; worker releases within five seconds.'
    exit 0
}
if (!$Worker) {
    Write-Lease ([DateTime]::UtcNow.AddMinutes($Minutes))
    if (!(Active-Worker)) {
        $arguments = '-NoProfile -ExecutionPolicy Bypass -File "{0}" -Worker' -f $PSCommandPath
        Start-Process powershell -ArgumentList $arguments -WindowStyle Hidden -RedirectStandardOutput (Join-Path $stateDir 'worker.log') -RedirectStandardError (Join-Path $stateDir 'worker.err') | Out-Null
    }
    Write-Output "Sleep lease renewed for $Minutes minutes; display timeout unchanged."
    exit 0
}
$lock = $null
$request = [IntPtr]::Zero
try {
    try {$lock=[IO.File]::Open((Join-Path $stateDir 'worker.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)} catch [IO.IOException] {exit 0}
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class DrigPowerLease {
 [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
 public struct Reason { public uint Version; public uint Flags; [MarshalAs(UnmanagedType.LPWStr)] public string Text; }
 [DllImport("kernel32.dll", SetLastError=true)] public static extern IntPtr PowerCreateRequest(ref Reason reason);
 [DllImport("kernel32.dll", SetLastError=true)] [return:MarshalAs(UnmanagedType.Bool)] public static extern bool PowerSetRequest(IntPtr request, int type);
 [DllImport("kernel32.dll", SetLastError=true)] [return:MarshalAs(UnmanagedType.Bool)] public static extern bool PowerClearRequest(IntPtr request, int type);
 [DllImport("kernel32.dll")] [return:MarshalAs(UnmanagedType.Bool)] public static extern bool CloseHandle(IntPtr handle);
}
"@
    $reason=New-Object DrigPowerLease+Reason
    $reason.Version=0;$reason.Flags=1;$reason.Text='GRID graphics: temporary work lease'
    $request=[DrigPowerLease]::PowerCreateRequest([ref]$reason)
    if ($request -eq [IntPtr]::Zero -or $request -eq [IntPtr](-1)) {throw 'PowerCreateRequest failed'}
    if (![DrigPowerLease]::PowerSetRequest($request,1)) {throw 'PowerSetRequest(SystemRequired) failed'}
    $self=Get-Process -Id $PID
    @{pid=$PID;started_utc=$self.StartTime.ToUniversalTime().ToString('o');system_request=$true;display_request=$false} | ConvertTo-Json | Set-Content -LiteralPath $workerPath
    while ($true) {
        $lease=Read-State $leasePath
        if (!$lease -or [DateTime]::Parse($lease.expires_utc).ToUniversalTime() -le [DateTime]::UtcNow) {break}
        Start-Sleep -Seconds 5
    }
} finally {
    if ($request -ne [IntPtr]::Zero -and $request -ne [IntPtr](-1)) {
        [DrigPowerLease]::PowerClearRequest($request,1) | Out-Null
        [DrigPowerLease]::CloseHandle($request) | Out-Null
    }
    if ($lock) {$lock.Dispose()}
}
