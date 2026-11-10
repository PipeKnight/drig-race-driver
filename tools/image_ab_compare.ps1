# Matched image A/B of two capture folders (flag-NNN.png or frame-NNN.png, paired by index; usage -Off DIR -On DIR -Out DIR): per frame, the fraction of pixels whose
# max channel differs by > 8/255 and the mean luma change; stacks the top 3
# (off over on, downscaled to 640x360 each) as OUTDIR/pair-NNN.png.
param([string]$Off, [string]$On, [string]$Out)
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System; using System.Drawing; using System.Drawing.Imaging;
public static class Ab {
  public static double[] Diff(string a, string b) {
    using (var x = new Bitmap(a)) using (var y = new Bitmap(b)) {
      var r = new Rectangle(0, 0, x.Width, x.Height);
      var dx = x.LockBits(r, ImageLockMode.ReadOnly, PixelFormat.Format24bppRgb);
      var dy = y.LockBits(r, ImageLockMode.ReadOnly, PixelFormat.Format24bppRgb);
      int n = dx.Stride * x.Height; var bx = new byte[n]; var by = new byte[n];
      System.Runtime.InteropServices.Marshal.Copy(dx.Scan0, bx, 0, n);
      System.Runtime.InteropServices.Marshal.Copy(dy.Scan0, by, 0, n);
      x.UnlockBits(dx); y.UnlockBits(dy);
      long changed = 0; double dl = 0;
      for (int row = 0; row < x.Height; row++) for (int c = 0; c < x.Width; c++) {
        int i = row * dx.Stride + c * 3; int m = 0;
        for (int k = 0; k < 3; k++) m = Math.Max(m, Math.Abs(bx[i+k] - by[i+k]));
        if (m > 8) changed++;
        dl += (0.0722*(by[i]-bx[i]) + 0.7152*(by[i+1]-bx[i+1]) + 0.2126*(by[i+2]-bx[i+2]));
      }
      double px = (double)x.Width * x.Height;
      return new double[] { changed / px, dl / px };
    }
  }
  public static void Stack(string a, string b, string o) {
    using (var x = new Bitmap(a)) using (var y = new Bitmap(b)) using (var s = new Bitmap(640, 720)) using (var g = Graphics.FromImage(s)) {
      g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBicubic;
      g.DrawImage(x, 0, 0, 640, 360); g.DrawImage(y, 0, 360, 640, 360); s.Save(o, ImageFormat.Png);
    }
  }
}
"@
New-Item -ItemType Directory -Force $Out | Out-Null
# Frames are flag-NNN or frame-NNN depending on the verdict: match by index.
function Index($dir) { $h = @{}; foreach ($f in Get-ChildItem $dir -Filter '*.png') { if ($f.Name -match '^(flag|frame)-(\d{3}|final)\.png$') { $h[$Matches[2]] = $f.FullName } }; $h }
$ho = Index $Off; $hn = Index $On
$rows = foreach ($k in $ho.Keys) {
  if ($hn.ContainsKey($k)) { $d = [Ab]::Diff($ho[$k], $hn[$k]); [pscustomobject]@{ name = $k; changed = $d[0]; dluma = $d[1]; off = $ho[$k]; on = $hn[$k] } }
}
$rows | Sort-Object name | ForEach-Object { "{0}`t{1:F4}`t{2:F2}" -f $_.name, $_.changed, $_.dluma } | Set-Content -Encoding utf8 (Join-Path $Out 'diff.tsv')
$top = $rows | Sort-Object changed -Descending | Select-Object -First 3
foreach ($t in $top) { [Ab]::Stack($t.off, $t.on, (Join-Path $Out ("pair-" + $t.name + ".png"))) }
$f = $rows | Where-Object name -eq 'final'
if ($f) { [Ab]::Stack($f.off, $f.on, (Join-Path $Out 'pair-final.png')) }
$c1 = @($rows | Where-Object { $_.changed -gt 0.01 }).Count
"compare frames=$(@($rows).Count) changed_over_1pct=$c1 max_changed=$('{0:F4}' -f ($top[0].changed)) top=$(($top | ForEach-Object name) -join ',') out=$Out"
