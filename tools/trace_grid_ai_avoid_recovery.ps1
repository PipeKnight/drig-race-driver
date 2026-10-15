# Bounded read-only per-frame trace of the original AI avoid/recovery state for
# all 12 driver objects of the private GRID research process.
# Opens only VM_READ | QUERY_LIMITED_INFORMATION; no writes, injection, suspension
# or input. Output is a private TSV under local/ (see docs/AI_AVOID_RECOVERY_LIVE.md).
#
# Layout (live PID 7208, analysis image aac46a64..., docs/AI_AVOID_RECOVERY_RESEARCH.md):
#   driver i = DriverBase + i*0x4FE0; own car state = *(driver-0x40)
#   request = driver+0x2CE0, track view = driver+0x2DD0, current behaviour = *(driver+0x2DC0)
#   tests = pointer array driver+0x3EA0 (type index), behaviours = vector driver+0x3F10..
param(
 [Parameter(Mandatory=$true)][int]$ProcessId,
 [Parameter(Mandatory=$true)][string]$Output,
 [long]$DriverBase=0x56a3b10,
 [double]$Seconds=60,
 [int]$BaselineEvery=20
)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$out=[IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($Output)){$Output}else{Join-Path $root $Output}))
if(!$out.StartsWith((Join-Path $root 'local')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Output must remain under local/'}
if(Test-Path -LiteralPath $out){throw 'Output already exists'}
if($Seconds -le 0 -or $Seconds -gt 1800){throw 'Seconds must be in (0, 1800]'}
$expectedPath='C:\Users\debros\Tools\grid-research\grid-compat\grid.exe'
$expectedHash='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
$process=Get-Process -Id $ProcessId
if($process.MainModule.FileName -ne $expectedPath){throw 'Unexpected executable path'}
if((Get-FileHash -LiteralPath $expectedPath).Hash.ToLower() -ne $expectedHash){throw 'Executable identity changed'}
Add-Type -TypeDefinition @'
using System; using System.IO; using System.Text; using System.Diagnostics; using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class GridAiTrace {
 [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenProcess(uint a, bool i, int p);
 [DllImport("kernel32.dll", SetLastError=true)] static extern bool ReadProcessMemory(IntPtr h, IntPtr a, byte[] b, IntPtr s, out IntPtr r);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
 [DllImport("kernel32.dll")] static extern bool GetExitCodeProcess(IntPtr h, out uint c);
 static IntPtr H;
 static byte[] R(long a, int n) { var b = new byte[n]; IntPtr r; if (!ReadProcessMemory(H, new IntPtr(a), b, new IntPtr(n), out r) || r.ToInt64() != n) throw new Exception(string.Format("read {0:x}", a)); return b; }
 static float F(byte[] b, int o) { return BitConverter.ToSingle(b, o); }
 static uint U(byte[] b, int o) { return BitConverter.ToUInt32(b, o); }
 static readonly Dictionary<uint,string> Beh = new Dictionary<uint,string> {
  {0xCF8E50,"overtake"},{0xCF8E74,"race"},{0xCF9008,"alongside"},{0xCF902C,"avoid"},{0xCF8E08,"block"},{0xCF8E2C,"draft"},
  {0xCF8E98,"race_damaged"},{0xCF8EBC,"race_start"},{0xCF8EE0,"controlled_stop"},{0xCF8F04,"cruise"},{0xCF8F28,"recovery"},
  {0xCF8F4C,"recover_tbone"},{0xCF8F70,"lost_control"},{0xCF8F9C,"retire"},{0xCF8FC0,"drift"},{0xCF8FE4,"visibility_loss"} };
 const int Lo = 0x2AC0, Len = 0x2240; // driver window: path obstacle, request, view, tests, behaviours
 static string G(float x) { return x.ToString("G6", System.Globalization.CultureInfo.InvariantCulture); }
 public static string Run(int pid, long baseAddr, double seconds, string path, int baselineEvery) {
  H = OpenProcess(0x0010 | 0x1000, false, pid); if (H == IntPtr.Zero) throw new Exception("open");
  try {
   int n = 12; long[] drv = new long[n]; long[] own = new long[n];
   var tests = new long[n, 21]; var behs = new Dictionary<string,long>[n];
   for (int i = 0; i < n; i++) {
    drv[i] = baseAddr + i * 0x4FE0L;
    own[i] = U(R(drv[i] - 0x40, 4), 0);
    var w = R(drv[i] + Lo, Len);
    for (int k = 0; k < 21; k++) tests[i, k] = U(w, 0x3EA0 - Lo + 4 * k);
    if (U(R(tests[i, 14], 4), 0) != 0xCF73EC) throw new Exception("avoid test vtable " + i);
    behs[i] = new Dictionary<string,long>();
    long vb = U(w, 0x2C74 - Lo), ve = U(w, 0x2C78 - Lo); // behaviour vector begin/end
    for (long p = vb; p < ve; p += 4) { long bo = U(R(p, 4), 0); string nm; if (Beh.TryGetValue(U(R(bo, 4), 0), out nm)) behs[i][nm] = bo; }
    foreach (var need in new[]{"avoid","recovery","recover_tbone","alongside","overtake","race_start"}) if (!behs[i].ContainsKey(need)) throw new Exception("missing behaviour " + need + " in driver " + i);
   }
   var sw = Stopwatch.StartNew(); long frames = 0, rows = 0, polls = 0;
   var last = new string[n]; var seen = new long[n]; var cnt = new Dictionary<string,long>[n];
   for (int i = 0; i < n; i++) cnt[i] = new Dictionary<string,long>();
   using (var o = new StreamWriter(path, false, new UTF8Encoding(false))) {
    o.WriteLine("t\tdrv\tframe\tcur\tsc_avoid\tsc_recovery\tsc_tbone\tsc_along\tsc_overtake\tav_fired\tav_id\tav_rec\tav_eta\tav_closing\tav_side\tav_timer\tav_tspeed\tav_offset\tav_clear\trc_mode\trc_latch\trc_toggle\trc_elapsed\trc_otimer\ttb_timer\tspun\tspun_fast\tontrack\tontrack_n\tcollob\tcollob_front\tcontact\tcontact_type\tcontact_rec\treq_reverse\treq_fullbrake\treq_force\treq_nosteer\treq_b2s\treq_desired\treq_cap\treq_head\treq_offset\treq_merge\tview_b_rad\tview_a_deg\tlateral\toctant\ttarget_right\tspeed\tpx\tpy\tpz\tfx\tfz\tvx\tvz\tobs_mag\tobs_nx\tobs_ny\tobs_nz\trec_speed\trec_id\trec_state\trec_px\trec_pz\tob_rec\tob_dist\tob_v\tob_eta\tob_speed\tob_state\tob_dot\tob_links\tob_gate\tob_angle\tob_excluded");
    while (sw.Elapsed.TotalSeconds < seconds) {
     uint code; if (GetExitCodeProcess(H, out code) && code != 259) break;
     polls++; long before = frames;
     for (int i = 0; i < n; i++) {
      var w = R(drv[i] + Lo, Len); var ow = R(own[i], 0x140);
      // frame identity: own position + view heading + request heading
      string key = BitConverter.ToString(ow, 0x20, 12) + BitConverter.ToString(w, 0x2DD0 - Lo + 0x64, 4) + BitConverter.ToString(ow, 0xA0, 12);
      if (key == last[i]) continue;
      last[i] = key; seen[i]++; frames++;
      Func<string,int,byte[]> B = (nm, len) => R(behs[i][nm], len);
      Func<int,int,byte[]> T = (k, len) => tests[i, k] == 0 ? new byte[len] : R(tests[i, k], len);
      var av = T(14, 0x30); var sp = T(1, 0x10); var ot = T(11, 0x10); var co = T(10, 0x10); var cv = T(17, 0x20);
      var ba = B("avoid", 0x50); var br = B("recovery", 0x40); var bt = B("recover_tbone", 0x38);
      float scA = F(ba, 0x10), scR = F(br, 0x10), scT = F(bt, 0x10), scL = F(B("alongside", 0x14), 0x10), scO = F(B("overtake", 0x14), 0x10);
      long cur = U(w, 0x2DC0 - Lo); string curName = "?";
      if (cur != 0) { string nm; if (Beh.TryGetValue(U(R(cur, 4), 0), out nm)) curName = nm; }
      int rq = 0x2CE0 - Lo, vw = 0x2DD0 - Lo;
      // view geometry (0x77C9B0 / 0x40AE90)
      float fx = F(ow, 0x30), fz = F(ow, 0x38);
      float tx = F(w, vw + 0xD0) - F(w, vw + 0x170), tz = F(w, vw + 0xD8) - F(w, vw + 0x178);
      float lateral = (F(w, vw + 0x40) - F(w, vw + 0x170)) * F(w, vw + 0x180) + (F(w, vw + 0x44) - F(w, vw + 0x174)) * F(w, vw + 0x184)
                    + (F(w, vw + 0x48) - F(w, vw + 0x178)) * F(w, vw + 0x188) + (F(w, vw + 0x4C) - F(w, vw + 0x17C)) * F(w, vw + 0x18C);
      double tl = Math.Sqrt(tx * tx + tz * tz), fl = Math.Sqrt(fx * fx + fz * fz);
      double a = (tl > 0 && fl > 0) ? Math.Acos(Math.Max(-1, Math.Min(1, (tx * fx + tz * fz) / (tl * fl)))) : 0;
      if (fz * tx - fx * tz >= 0) a = -a;
      double bdeg = F(w, vw + 0x64) * 57.29578;
      double adeg = a * 57.29578;
      if (lateral <= 0) { adeg = -adeg; bdeg = -bdeg; }
      int oct = Octant(adeg, bdeg);
      // planned look-ahead target relative to the car's right axis (own+0x50)
      float targetRight = (F(w, vw + 0xD0) - F(ow, 0xF0)) * F(ow, 0x50) + (F(w, vw + 0xD8) - F(ow, 0xF8)) * F(ow, 0x58);
      // path obstacle (driver+0x2AC0: rel. velocity +0, record +0x20, distance +0x24) and an emulation of the
      // racing-normally exclusion 0x79A530 (speed, state, heading, link lookup 0x798C00 over the view window, gate angle)
      int ob = 0x2AC0 - Lo; long obRec = U(w, ob + 0x20); float obDist = F(w, ob + 0x24);
      float obV = (float)Math.Sqrt(F(w, ob) * F(w, ob) + F(w, ob + 8) * F(w, ob + 8)); float obEta = obV > 0 ? obDist / obV : float.MaxValue;
      string obCols = "\t\t\t\t\t\t\t\t\t\t\t";
      bool obNear = false;
      if (obRec != 0) {
       var orr = R(obRec, 0x100); float ospd = F(orr, 0x10); uint ost = U(orr, 0xFC);
       float rfx = F(orr, 0x30), rfz = F(orr, 0x38), rcx = F(orr, 0x20), rcz = F(orr, 0x28);
       float hdot = fx * rfx + fz * rfz;
       long arr = U(w, vw + 0x14); int cap = (int)U(w, vw + 4); var la = R(arr, 4 * cap);
       int found = 0; long gate = 0; double ang = double.NaN, best = double.MaxValue;
       for (int k = 0; k < cap; k++) {
        long l = U(la, 4 * k); if (l == 0) continue;
        float[] g = Link(l); if (g == null) continue;
        double s1 = g[3] * (rcx - g[0]) - g[2] * (rcz - g[1]), s2 = g[8] * (rcx - g[5]) - g[7] * (rcz - g[6]);
        if (s1 >= 0 && !(s2 >= 0)) { found++;
          // nearest containing link (0x798C00 keeps the smallest 0x798920 distance): distance to the gate-to-gate segment
          double sx = g[5] - g[0], sz = g[6] - g[1], q = ((rcx - g[0]) * sx + (rcz - g[1]) * sz) / Math.Max(1e-6, sx * sx + sz * sz); q = Math.Max(0, Math.Min(1, q));
          double ex = rcx - (g[0] + q * sx), ez = rcz - (g[1] + q * sz), dist = Math.Sqrt(ex * ex + ez * ez);
          if (dist <= best) { best = dist; gate = (long)g[4]; double gtx = -g[3], gtz = g[2];
          double c = (gtx * rfx + gtz * rfz) / (Math.Sqrt(gtx * gtx + gtz * gtz) * Math.Sqrt(rfx * rfx + rfz * rfz)); ang = Math.Acos(Math.Max(-1, Math.Min(1, c))); } }
       }
       bool excluded = ospd >= 4 && ost != 2 && hdot >= 0 && gate != 0 && ang > 0.7853982;
       obNear = obEta < 2;
       obCols = "\t" + obRec.ToString("x") + "\t" + G(obDist) + "\t" + G(obV) + "\t" + G(obEta) + "\t" + G(ospd) + "\t" + ost + "\t" + G(hdot) + "\t" + found + "\t" + gate + "\t" + G((float)ang) + "\t" + (excluded ? 1 : 0);
       if (obNear) { Bump(cnt[i], "ob_eta_lt2"); if (excluded) Bump(cnt[i], "ob_eta_lt2_excluded"); if (av[8] != 0) Bump(cnt[i], "ob_eta_lt2_fired"); if (excluded && av[8] != 0) Bump(cnt[i], "ob_excluded_but_fired"); }
      }
      bool interesting = obNear || U(av, 8) % 256 != 0 || scA > 0 || scR > 0 || scT > 0 || sp[8] != 0 || ot[8] == 0 || co[8] != 0 || cv[8] != 0
        || w[rq + 0xAF] != 0 || w[rq + 0xB2] != 0 || F(bt, 0x34) > 0 || F(br, 0x3C) > 0
        || (curName != "race" && curName != "overtake" && curName != "alongside" && curName != "race_start");
      Bump(cnt[i], "frames");
      if (av[8] != 0) Bump(cnt[i], "avoid_fired"); if (curName == "avoid") Bump(cnt[i], "avoid_active");
      if (curName == "recovery") Bump(cnt[i], "recovery_active"); if (curName == "recover_tbone") Bump(cnt[i], "tbone_active");
      if (co[8] != 0) Bump(cnt[i], "collob"); if (cv[8] != 0) Bump(cnt[i], "contacted"); if (sp[8] != 0) Bump(cnt[i], "spun");
      if (ot[8] == 0) Bump(cnt[i], "offtrack"); if (w[rq + 0xAF] != 0) Bump(cnt[i], "reverse");
      Bump(cnt[i], "cur_" + curName);
      if (!interesting && (seen[i] % baselineEvery) != 0) continue;
      long rec = U(av, 0x10); byte[] rr = rec != 0 ? R(rec, 0x100) : null;
      var sb = new StringBuilder();
      sb.Append(sw.Elapsed.TotalSeconds.ToString("F4", System.Globalization.CultureInfo.InvariantCulture)).Append('\t').Append(i).Append('\t').Append(seen[i]).Append('\t').Append(curName);
      foreach (var x in new float[]{scA, scR, scT, scL, scO}) sb.Append('\t').Append(G(x));
      sb.Append('\t').Append(av[8]).Append('\t').Append(U(av, 0xC)).Append('\t').Append(rec.ToString("x")).Append('\t').Append(G(F(av, 0x14))).Append('\t').Append(G(F(av, 0x18)));
      sb.Append('\t').Append(U(ba, 0x38)).Append('\t').Append(G(F(ba, 0x40))).Append('\t').Append(G(F(ba, 0x44))).Append('\t').Append(G(F(ba, 0x48))).Append('\t').Append(G(F(ba, 0x4C)));
      sb.Append('\t').Append(U(br, 0x30)).Append('\t').Append(br[0x35]).Append('\t').Append(br[0x36]).Append('\t').Append(G(F(br, 0x38))).Append('\t').Append(G(F(br, 0x3C))).Append('\t').Append(G(F(bt, 0x34)));
      sb.Append('\t').Append(sp[8]).Append('\t').Append(sp[0xC]).Append('\t').Append(ot[8]).Append('\t').Append(U(ot, 0xC)).Append('\t').Append(co[8]).Append('\t').Append(co[0xC]);
      sb.Append('\t').Append(cv[8]).Append('\t').Append(U(cv, 0x14)).Append('\t').Append(U(cv, 0x10).ToString("x"));
      foreach (int f in new[]{0xAF, 0xB2, 0xA5, 0xA6, 0xA4}) sb.Append('\t').Append(w[rq + f]);
      foreach (int f in new[]{0x84, 0x7C, 0x88, 0x98, 0x9C}) sb.Append('\t').Append(G(F(w, rq + f)));
      sb.Append('\t').Append(G(F(w, vw + 0x64))).Append('\t').Append(G((float)adeg)).Append('\t').Append(G(lateral)).Append('\t').Append(oct).Append('\t').Append(G(targetRight));
      sb.Append('\t').Append(G(F(ow, 0xD0)));
      foreach (int f in new[]{0x20, 0x24, 0x28, 0x30, 0x38, 0xA0, 0xA8, 0x120, 0x130, 0x134, 0x138}) sb.Append('\t').Append(G(F(ow, f)));
      if (rr != null) sb.Append('\t').Append(G(F(rr, 0x10))).Append('\t').Append(U(rr, 0xC0)).Append('\t').Append(U(rr, 0xFC)).Append('\t').Append(G(F(rr, 0x20))).Append('\t').Append(G(F(rr, 0x28)));
      else sb.Append("\t\t\t\t\t");
      sb.Append(obCols);
      o.WriteLine(sb.ToString()); rows++;
     }
     if (frames == before) System.Threading.Thread.Sleep(1);
    }
   }
   var s = new StringBuilder();
   s.AppendFormat("seconds={0:F1} polls={1} frames={2} rows={3}\n", sw.Elapsed.TotalSeconds, polls, frames, rows);
   for (int i = 0; i < n; i++) { s.Append("drv").Append(i).Append(" own=").Append(own[i].ToString("x")); foreach (var kv in cnt[i]) s.Append(' ').Append(kv.Key).Append('=').Append(kv.Value); s.Append('\n'); }
   return s.ToString();
  } finally { CloseHandle(H); }
 }
 // link -> [g1 pos x, z, g1 dir x, z, g1 id, g2 pos x, z, g2 dir x, z]; links and gates are static track data
 static readonly Dictionary<long,float[]> LinkCache = new Dictionary<long,float[]>();
 static float[] Link(long l) {
  float[] g; if (LinkCache.TryGetValue(l, out g)) return g;
  var lb = R(l, 0x14); long g1 = U(lb, 0xC), g2 = U(lb, 0x10); if (g1 == 0 || g2 == 0) return null;
  var a = R(g1, 0x30); var b = R(g2, 0x30);
  g = new float[]{ F(a, 0x10), F(a, 0x18), F(a, 0x20), F(a, 0x28), U(a, 0), F(b, 0x10), F(b, 0x18), F(b, 0x20), F(b, 0x28) };
  LinkCache[l] = g; return g;
 }
 static void Bump(Dictionary<string,long> d, string k) { long v; d.TryGetValue(k, out v); d[k] = v + 1; }
 // 0x77C9B0 table (degrees, after the side mirror); 0 on boundaries.
 public static int Octant(double a, double b) {
  if (b > 0 && b < 90 && a > 0 && a < 90) return 1;
  if (b < 0) { if (b > -90 && a > 0 && a < 90) return 2; if (b > -90 && a < 0 && a > -90) return 3; }
  if (b < -90) { if (b > -180 && a < 0 && a > -90) return 4; if (b > -180 && a < -90 && a > -180) return 5; }
  if (b > 90) { if (b < 180 && a < -90 && a > -180) return 6; if (b < 180 && a > 90 && a < 180) return 7; }
  if (b > 0 && b < 90 && a > 90 && a < 180) return 8;
  return 0;
 }
}
'@
$summary=[GridAiTrace]::Run($ProcessId,$DriverBase,$Seconds,$out,$BaselineEvery)
$summary
"PRIVATE_OUTPUT=$out"
