fn main() {
    if let Err(e) = grid_tools::contact_diagnostic::run(false) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
