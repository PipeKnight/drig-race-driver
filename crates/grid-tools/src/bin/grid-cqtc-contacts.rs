fn main() {
    if let Err(e) = grid_tools::contact_diagnostic::run(true) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
