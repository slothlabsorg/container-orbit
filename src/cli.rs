//! Command-line surface.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "orbit",
    version,
    about = "Delegate Docker to a beefier machine on your LAN, transparently.",
    long_about = "orbit redirects your local `docker` to a remote engine over SSH and \
automatically forwards published container ports back to localhost, so heavy builds and \
containers run on another machine while you keep working on this one."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Increase log verbosity (-v, -vv).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Host side: prepare this machine to lend its Docker engine.
    Host {
        #[command(subcommand)]
        cmd: HostCmd,
    },

    /// Link this client to a host (`user@host`). Sets up the SSH key and docker context.
    Link {
        /// Target as `user@host` or just `host` (uses your current username).
        target: String,
        /// SSH port on the host.
        #[arg(long, default_value_t = 22)]
        port: u16,
        /// Override the remote docker socket path (auto-detected otherwise).
        #[arg(long)]
        socket: Option<String>,
    },

    /// Switch docker to the host and start forwarding ports (detached by default).
    Up {
        /// Run the forwarder in the foreground instead of detaching.
        #[arg(long)]
        foreground: bool,
    },

    /// Stop forwarding and restore your previous docker context.
    Down,

    /// Show link, connection, forwarded ports and remote resource usage.
    Status,

    /// List or manage forwarded ports.
    Ports {
        #[command(subcommand)]
        cmd: Option<PortsCmd>,
    },

    /// Diagnose the setup and suggest fixes.
    Doctor,

    /// Internal: the detached forwarder worker. Not for direct use.
    #[command(hide = true)]
    Forward,
}

#[derive(Subcommand, Debug)]
pub enum HostCmd {
    /// Detect the docker engine and print the join string for clients.
    Init,
}

#[derive(Subcommand, Debug)]
pub enum PortsCmd {
    /// Manually forward an extra TCP port (e.g. a non-docker service on the host).
    Add { port: u16 },
    /// Stop forwarding a manually-added port.
    Rm { port: u16 },
}
