mod cli;
mod commands;
mod config;
mod docker_ctx;
mod forwarder;
mod host;
mod mcp;
mod net_scan;
mod ssh;
mod util;

use clap::Parser;
use cli::{Cli, Command, HostCmd, PortsCmd, ServiceCmd};

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    init_tracing(args.verbose, args.log_file.as_deref());

    let result = match args.command {
        Command::Setup {
            host,
            user,
            port,
            yes,
            no_test,
        } => commands::setup::run(host, user, port, yes, no_test).await,
        Command::Host { cmd } => match cmd {
            HostCmd::Init => commands::host_init::run().await,
            HostCmd::Setup => commands::host_setup::run().await,
            HostCmd::AddKey { pubkey } => commands::host_setup::add_key(&pubkey).await,
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
        Command::Logs { follow, lines } => commands::logs::run(follow, lines).await,
        Command::Service { cmd } => match cmd {
            ServiceCmd::Install => commands::service::install().await,
            ServiceCmd::Uninstall => commands::service::uninstall().await,
            ServiceCmd::Status => commands::service::status().await,
        },
        Command::Mcp => commands::mcp::run().await,
        Command::Funding => commands::funding::run().await,
        Command::Forward => commands::run_worker::run().await,
    };

    if let Err(e) = result {
        eprintln!("\n{} {:#}", owo_colors::OwoColorize::red(&"error:"), e);
        std::process::exit(1);
    }
}

fn init_tracing(verbose: u8, log_file: Option<&str>) {
    use tracing_subscriber::prelude::*;

    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let make_filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("orbit={level}")))
    };

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .with_filter(make_filter());

    let file_layer = log_file.and_then(|path| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(|f| {
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_ansi(false)
                    .with_writer(f)
                    .with_filter(make_filter())
            })
    });

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();
}
