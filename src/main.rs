mod cli;
mod commands;
mod config;
mod docker_ctx;
mod forwarder;
mod host;
mod ssh;
mod util;

use clap::Parser;
use cli::{Cli, Command, HostCmd, PortsCmd};

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    init_tracing(args.verbose);

    let result = match args.command {
        Command::Host { cmd } => match cmd {
            HostCmd::Init => commands::host_init::run().await,
        },
        Command::Link {
            target,
            port,
            socket,
        } => commands::link::run(&target, port, socket).await,
        Command::Up { foreground } => commands::up::run(foreground).await,
        Command::Down => commands::down::run().await,
        Command::Status => commands::status::run().await,
        Command::Ports { cmd } => match cmd {
            Some(PortsCmd::Add { port }) => commands::ports::add(port).await,
            Some(PortsCmd::Rm { port }) => commands::ports::rm(port).await,
            None => commands::ports::list().await,
        },
        Command::Doctor => commands::doctor::run().await,
        Command::Forward => commands::run_worker::run().await,
    };

    if let Err(e) = result {
        eprintln!("\n{} {:#}", owo_colors::OwoColorize::red(&"error:"), e);
        std::process::exit(1);
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("orbit={level}")));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .init();
}
