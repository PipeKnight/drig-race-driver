fn main() {
    if let Err(error) = grid_tools::impact_diagnostic::run(false) {
        eprintln!("grid-impact-producer: {error}");
        std::process::exit(1);
    }
}
