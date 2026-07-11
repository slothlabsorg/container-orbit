//! `orbit host init` — run on the machine that will lend its Docker engine.
//!
//! Detects the docker socket/adapter, confirms the daemon is reachable, and
//! prints the join string a client passes to `orbit link`.

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::host;
use crate::util;

pub async fn run() -> Result<()> {
    util::header("orbit host init");

    // 1. Docker reachable on this machine?
    util::step("Checking the local Docker engine…");
    if !util::succeeds("docker", &["info", "--format", "{{.ServerVersion}}"]).await {
        anyhow::bail!(
            "Docker is not reachable on this host.\n\
             Start Docker Desktop / OrbStack / Rancher (or the dockerd service) and retry."
        );
    }
    let server = util::run("docker", &["info", "--format", "{{.ServerVersion}}"]).await?;
    util::ok(&format!("Docker engine {server} is running"));

    // 2. Which adapter / socket?
    let info = host::detect_local()?;
    util::ok(&format!("Adapter: {} (socket {})", info.kind, info.socket));
    if !info.kind.supports_socket_forward() {
        util::warn(
            "this host needs the WSL2 bridge for automatic port forwarding — \
             see docs/ROADMAP.md. The docker context will still work.",
        );
    }

    // 3. SSH server reachable? We can't toggle it for the user, but we can nudge.
    let user = util::run("whoami", &[])
        .await
        .unwrap_or_else(|_| "<user>".into());
    let addr = lan_address().await;

    util::header("Share this with the client machine");
    println!("  Run on your laptop:\n");
    println!(
        "      {}",
        format!("orbit link {user}@{addr}").bold().green()
    );
    println!();
    util::info("user", &user);
    util::info("address", &addr);
    util::info("adapter", &info.kind.to_string());
    util::info("socket", &info.socket);

    util::header("If linking fails");
    println!("  • Enable remote login / SSH server on this machine:");
    println!("      macOS:  System Settings → General → Sharing → Remote Login");
    println!("      Linux:  sudo systemctl enable --now ssh");
    println!("  • Make sure both machines are on the same LAN.");
    Ok(())
}

/// Best-effort LAN IP. Falls back to the hostname.
async fn lan_address() -> String {
    // macOS: ipconfig over common interfaces.
    for iface in ["en0", "en1"] {
        if let Ok(ip) = util::run("ipconfig", &["getifaddr", iface]).await {
            if !ip.is_empty() {
                return ip;
            }
        }
    }
    // Linux: first global IPv4 from `hostname -I`.
    if let Ok(ips) = util::run("hostname", &["-I"]).await {
        if let Some(first) = ips.split_whitespace().next() {
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    util::run("hostname", &[])
        .await
        .unwrap_or_else(|_| "<this-host>".into())
}
