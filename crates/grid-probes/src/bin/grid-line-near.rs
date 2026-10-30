//! Research probe: the racing-line points of a route near a world point.
//! `grid-line-near ROOT ROUTE X Y Z [RADIUS]`.
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        return Err("usage: grid-line-near ROOT ROUTE X Y Z [RADIUS]".into());
    }
    let route = grid_tools::route::Route::new(&a[2])?;
    let line = grid_tools::ai_driver::Line::load_route(Path::new(&a[1]), &route)?;
    let p: [f32; 3] = [a[3].parse()?, a[4].parse()?, a[5].parse()?];
    let radius: f32 = a.get(6).map_or(Ok(40.), |r| r.parse())?;
    let (arc, lateral, racing, track) = line.limits_near(p);
    println!(
        "arc_at_point={arc:.1} lateral={lateral:.2} racing_limits={racing:?} track_limits={track:?} points={}",
        line.points().len()
    );
    for (i, q) in line.points().iter().enumerate() {
        let d = ((q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
        if d <= radius {
            println!("{i} {:.2} {:.2} {:.2} d={d:.1}", q[0], q[1], q[2]);
        }
    }
    let props = grid_tools::entity_geometry::Props::read_route(Path::new(&a[1]), &route)?;
    for (i, b) in props.boxes.iter().enumerate() {
        let near = (0..3).all(|j| p[j] >= b.bounds[j] - 6. && p[j] <= b.bounds[j + 3] + 6.);
        if near {
            println!(
                "box {i} {} entity={:?} enabled={} bounds={:?}",
                b.template, b.entity, b.enabled, b.bounds
            );
        }
    }
    Ok(())
}
