# Bounded read-only snapshot of the original physics world scheduler and solver
# settings for one car: car (0xE85F28 list) -> +DB0 -> +8 physical -> +32C world
# -> +98 scheduler. Opens only VM_READ | QUERY_LIMITED_INFORMATION; no writes,
# injection or suspension. Output is private JSON under local/.
param(
 [Parameter(Mandatory=$true)][int]$ProcessId,
 [Parameter(Mandatory=$true)][string]$Output,
 [int]$CarIndex=0,
 [ValidateRange(1,200)][int]$Samples=1,
 [ValidateRange(0,2000)][int]$IntervalMs=0
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$out=[IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($Output)){$Output}else{Join-Path $root $Output}))
if(!$out.StartsWith((Join-Path $root 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Output must remain under local/'}
if(Test-Path -LiteralPath $out){throw 'Output already exists'}
$expectedPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$expectedHash='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
$process=Get-Process -Id $ProcessId
if($process.MainModule.FileName -ne $expectedPath){throw 'Unexpected executable path'}
if((Get-FileHash -LiteralPath $expectedPath).Hash.ToLower() -ne $expectedHash){throw 'Executable identity changed'}
if($process.MainModule.BaseAddress.ToInt64() -ne 0x400000){throw 'Unverified module relocation'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GridWorldSettingsReader {
 [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
 [DllImport("kernel32.dll", SetLastError=true)] static extern bool ReadProcessMemory(IntPtr h, IntPtr address, byte[] buffer, IntPtr size, out IntPtr read);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
 public static IntPtr Open(int pid) {
  IntPtr h = OpenProcess(0x0010 | 0x1000, false, pid);
  if (h == IntPtr.Zero) throw new InvalidOperationException("OpenProcess failed " + Marshal.GetLastWin32Error());
  return h;
 }
 public static byte[] Read(IntPtr h, long address, int size) {
  if (address < 0x10000 || address > 0x7fffffff || size < 1 || size > 0x10000) throw new InvalidOperationException("Read bound");
  var b = new byte[size]; IntPtr n;
  if (!ReadProcessMemory(h, new IntPtr(address), b, new IntPtr(size), out n) || n.ToInt64() != size) throw new InvalidOperationException("Read failed at " + address.ToString("x"));
  return b;
 }
 public static uint Ptr(IntPtr h, long address) { return BitConverter.ToUInt32(Read(h, address, 4), 0); }
}
'@
$h=[GridWorldSettingsReader]::Open($ProcessId)
try {
 $count=[GridWorldSettingsReader]::Ptr($h,0xe85fcc)
 if($count -lt 1 -or $count -gt 32 -or $CarIndex -ge $count){throw "Unexpected car count $count"}
 $snapshots=@()
 for($s=0;$s -lt $Samples;$s++){
  $car=[GridWorldSettingsReader]::Ptr($h,0xe85f28+4*$CarIndex)
  $wrapper=[GridWorldSettingsReader]::Ptr($h,$car+0xdb0)
  $physical=[GridWorldSettingsReader]::Ptr($h,$wrapper+8)
  $world=[GridWorldSettingsReader]::Ptr($h,$physical+0x32c)
  $scheduler=[GridWorldSettingsReader]::Ptr($h,$world+0x98)
  if([GridWorldSettingsReader]::Ptr($h,$scheduler+4) -ne $world){throw 'Scheduler back-link mismatch'}
  $sched=[GridWorldSettingsReader]::Read($h,$scheduler,0x40)
  $step=[GridWorldSettingsReader]::Read($h,$world+0x7c4,8)
  $snapshots+=[ordered]@{
   utc=[DateTime]::UtcNow.ToString('o')
   car=('{0:x}' -f $car); wrapper=('{0:x}' -f $wrapper); physical=('{0:x}' -f $physical)
   world=('{0:x}' -f $world); scheduler=('{0:x}' -f $scheduler)
   maxfps=[BitConverter]::ToSingle($sched,8); minfps=[BitConverter]::ToSingle($sched,12)
   maxTimesToRun=[BitConverter]::ToInt32($sched,16)
   world_7c4=[BitConverter]::ToUInt32($step,0); world_7c8=[BitConverter]::ToSingle($step,4)
   scheduler_hex=[BitConverter]::ToString($sched).Replace('-','')
   world_hex=$(if($s -eq 0){[BitConverter]::ToString([GridWorldSettingsReader]::Read($h,$world,0x800)).Replace('-','')}else{$null})
  }
  if($IntervalMs -gt 0 -and $s+1 -lt $Samples){Start-Sleep -Milliseconds $IntervalMs}
 }
 $record=[ordered]@{pid=$ProcessId;started_utc=$process.StartTime.ToUniversalTime().ToString('o');exe_sha256=$expectedHash;car_count=$count;car_index=$CarIndex;snapshots=$snapshots}
 [IO.File]::WriteAllText($out,($record | ConvertTo-Json -Depth 6))
 $snapshots | ForEach-Object { '{0} maxfps={1} minfps={2} maxTimesToRun={3} world+7C4={4} world+7C8={5}' -f $_.utc,$_.maxfps,$_.minfps,$_.maxTimesToRun,$_.world_7c4,$_.world_7c8 }
 "PRIVATE_OUTPUT=$out"
} finally { [void][GridWorldSettingsReader]::CloseHandle($h) }
