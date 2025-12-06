fn main() {
    if let Err(e) = brrtrouter::cli::run_cli() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
