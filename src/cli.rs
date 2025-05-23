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
        #[command(subcommand)]
        sub: GenerateSubcommand,
    },
}

#[derive(Subcommand)]
pub enum GenerateSubcommand {
    /// Generate handler files from OpenAPI spec
    Handlers {
        #[arg(short, long)]
        spec: PathBuf,

        #[arg(short, long, default_value = "src/handlers")]
        out: PathBuf,

        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
}

pub fn run_cli() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Generate { sub } => match sub {
            GenerateSubcommand::Handlers { spec, out, force } => {
                crate::generator::generate_handlers_from_spec(spec, out, *force)
                    .expect("failed to generate handlers");
            }
        },
    }
}
