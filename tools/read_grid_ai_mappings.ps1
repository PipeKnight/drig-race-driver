# Read-only snapshot of original AI driver-parameter objects (NEFloat mapping
# objects, vtable 0xCF9068) plus AI driver/vehicle config objects (vtables
# 0xCF8C08 / 0xCF7EF8) in the private GRID research process.
# Opens only VM_READ | QUERY_LIMITED_INFORMATION; no writes, injection,
# suspension, input or VirtualQueryEx. Unreadable 64 KiB windows are skipped.
# Output is private JSON under local/.
param(
 [Parameter(Mandatory=$true)][int]$ProcessId,
 [Parameter(Mandatory=$true)][string]$Output,
 [long]$ScanStart=0x10000,
 [long]$ScanEnd=0x7fff0000,
 [long[]]$ExtraNeedles=@()
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$out=[IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($Output)){$Output}else{Join-Path $root $Output}))
if(!$out.StartsWith((Join-Path $root 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Output must remain under local/'}
if(Test-Path -LiteralPath $out){throw 'Output already exists'}
if($ScanStart -lt 0x10000 -or $ScanEnd -gt 4294901760 -or $ScanEnd -le $ScanStart){throw 'Scan bounds'}
$expectedPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$expectedHash='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
$process=Get-Process -Id $ProcessId
if($process.MainModule.FileName -ne $expectedPath){throw 'Unexpected executable path'}
if((Get-FileHash -LiteralPath $expectedPath).Hash.ToLower() -ne $expectedHash){throw 'Executable identity changed'}
if($process.MainModule.BaseAddress.ToInt64() -ne 0x400000){throw 'Unverified module relocation'}
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class GridAiMappingReader {
 [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
 [DllImport("kernel32.dll", SetLastError=true)] static extern bool ReadProcessMemory(IntPtr h, IntPtr address, byte[] buffer, IntPtr size, out IntPtr read);
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
 public static IntPtr Open(int pid) {
  IntPtr h = OpenProcess(0x0010 | 0x1000, false, pid);
  if (h == IntPtr.Zero) throw new InvalidOperationException("OpenProcess failed " + Marshal.GetLastWin32Error());
  return h;
 }
 public static byte[] TryRead(IntPtr h, long address, int size) {
  if (address < 0x10000 || size < 1 || size > 0x10000) throw new InvalidOperationException("Read bound");
  var b = new byte[size]; IntPtr n;
  if (!ReadProcessMemory(h, new IntPtr(address), b, new IntPtr(size), out n) || n.ToInt64() != size) return null;
  return b;
 }
 static void Match(byte[] b, long a, uint[] needles, List<long[]> hits, int limit) {
  for (int i = 0; i + 4 <= b.Length; i += 4) {
   uint v = BitConverter.ToUInt32(b, i);
   for (int k = 0; k < needles.Length; k++) if (v == needles[k]) { hits.Add(new long[] { a + i, k }); if (hits.Count > limit) throw new InvalidOperationException("Hit limit"); }
  }
 }
 public static List<long[]> Scan(IntPtr h, long start, long end, uint[] needles, int limit) {
  var hits = new List<long[]>();
  for (long a = start; a < end; a += 0x10000) {
   var b = TryRead(h, a, 0x10000);
   if (b != null) { Match(b, a, needles, hits, limit); continue; }
   for (long p = a; p < a + 0x10000; p += 0x1000) { var c = TryRead(h, p, 0x1000); if (c != null) Match(c, p, needles, hits, limit); }
  }
  return hits;
 }
}
'@
$h=[GridAiMappingReader]::Open($ProcessId)
try {
 # Verify that the live image code at the mapping evaluator matches the analysis image.
 $raw=[IO.File]::ReadAllBytes((Join-Path $root 'local/compatibility/grid-unpacked.exe'))
 foreach($entry in @(0x7a7a00,0x77a4c0,0x7886c0,0x77ac50)){
  $live=[GridAiMappingReader]::TryRead($h,$entry,32); $off=$entry-0x400000-0x1000+0x400
  if($null -eq $live -or [BitConverter]::ToString($live).Replace('-','') -ne [BitConverter]::ToString([byte[]]$raw[$off..($off+31)]).Replace('-','')){throw ('Live code differs at {0:x}' -f $entry)}
 }
 $needles=[uint32[]](@(0xcf9068,0xcf8c08,0xcf7ef8)+@($ExtraNeedles | ForEach-Object {[uint32]$_}))
 $sw=[Diagnostics.Stopwatch]::StartNew()
 $hits=[GridAiMappingReader]::Scan($h,$ScanStart,$ScanEnd,$needles,20000)
 $mappings=@(); $driverConfigs=@(); $vehicleConfigs=@()
 foreach($hit in $hits){
  $addr=$hit[0]; $kind=$hit[1]
  if($kind -eq 0){
   $b=[GridAiMappingReader]::TryRead($h,$addr,0x70); if($null -eq $b){continue}
   $cap=[BitConverter]::ToUInt32($b,0x18); $ptr=[BitConverter]::ToUInt32($b,0x1c)
   if($cap -ne 0x40 -or $ptr -ne $addr+0x20){continue}
   $nameBytes=$b[0x20..0x5f]; $z=[Array]::IndexOf([byte[]]$nameBytes,[byte]0); if($z -lt 0){$z=64}
   $name=[Text.Encoding]::ASCII.GetString([byte[]]$nameBytes,0,$z)
   $lb=[BitConverter]::ToUInt32($b,0x64); $le=[BitConverter]::ToUInt32($b,0x68)
   $mappings+=[ordered]@{address=('{0:x}' -f $addr);name=$name;value=[BitConverter]::ToSingle($b,8);w4=[BitConverter]::ToUInt32($b,4);wc=('{0:x}' -f [BitConverter]::ToUInt32($b,0xc));limits=$(if($lb -ne 0){($le-$lb)/16}else{0})}
  } elseif($kind -eq 1){
   $b=[GridAiMappingReader]::TryRead($h,$addr,0xcb0); if($null -eq $b){continue}
   # laDistMin name pointer slot (+0xA0) must reference the string at 0xCF8BF0
   if([BitConverter]::ToUInt32($b,0xa0) -ne 0xcf8bf0){continue}
   $driverConfigs+=[ordered]@{address=('{0:x}' -f $addr);hex=[BitConverter]::ToString($b).Replace('-','')}
  } elseif($kind -eq 2){
   $b=[GridAiMappingReader]::TryRead($h,$addr,0x690); if($null -eq $b){continue}
   # stAngVelFactor name pointer slot (+0x28) must reference an .rdata string near the parser table
   $np=[BitConverter]::ToUInt32($b,0x28); if($np -lt 0xcf7000 -or $np -gt 0xcf8000){continue}
   $vehicleConfigs+=[ordered]@{address=('{0:x}' -f $addr);hex=[BitConverter]::ToString($b).Replace('-','')}
  }
 }
 $record=[ordered]@{pid=$ProcessId;started_utc=$process.StartTime.ToUniversalTime().ToString('o');exe_sha256=$expectedHash;scan_ms=$sw.ElapsedMilliseconds;raw_hits=$hits.Count;hit_list=@($hits | ForEach-Object { '{0:x}:{1}' -f $_[0],$_[1] });mappings=$mappings;driver_configs=$driverConfigs;vehicle_configs=$vehicleConfigs}
 [IO.File]::WriteAllText($out,($record | ConvertTo-Json -Depth 6))
 "hits=$($hits.Count) mappings=$($mappings.Count) driver_configs=$($driverConfigs.Count) vehicle_configs=$($vehicleConfigs.Count) ms=$($sw.ElapsedMilliseconds)"
 "PRIVATE_OUTPUT=$out"
} finally { [void][GridAiMappingReader]::CloseHandle($h) }
