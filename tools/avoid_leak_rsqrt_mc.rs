// Monte Carlo of the float angle test in the original's racing-normally exclusion 0x79A530.
//
// Build and run (x86_64 only): rustc -O -o local/avoid-leak-mc.exe tools/avoid_leak_rsqrt_mc.rs
//
// The original computes cos = dot_xz(t, f) / (sqrt~(|t|^2) * sqrt~(|f_xz|^2)) in f32, where
// sqrt~(x) = s + (1 - s*r) * (s*0.5) with r = rsqrtps(x), s = r*x (one Newton step, which
// approaches from below). acos (0xBF4067) returns NaN for |cos| > 1, and `comiss`/`ja` then
// reports "not racing normally", so avoid may fire. This prints P(NaN) per bin of the
// horizontal deviation from antiparallel for the host CPU's rsqrtps and portable models.
// See docs/AI_AVOID_RECOVERY_LIVE.md "The 0.36% leak".
use std::arch::x86_64::*;

fn rsqrt(model: &str, x: f32) -> f32 {
    let exact = (1.0 / (x as f64).sqrt()) as f32;
    match model {
        "host" => unsafe { _mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(x))) },
        "exact" => exact,
        "t12" => f32::from_bits(exact.to_bits() & !0xFFF),
        "t11" => f32::from_bits(exact.to_bits() & !0x7FF),
        _ => unreachable!(),
    }
}

fn sqrt_est(model: &str, x: f32) -> f32 {
    if x == 0.0 {
        return 0.0;
    }
    let r = rsqrt(model, x);
    let s = r * x;
    (1.0 - s * r) * (s * 0.5) + s
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn main() {
    let edges: Vec<f64> = (0..=8).map(|k| k as f64 * 1e-4).collect();
    println!("bins of deviation from antiparallel, 1e-4 rad each, 0..8e-4");
    for model in ["host", "t12", "t11", "exact"] {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut out = vec![];
        for w in edges.windows(2) {
            let (mut hit, n) = (0, 200_000);
            for _ in 0..n {
                // gate direction t = (-n.z, 0, n.x) of a unit gate normal
                let yaw = rng.next() * std::f64::consts::TAU;
                let (tx, tz) = (-(yaw.sin() as f32), yaw.cos() as f32);
                // unit 3D record forward, pitch within +-5.7 deg, heading against t by d
                let d = (w[0] + (w[1] - w[0]) * rng.next()) * if rng.next() < 0.5 { 1. } else { -1. };
                let h = ((rng.next() - 0.5) * 0.2).cos();
                let (fx, fz) = (((yaw + d).sin() * h) as f32, (-(yaw + d).cos() * h) as f32);
                let c = (tx * fx + tz * fz)
                    / (sqrt_est(model, tx * tx + tz * tz) * sqrt_est(model, fx * fx + fz * fz));
                if (c as f64).acos().is_nan() {
                    hit += 1;
                }
            }
            out.push(format!("{:.2}", hit as f64 / n as f64));
        }
        println!("{model:5} {}", out.join(" "));
    }
}
