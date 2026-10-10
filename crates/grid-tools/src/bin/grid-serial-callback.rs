fn main() {
    if let Err(error) = grid_tools::impact_diagnostic::run_serial() {
        eprintln!("grid-serial-callback: {error}");
        std::process::exit(1);
    }
}
