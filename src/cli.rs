//! Command-line surface.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "orbit",
    version,
    about = "Delegate Docker to a beefier machine on your LAN, transparently.",
    long_about = "orbit redirects your local `docker` to a remote engine over SSH and \
automatically forwards published container ports back to localhost, so heavy builds and \
containers run on another machine while you keep working on this one.\n\n\
New here? Run `orbit setup` — it walks you through everything in about two minutes."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Increase log verbosity (-v info, -vv debug, -vvv trace — logs every ssh/forward action).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Also write logs to this file (in addition to the terminal).
    #[arg(long, global = true, value_name = "PATH")]
    pub log_file: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Guided, zero-friction setup. The easiest way to start — interactive.
    Setup {
        /// Skip host discovery and use this address (IP or hostname).
        #[arg(long)]
        host: Option<String>,
        /// SSH user on the host (defaults to your current username).
        #[arg(long)]
        user: Option<String>,
        /// SSH port on the host.
        #[arg(long, default_value_t = 22)]
        port: u16,
        /// Don't prompt — accept detected defaults (for scripts/CI).
        #[arg(long)]
        yes: bool,
        /// Skip the end-to-end self-test container after linking.
        #[arg(long)]
        no_test: bool,
    },

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

    /// Show the forwarder log.
    Logs {
        /// Follow the log (like `tail -f`).
        #[arg(short, long)]
        follow: bool,
        /// Number of trailing lines to print first.
        #[arg(short = 'n', long, default_value_t = 200)]
        lines: usize,
    },

    /// Run orbit as a background login service (launchd/systemd).
    Service {
        #[command(subcommand)]
        cmd: ServiceCmd,
    },

    /// Start the MCP server (stdio) so AI assistants can drive orbit.
    Mcp,

    /// Internal: the detached forwarder worker. Not for direct use.
    #[command(hide = true)]
    Forward,
}

#[derive(Subcommand, Debug)]
pub enum HostCmd {
    /// Detect the docker engine and print the join string for clients.
    Init,
    /// Guided host-side setup: check Docker + SSH and print the join string.
    Setup,
    /// Authorize a client's SSH public key on this host (append to authorized_keys).
    AddKey {
        /// The full public key line, e.g. "ssh-ed25519 AAAA... orbit".
        pubkey: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum PortsCmd {
    /// Manually forward an extra TCP port (e.g. a non-docker service on the host).
    Add { port: u16 },
    /// Stop forwarding a manually-added port.
    Rm { port: u16 },
}

#[derive(Subcommand, Debug)]
pub enum ServiceCmd {
    /// Install a login service that keeps `orbit up` running.
    Install,
    /// Remove the orbit login service.
    Uninstall,
    /// Show whether the orbit login service is installed and running.
    Status,
}
