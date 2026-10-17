//! Owned wheel update and publication at a completed geometry rebuild boundary.
#[allow(dead_code)]
#[path = "grid-vehicle-publication.rs"]
mod diagnostic;
fn main() {
    if let Err(error) = diagnostic::run_mode(true, true) {
        eprintln!("grid-wheel-deformation-publication: {error}");
        std::process::exit(1);
    }
}
