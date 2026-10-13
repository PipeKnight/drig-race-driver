//! Whole publication after rebuilding, including owned light/wheel state.
#[allow(dead_code)]
#[path = "grid-vehicle-publication.rs"]
mod diagnostic;
fn main() {
    if let Err(error) = diagnostic::run(true) {
        eprintln!("grid-deformation-publication: {error}");
        std::process::exit(1);
    }
}
