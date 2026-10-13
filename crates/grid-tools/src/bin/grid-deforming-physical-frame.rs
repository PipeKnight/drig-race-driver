//! Whole physical frame with both registered static deformation phases.
#[allow(dead_code)]
#[path = "grid-physical-frame.rs"]
mod diagnostic;
fn main() {
    if let Err(error) = diagnostic::run(true, true) {
        eprintln!("grid-deforming-physical-frame: {error}");
        std::process::exit(1);
    }
}
