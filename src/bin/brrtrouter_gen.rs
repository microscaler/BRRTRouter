use anyhow::Result;
use brrtrouter::cli::run_cli;

fn main() -> Result<()> {
    // Debug the User schema processing
    brrtrouter::generator::schema::debug_user_schema();
    
    // Continue with normal CLI
    run_cli()
}
