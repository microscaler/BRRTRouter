use crate::{load_spec, router::Router, dispatcher::Dispatcher, hot_reload::watch_spec, server::{AppService, HttpServer}};
use clap::{Parser, Subcommand};
use may::sync::mpsc;
use may::coroutine;
use std::sync::{Arc, RwLock};
use std::io;
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
    /// Run the server for a spec using echo handlers
    Serve {
        #[arg(short, long)]
        spec: PathBuf,

        #[arg(long, default_value_t = false)]
        watch: bool,

        #[arg(long, default_value = "0.0.0.0:8080")]
        addr: String,
    },
}

pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Generate { spec, force } => {
            let (_routes, _slug) = load_spec(spec.to_str().unwrap())?;
            let project_dir = crate::generator::generate_project_from_spec(spec.as_path(), *force)
                .expect("failed to generate example project");
            // Format the newly generated project
            if let Err(e) = crate::generator::format_project(&project_dir) {
                eprintln!("cargo fmt failed: {e}");
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
            let mut service = AppService::new(router.clone(), dispatcher.clone(), schemes, spec.clone(), None, None);
            if *watch {
                let watcher = watch_spec(spec.clone(), router.clone(), dispatcher.clone(), |disp, new_routes| {
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
                })?;
                service.watcher = Some(watcher);
            }
            let handle = HttpServer(service).start(addr)?;
            handle.join().map_err(|e| Box::<dyn std::error::Error>::from(io::Error::other(format!("{e:?}"))))?;
            Ok(())
        }
    }
}
