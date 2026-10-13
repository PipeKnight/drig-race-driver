//! Owned static-car raw adapter using the existing private serial protocol.
fn main() {
    if let Err(error) = grid_tools::impact_diagnostic::run_owned_static() {
        eprintln!("grid-owned-deformation-callback: {error}");
        std::process::exit(1);
    }
}
