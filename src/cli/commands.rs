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

        /// Output directory for generated project (default: examples/{slug})
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Overwrite existing files without prompting
        #[arg(short, long, default_value_t = false)]
        force: bool,

        /// Perform a dry run: show what would change without writing files
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Limit regeneration to specific parts (comma-separated or repeated)
        #[arg(long, value_enum, num_args = 1.., value_delimiter = ',')]
        only: Option<Vec<OnlyPart>>,

        /// Version for generated Cargo.toml [package].version
        /// If not provided, defaults to "0.1.0"
        #[arg(long, default_value = "0.1.0")]
        version: String,

        /// Package name for generated Cargo.toml [package].name (e.g. rerp_accounting_financial_reports_gen)
        /// If not provided, derived from OpenAPI spec info.title (slug)
        #[arg(long)]
        package_name: Option<String>,

        /// Path to dependencies configuration file (brrtrouter-dependencies.toml)
        /// If not provided, will auto-detect alongside the OpenAPI spec
        #[arg(long)]
        dependencies_config: Option<PathBuf>,
    },
    /// Generate implementation stubs in impl crate
    ///
    /// Creates stub files for controllers in the {component}_impl crate.
    /// Stubs are NOT auto-regenerated - they are user-owned once created.
    /// Use --force to overwrite existing stubs (per-path basis).
    /// Handlers that contain the sentinel (e.g. // BRRTRouter: user-owned) are
    /// never overwritten by --force; use --sync to patch only signature/Response shape.
    GenerateStubs {
        /// Path to the OpenAPI specification file (YAML or JSON)
        #[arg(short, long)]
        spec: PathBuf,

        /// Output directory for impl crate (e.g., crates/bff_impl or crates/impl)
        #[arg(short, long)]
        output: PathBuf,

        /// Component name (e.g., "bff" or "rerp_accounting_general_ledger_gen")
        /// If not provided, will be derived from output directory name (stripping "_impl" suffix)
        #[arg(long)]
        component_name: Option<String>,

        /// Generate stub for specific handler only (per-path basis)
        #[arg(short, long)]
        path: Option<String>,

        /// Overwrite existing stub files (required to regenerate)
        #[arg(short, long, default_value_t = false)]
        force: bool,

        /// Sync only: patch handler signature and Response struct literal to match spec; do not overwrite body. Only affects files that contain the user-owned sentinel.
        #[arg(long, default_value_t = false)]
        sync: bool,
    },
    /// Lint an OpenAPI specification
    ///
    /// Checks the specification for common issues and best practices:
    /// - operationId casing (must be snake_case)
    /// - Schema format consistency
    /// - Missing type definitions
    /// - Schema completeness
    /// - Missing operationId
    /// - Schema reference resolution
    Lint {
        /// Path to the OpenAPI specification file (YAML or JSON)
        #[arg(short, long)]
        spec: PathBuf,

        /// Exit with error code if any errors are found
        #[arg(long, default_value_t = false)]
        fail_on_error: bool,

        /// Show only errors (hide warnings and info)
        #[arg(long, default_value_t = false)]
        errors_only: bool,
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
            output,
            force,
            dry_run,
            only,
            version,
            package_name,
            dependencies_config,
        } => {
            let spec_path = spec
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in spec path"))?;
            let (_routes, _slug) = load_spec(spec_path)?;
            let scope = map_only_to_scope(only.as_deref());
            let project_dir = crate::generator::generate_project_with_options(
                spec.as_path(),
                output.as_deref(),
                *force,
                *dry_run,
                &scope,
                Some(version.clone()),
                package_name.as_deref(),
                dependencies_config.as_deref(),
            )
            .expect("failed to generate example project");
            // Format the newly generated project (single implementation: generator owns fmt)
            if !*dry_run {
                crate::generator::format_project(&project_dir)?;
            }
            Ok(())
        }
        Commands::GenerateStubs {
            spec,
            output,
            component_name,
            path,
            force,
            sync,
        } => {
            crate::generator::generate_impl_stubs(
                spec.as_path(),
                output.as_path(),
                component_name.as_deref(),
                path.as_deref(),
                *force,
                *sync,
            )?;
            // Format generated stubs using the same implementation as generate
            crate::generator::format_project(output.as_path())?;
            Ok(())
        }
        Commands::Lint {
            spec,
            fail_on_error,
            errors_only,
        } => {
            let issues = crate::linter::lint_spec(spec.as_path())?;

            if *errors_only {
                let errors: Vec<_> = issues
                    .iter()
                    .filter(|i| i.severity == crate::linter::LintSeverity::Error)
                    .cloned()
                    .collect();
                crate::linter::print_lint_issues(&errors);
                if *fail_on_error && !errors.is_empty() {
                    crate::linter::fail_if_errors(&errors);
                }
            } else {
                crate::linter::print_lint_issues(&issues);
                if *fail_on_error {
                    crate::linter::fail_if_errors(&issues);
                }
            }

            Ok(())
        }
        Commands::Serve { spec, watch, addr } => {
            let spec_path = spec
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in spec path"))?;
            let (routes, schemes, _slug) = crate::spec::load_spec_full(spec_path)?;
            let router = Arc::new(RwLock::new(Router::new(routes.clone())));
            let mut dispatcher = Dispatcher::new();
            for r in &routes {
                let (tx, rx) = mpsc::channel();
                // SAFETY: may::coroutine::spawn() is marked unsafe by the may runtime.
                // Safe because: May runtime is initialized, handler is Send + 'static
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
                Arc::clone(&router),
                Arc::clone(&dispatcher),
                schemes,
                spec.clone(),
                None,
                None,
            );
            if *watch {
                let watcher = watch_spec(
                    spec.clone(),
                    Arc::clone(&router),
                    Arc::clone(&dispatcher),
                    Some(service.validator_cache.clone()),
                    |disp, new_routes| {
                        for r in &new_routes {
                            let (tx, rx) = mpsc::channel();
                            // SAFETY: may::coroutine::spawn() is marked unsafe by the may runtime.
                            // Safe because: May runtime is initialized, handler is Send + 'static
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
