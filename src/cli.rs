use crate::load_spec;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "brrrouter")]
#[command(about = "BRRTRouter CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate handler stubs from an OpenAPI spec
    Generate {
        #[arg(short, long)]
        spec: PathBuf,

        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
}

pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Generate { spec, force } => {
            let (routes, slug) = load_spec(spec.to_str().unwrap(), false)?;
            crate::generator::generate_project_from_spec(spec.as_path(), *force)
                .expect("failed to generate example project");
            Ok(())
        }
    }
}
