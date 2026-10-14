# Bounded read-only player controls and motion; no injection or process writes.
param(
 [Parameter(Mandatory=$true)][string]$SessionJson,
 [Parameter(Mandatory=$true)][string]$Output,
 [Parameter(Mandatory=$true)][long]$ControllerAddress,
 [ValidateRange(500,30000)][int]$DurationMs=20000
)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$taskLocal=(Join-Path $taskRoot 'local')+[IO.Path]::DirectorySeparatorChar
$taskSessionPath=[IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($SessionJson)){$SessionJson}else{Join-Path $taskRoot $SessionJson}))
$taskOutput=[IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($Output)){$Output}else{Join-Path $taskRoot $Output}))
if(!$taskSessionPath.StartsWith($taskLocal,[StringComparison]::OrdinalIgnoreCase) -or !$taskOutput.StartsWith($taskLocal,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $taskOutput)){throw 'New private local output and private session required'}
$taskParent=Get-Item -LiteralPath ([IO.Path]::GetDirectoryName($taskOutput))
while($taskParent.FullName.Length -ge (Join-Path $taskRoot 'local').Length){
 if(($taskParent.Attributes -band [IO.FileAttributes]::ReparsePoint) -and $taskParent.LinkType -in @('Junction','SymbolicLink')){throw 'Reparse-point output ancestor'}
 $taskParent=$taskParent.Parent
}
if($ControllerAddress -lt 0x20000 -or $ControllerAddress -gt 0x7ffe0000){throw 'Controller outside bound'}
$taskSession=Get-Content -LiteralPath $taskSessionPath -Raw | ConvertFrom-Json
$taskPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$taskHash='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
if($taskSession.Path -ne $taskPath -or $taskSession.ExeSHA256 -ne $taskHash -or (Get-FileHash -LiteralPath $taskPath).Hash.ToLower() -ne $taskHash){throw 'Private executable identity mismatch'}
$taskProcess=Get-Process -Id $taskSession.Pid
$taskStarted=([DateTimeOffset]$taskSession.StartedUtc).UtcDateTime
if($taskProcess.MainModule.FileName -ne $taskPath -or $taskProcess.MainModule.BaseAddress.ToInt64() -ne 0x400000 -or $taskProcess.StartTime.ToUniversalTime().Ticks -ne $taskStarted.Ticks){throw 'Private process identity mismatch'}
$taskSource=Join-Path $taskRoot 'local/compatibility/grid-unpacked.exe'
if((Get-FileHash -LiteralPath $taskSource).Hash.ToLower() -ne 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'){throw 'Analysis reference changed'}
$taskRaw=[IO.File]::ReadAllBytes($taskSource)
$taskEntries=[uint32[]]@(0x4223d0,0x451770,0x42de60,0xa233d0,0xa25d90,0xa27780,0xa0a290)
$taskPrefixes=[Collections.Generic.List[byte[]]]::new()
foreach($taskEntry in $taskEntries){$taskOffset=$taskEntry-0x400000-0x1000+0x400;$taskPrefixes.Add([byte[]]$taskRaw[$taskOffset..($taskOffset+31)])}
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
public static class GridDrivingReadOnly {
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool ReadProcessMemory(IntPtr handle,IntPtr address,byte[] bytes,UIntPtr size,out UIntPtr done);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetProcessTimes(IntPtr handle,out long created,out long exited,out long kernel,out long user);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool QueryFullProcessImageName(IntPtr handle,uint flags,StringBuilder name,ref uint size);
 static byte[] Read(IntPtr h,uint address,int size) {
  if(address<0x10000 || (ulong)address+(uint)size>0x7fff0000 || size<1 || size>0x340) throw new InvalidOperationException("Read outside bounded scope");
  var b=new byte[size]; UIntPtr done;
  if(!ReadProcessMemory(h,new IntPtr((long)address),b,new UIntPtr((uint)size),out done) || done.ToUInt64()!=(uint)size) throw new InvalidOperationException("Owned read failed");
  return b;
 }
 static uint Word(byte[] b,int at){return BitConverter.ToUInt32(b,at);}
 static uint Ptr(IntPtr h,uint at){return Word(Read(h,at,4),0);}
 static string Hex(byte[] b){return BitConverter.ToString(b).Replace("-","").ToLowerInvariant();}
 public static void Capture(int pid,long started,string expected,uint controller,int ms,string output,uint[] entries,byte[][] prefixes) {
  var h=OpenProcess(0x1010,false,pid); if(h==IntPtr.Zero) throw new InvalidOperationException("Read-only process unavailable");
  try {
   long created,exited,kernel,user;var name=new StringBuilder(32768);uint length=32768;
   if(!GetProcessTimes(h,out created,out exited,out kernel,out user) || created!=started || !QueryFullProcessImageName(h,0,name,ref length) || !String.Equals(name.ToString(),expected,StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("Handle identity mismatch");
   if(ms<500 || ms>30000 || entries.Length!=7 || prefixes.Length!=7)throw new InvalidOperationException("Capture configuration bound");
   for(int i=0;i<7;i++)if(Hex(Read(h,entries[i],32))!=Hex(prefixes[i]))throw new InvalidOperationException("Live code prefix mismatch");
   var state=Read(h,controller,0x340);uint root=Word(state,0x128),component=Word(state,0x110);
   if((ulong)root+0x8f10!=controller || Word(state,0x118)!=(ulong)root+0x1a40 || Ptr(h,root+0x20)!=component || Ptr(h,component+0x6c)!=0 || Word(state,0x328)!=477036 || Word(state,0x32c)!=319292)throw new InvalidOperationException("Player Lacetti controller mismatch");
   uint car=Ptr(h,component+0x24),wrapper=Ptr(h,car+0xdb0),physical=Ptr(h,wrapper+8);
   uint world=Ptr(h,physical+0x32c),job=0;var jobs=Read(h,0xe8ed98,20*0x14);
   for(int i=0;i<20;i++)if(Word(jobs,i*0x14+4)==car){if(job!=0)throw new InvalidOperationException("Duplicate player job");job=0xe8ed98+(uint)i*0x14;}
   if(job==0 || Ptr(h,job)!=0xd15438 || Ptr(h,0xd1543c)!=0xa0a290)throw new InvalidOperationException("Owned player job mismatch");
   using(var file=new FileStream(output,FileMode.CreateNew,FileAccess.Write,FileShare.None)) {
    var text=new StringBuilder();long began=Stopwatch.GetTimestamp();int count=0;
    text.Append("DT02\t").Append(Stopwatch.Frequency).Append('\t').Append(pid).Append('\t').Append(started).Append('\t').Append(controller.ToString("x")).Append('\t').Append(car.ToString("x")).Append('\t').Append(physical.ToString("x")).Append('\t').Append(world.ToString("x")).Append('\t').Append(job.ToString("x")).Append('\n');
    Console.WriteLine("driving_capture event=started pid="+pid+" qpc="+began+" read_only=1");
    while((Stopwatch.GetTimestamp()-began)*1000/Stopwatch.Frequency<ms) {
     if(count>=3100 || text.Length>2097152)throw new InvalidOperationException("Capture output bound");
     long begin=Stopwatch.GetTimestamp();state=Read(h,controller,0x340);
     if(Word(state,0x128)!=root || Word(state,0x110)!=component || Word(state,0x118)!=root+0x1a40 || Ptr(h,root+0x20)!=component || Ptr(h,component+0x24)!=car || Ptr(h,component+0x6c)!=0 || Ptr(h,car+0xdb0)!=wrapper || Ptr(h,wrapper+8)!=physical)throw new InvalidOperationException("Player ownership changed");
     var controls=Read(h,car+0xe1c,40);var motion=Read(h,physical+0x1d0,96);var engine=new byte[24];int at=0;
     foreach(uint offset in new uint[]{0x1894,0x196c,0x1648,0x17a4,0x17a8,0x21f0}){Array.Copy(Read(h,car+offset,4),0,engine,at,4);at+=4;}
     bool repeat=Hex(controls)==Hex(Read(h,car+0xe1c,40));long end=Stopwatch.GetTimestamp();
     if(Ptr(h,physical+0x32c)!=world || Ptr(h,job)!=0xd15438 || Ptr(h,job+4)!=car)throw new InvalidOperationException("World/player job ownership changed");
     var schedule=new byte[12];Array.Copy(Read(h,world+0x7c4,8),schedule,8);Array.Copy(Read(h,job+8,4),0,schedule,8,4);end=Stopwatch.GetTimestamp();
     text.Append("T\t").Append(count++).Append('\t').Append(begin).Append('\t').Append(end).Append('\t').Append(repeat?1:0).Append('\t').Append(Hex(controls)).Append('\t').Append(Hex(motion)).Append('\t').Append(Hex(engine)).Append('\t').Append(Hex(schedule)).Append('\n');
     Thread.Sleep(10);
    }
    text.Append("S\t").Append(count).Append('\t').Append(Stopwatch.GetTimestamp()).Append('\n');
    var data=Encoding.UTF8.GetBytes(text.ToString());file.Write(data,0,data.Length);file.Flush(true);
    Console.WriteLine("driving_capture event=completed records="+count+" sequential_reads=1");
   }
  }finally{CloseHandle(h);}
 }
}
'@
[GridDrivingReadOnly]::Capture($taskSession.Pid,$taskStarted.ToFileTimeUtc(),$taskPath,[uint32]$ControllerAddress,$DurationMs,$taskOutput,$taskEntries,$taskPrefixes.ToArray())
