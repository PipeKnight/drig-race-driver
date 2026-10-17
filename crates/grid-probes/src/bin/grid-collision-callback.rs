fn main() {
    if let Err(error) = grid_tools::impact_diagnostic::run(true) {
        eprintln!("grid-collision-callback: {error}");
        std::process::exit(1);
    }
}
