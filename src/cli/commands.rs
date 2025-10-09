use crate::{
    dispatcher::Dispatcher,
    hot_reload::watch_spec,
    load_spec,
    router::Router,
    server::{AppService, HttpServer},
};
use clap::{Parser, Subcommand, ValueEnum};
use may::coroutine;
use may::sync::mpsc;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Command-line interface for BRRTRouter
///
/// Provides commands for generating code from OpenAPI specifications
/// and running development servers.
#[derive(Parser)]
#[command(name = "brrrouter")]
#[command(about = "BRRTRouter CLI", long_about = None)]
pub struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands for BRRTRouter
#[derive(Subcommand)]
pub enum Commands {
    /// Generate handler stubs from an OpenAPI spec
    Generate {
        /// Path to the OpenAPI specification file (YAML or JSON)
        #[arg(short, long)]
        spec: PathBuf,

        /// Overwrite existing files without prompting
        #[arg(short, long, default_value_t = false)]
        force: bool,

        /// Perform a dry run: show what would change without writing files
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Limit regeneration to specific parts (comma-separated or repeated)
        #[arg(long, value_enum, num_args = 1.., value_delimiter = ',')]
        only: Option<Vec<OnlyPart>>,
    },
    /// Run the server for a spec using echo handlers
    Serve {
        /// Path to the OpenAPI specification file (YAML or JSON)
        #[arg(short, long)]
        spec: PathBuf,

        /// Watch for changes and hot-reload the server
        #[arg(long, default_value_t = false)]
        watch: bool,

        /// Address and port to bind the server to
        #[arg(long, default_value = "0.0.0.0:8080")]
        addr: String,
    },
}

/// Specific parts of the generated project that can be selectively regenerated
///
/// Used with the `--only` flag to limit code generation to specific components.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OnlyPart {
    /// Handler modules (request/response types and handler logic)
    Handlers,
    /// Controller modules (coroutine-based request dispatching)
    Controllers,
    /// Type definitions derived from OpenAPI schemas
    Types,
    /// Handler registry (registration of all handlers with the dispatcher)
    Registry,
    /// Main application entry point
    Main,
    /// Documentation files (OpenAPI spec, HTML docs)
    Docs,
}

/// Execute the CLI command provided by the user
///
/// # Errors
///
/// Returns an error if:
/// - The OpenAPI spec cannot be loaded or parsed
/// - Code generation fails
/// - The server fails to start
/// - Hot reload watcher setup fails
pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Generate {
            spec,
            force,
            dry_run,
            only,
        } => {
            let (_routes, _slug) = load_spec(spec.to_str().unwrap())?;
            let scope = map_only_to_scope(only.as_deref());
            let project_dir = crate::generator::generate_project_with_options(
                spec.as_path(),
                *force,
                *dry_run,
                &scope,
            )
            .expect("failed to generate example project");
            // Format the newly generated project
            if !*dry_run {
                if let Err(e) = crate::generator::format_project(&project_dir) {
                    eprintln!("cargo fmt failed: {e}");
                }
            }
            Ok(())
        }
        Commands::Serve { spec, watch, addr } => {
            let (routes, schemes, _slug) = crate::spec::load_spec_full(spec.to_str().unwrap())?;
            let router = Arc::new(RwLock::new(Router::new(routes.clone())));
            let mut dispatcher = Dispatcher::new();
            for r in &routes {
                let (tx, rx) = mpsc::channel();
                unsafe {
                    coroutine::spawn(move || {
                        for req in rx.iter() {
                            crate::echo::echo_handler(req);
                        }
                    });
                }
                dispatcher.add_route(r.clone(), tx);
            }
            let dispatcher = Arc::new(RwLock::new(dispatcher));
            let mut service = AppService::new(
                router.clone(),
                dispatcher.clone(),
                schemes,
                spec.clone(),
                None,
                None,
            );
            if *watch {
                let watcher = watch_spec(
                    spec.clone(),
                    router.clone(),
                    dispatcher.clone(),
                    |disp, new_routes| {
                        for r in &new_routes {
                            let (tx, rx) = mpsc::channel();
                            unsafe {
                                coroutine::spawn(move || {
                                    for req in rx.iter() {
                                        crate::echo::echo_handler(req);
                                    }
                                });
                            }
                            disp.add_route(r.clone(), tx);
                        }
                    },
                )?;
                service.watcher = Some(watcher);
            }
            let handle = HttpServer(service).start(addr)?;
            handle.join().map_err(|e| {
                Box::<dyn std::error::Error>::from(io::Error::other(format!("{e:?}")))
            })?;
            Ok(())
        }
    }
}

/// Convert CLI `--only` parts to a `GenerationScope` configuration
///
/// If `only` is `None`, all parts are enabled. If `only` is provided,
/// only the specified parts are enabled.
fn map_only_to_scope(only: Option<&[OnlyPart]>) -> crate::generator::GenerationScope {
    use crate::generator::GenerationScope as Scope;
    let mut scope = Scope::all();
    if let Some(parts) = only {
        // Start with nothing, then enable selected parts
        scope = Scope {
            handlers: false,
            controllers: false,
            types: false,
            registry: false,
            main: false,
            docs: false,
        };
        for p in parts {
            match p {
                OnlyPart::Handlers => scope.handlers = true,
                OnlyPart::Controllers => scope.controllers = true,
                OnlyPart::Types => scope.types = true,
                OnlyPart::Registry => scope.registry = true,
                OnlyPart::Main => scope.main = true,
                OnlyPart::Docs => scope.docs = true,
            }
        }
    }
    scope
}
