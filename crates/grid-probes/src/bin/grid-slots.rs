//! Research probe: a route's grid slots against its racing line: arc, lateral
//! offset and heading of each slot. `grid-slots ROOT ROUTE [SUBROUTE]`.
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        return Err("usage: grid-slots ROOT ROUTE [SUBROUTE]".into());
    }
    let root = Path::new(&a[1]);
    let mut route = grid_tools::route::Route::new(&a[2])?;
    if let Some(s) = a.get(3) {
        route = route.with_subroute(s.parse()?)?;
    }
    let line = grid_tools::ai_driver::Line::load_route(root, &route)?;
    let points = line.points();
    let bytes = std::fs::read(root.join(route.file("grids.pssg")))?;
    let tree = grid_formats::pssg::read_tree(&bytes)?;
    for slot in 0..16 {
        let name = format!("slot_{slot:02}");
        let Ok(m) = grid_formats::grid::read_slot(&tree, name.as_bytes(), route.subroute()) else {
            println!("{name} none");
            continue;
        };
        let c = m.center;
        // The nearest racing-line segment in x/z: lateral offset and tangent.
        let mut best = (f32::MAX, 0., [0f32; 2]);
        for i in 0..points.len() {
            let (p, q) = (points[i], points[(i + 1) % points.len()]);
            let d = [q[0] - p[0], q[2] - p[2]];
            let l2 = d[0] * d[0] + d[1] * d[1];
            let t = (((c[0] - p[0]) * d[0] + (c[2] - p[2]) * d[1]) / l2).clamp(0., 1.);
            let e = [p[0] + t * d[0] - c[0], p[2] + t * d[1] - c[2]];
            let dist = (e[0] * e[0] + e[1] * e[1]).sqrt();
            if dist < best.0 {
                let side = d[0] * (c[2] - p[2]) - d[1] * (c[0] - p[0]);
                let l = l2.sqrt();
                best = (dist, side.signum() * dist, [d[0] / l, d[1] / l]);
            }
        }
        let f = m.forward;
        let fl = (f[0] * f[0] + f[2] * f[2]).sqrt();
        let align = (f[0] * best.2[0] + f[2] * best.2[1]) / fl;
        println!(
            "{name} centre=({:.1},{:.1},{:.1}) arc={:.1} lateral={:.2} heading_dot={align:.3}",
            c[0],
            c[1],
            c[2],
            line.arc(c),
            best.1
        );
    }
    Ok(())
}
