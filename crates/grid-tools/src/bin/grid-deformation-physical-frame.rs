//! Whole physical frame with registered static-contact preparation.
#[allow(dead_code)]
#[path = "grid-physical-frame.rs"]
mod diagnostic;
fn main() {
    if let Err(error) = diagnostic::run(true, false) {
        eprintln!("grid-deformation-physical-frame: {error}");
        std::process::exit(1);
    }
}
