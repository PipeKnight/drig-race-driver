<#
.SYNOPSIS
Sample a hung thread's call stacks with Windows' own dbghelp (no debugger needed; docs/SCREEN_FLOW.md §2.2).

.DESCRIPTION
Suspends the thread, walks its stack with dbghelp StackWalk64 and names the frames from the PDBs next to
the exe. Repeats this -Samples times, 50 ms apart, and then prints the top frames and the most common stack.
Read-only apart from the short suspends. Usage:
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\stack_sample.ps1 -ProcessId 1234 [-ThreadId N] [-Samples 40] [-SymPath target\release]
Without -ThreadId it samples the process's first thread, which is the main thread.
F773 found a winit main-loop spin with this.
#>
param([Parameter(Mandatory)][int]$ProcessId, [int]$ThreadId = 0, [int]$Samples = 40, [string]$SymPath = 'target\release')
if ($ThreadId -eq 0) {
    $ThreadId = ((Get-Process -Id $ProcessId).Threads | Sort-Object StartTime | Select-Object -First 1).Id
}
$SymPath = (Resolve-Path $SymPath).Path
$src = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class Sampler {
  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint a, bool i, int pid);
  [DllImport("kernel32.dll")] static extern IntPtr OpenThread(uint a, bool i, int tid);
  [DllImport("kernel32.dll")] static extern uint SuspendThread(IntPtr t);
  [DllImport("kernel32.dll")] static extern uint ResumeThread(IntPtr t);
  [DllImport("kernel32.dll")] static extern bool GetThreadContext(IntPtr t, IntPtr ctx);
  [DllImport("kernel32.dll")] static extern IntPtr LoadLibrary(string n);
  [DllImport("kernel32.dll")] static extern IntPtr GetProcAddress(IntPtr m, string n);
  [DllImport("dbghelp.dll")] static extern uint SymSetOptions(uint o);
  [DllImport("dbghelp.dll", CharSet = CharSet.Ansi)] static extern bool SymInitialize(IntPtr p, string path, bool invade);
  [DllImport("dbghelp.dll")] static extern bool SymFromAddr(IntPtr p, ulong addr, out ulong disp, IntPtr sym);
  [DllImport("dbghelp.dll")] static extern bool StackWalk64(uint machine, IntPtr p, IntPtr t, IntPtr frame, IntPtr ctx, IntPtr read, IntPtr fta, IntPtr gmb, IntPtr tr);
  public static string Run(int pid, int tid, int samples, string symPath) {
    IntPtr p = OpenProcess(0x1F0FFF, false, pid);
    IntPtr t = OpenThread(0x1F03FF, false, tid);
    SymSetOptions(0x2 | 0x4 | 0x10);
    if (!SymInitialize(p, symPath == "" ? null : symPath, true)) return "SymInitialize failed";
    IntPtr dbg = LoadLibrary("dbghelp.dll");
    IntPtr fta = GetProcAddress(dbg, "SymFunctionTableAccess64");
    IntPtr gmb = GetProcAddress(dbg, "SymGetModuleBase64");
    IntPtr raw = Marshal.AllocHGlobal(1232 + 16);
    IntPtr ctx = new IntPtr((raw.ToInt64() + 15) & ~15L);
    IntPtr frame = Marshal.AllocHGlobal(512);
    IntPtr sym = Marshal.AllocHGlobal(88 + 1024);
    var stacks = new Dictionary<string, int>();
    var tops = new Dictionary<string, int>();
    for (int s = 0; s < samples; s++) {
      for (int i = 0; i < 1248; i++) Marshal.WriteByte(raw, i, 0);
      for (int i = 0; i < 512; i++) Marshal.WriteByte(frame, i, 0);
      Marshal.WriteInt32(ctx, 0x30, 0x10000B);
      SuspendThread(t);
      var names = new List<string>();
      if (GetThreadContext(t, ctx)) {
        Marshal.WriteInt64(frame, 0, Marshal.ReadInt64(ctx, 0xF8)); Marshal.WriteInt32(frame, 12, 3);
        Marshal.WriteInt64(frame, 32, Marshal.ReadInt64(ctx, 0xA0)); Marshal.WriteInt32(frame, 44, 3);
        Marshal.WriteInt64(frame, 48, Marshal.ReadInt64(ctx, 0x98)); Marshal.WriteInt32(frame, 60, 3);
        for (int d = 0; d < 40; d++) {
          if (!StackWalk64(0x8664, p, t, frame, ctx, IntPtr.Zero, fta, gmb, IntPtr.Zero)) break;
          ulong pc = (ulong)Marshal.ReadInt64(frame, 0);
          if (pc == 0) break;
          for (int i = 0; i < 88; i++) Marshal.WriteByte(sym, i, 0);
          Marshal.WriteInt32(sym, 0, 88); Marshal.WriteInt32(sym, 80, 1024);
          ulong disp;
          string n = SymFromAddr(p, pc, out disp, sym) ? Marshal.PtrToStringAnsi(new IntPtr(sym.ToInt64() + 84)) : ("0x" + pc.ToString("x"));
          if (n.Length > 140) n = n.Substring(0, 140);
          names.Add(n);
        }
      }
      ResumeThread(t);
      string key = string.Join("\n    ", names.GetRange(0, Math.Min(names.Count, 25)));
      stacks[key] = stacks.ContainsKey(key) ? stacks[key] + 1 : 1;
      string top = names.Count > 0 ? names[0] : "?";
      tops[top] = tops.ContainsKey(top) ? tops[top] + 1 : 1;
      System.Threading.Thread.Sleep(50);
    }
    var sb = new StringBuilder();
    sb.Append("TOP FRAMES:\n");
    foreach (var kv in tops) sb.Append("  " + kv.Value + "x " + kv.Key + "\n");
    int best = 0; string bestKey = "";
    foreach (var kv in stacks) if (kv.Value > best) { best = kv.Value; bestKey = kv.Key; }
    sb.Append("MOST COMMON STACK (" + best + "/" + samples + "):\n    " + bestKey + "\n");
    return sb.ToString();
  }
}
'@
Add-Type -TypeDefinition $src
"thread=$ThreadId samples=$Samples"
[Sampler]::Run($ProcessId, $ThreadId, $Samples, $SymPath)
