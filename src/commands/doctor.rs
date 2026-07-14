//! `orbit doctor` — Flutter-style health check. Every line is a check with a
//! clear ✓ / ✗ / ! and a fix. If everything is green, orbit is guaranteed to work.

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::config::{self, Config};
use crate::docker_ctx;
use crate::forwarder;
use crate::metrics;
use crate::ssh;
use crate::util;

pub async fn run() -> Result<()> {
    util::header("orbit doctor");
    println!();
    let mut problems = 0u32;

    // [✓] Docker CLI
    if util::succeeds("docker", &["version", "--format", "{{.Client.Version}}"]).await {
        let v = util::run("docker", &["version", "--format", "{{.Client.Version}}"])
            .await
            .unwrap_or_default();
        pass(&format!("Docker CLI ({})", v.trim()));
    } else {
        problems += 1;
        fail("Docker CLI not found", "install Docker / OrbStack / Rancher Desktop");
    }

    // [✓] Linked
    let cfg = match Config::load() {
        Ok(c) => {
            pass(&format!("Linked to {}", c.ssh_target()));
            Some(c)
        }
        Err(_) => {
            fail("Not linked to a host", "run `orbit setup` (guided) or `orbit link <user@host>`");
            return verdict(problems + 1, None);
        }
    };
    let cfg = cfg.unwrap();

    // [✓] SSH
    if ssh::test_connection(&cfg).await.is_ok() {
        pass("SSH to the host works");
    } else {
        problems += 1;
        fail(
            "Cannot SSH to the host",
            "check the host is on, Remote Login/SSH is enabled, and the key is authorized (`orbit setup` re-does this)",
        );
    }

    // [✓] Remote docker daemon socket
    let socket_ok = ssh::remote_exec(&cfg, &format!("test -S {} && echo yes", cfg.remote_socket))
        .await
        .map(|o| o.trim() == "yes")
        .unwrap_or(false);
    if socket_ok {
        pass(&format!("Remote Docker daemon ({})", cfg.remote_socket));
    } else {
        problems += 1;
        fail(
            "Remote Docker socket not found",
            "is Docker running on the host? re-run `orbit setup` to re-detect it",
        );
    }

    // [✓] Connection (master + forwarded socket)
    let mut connected = false;
    if ssh::master_alive(&cfg).await {
        if let Ok(sock) = config::local_docker_socket() {
            match forwarder::connect(&sock) {
                Ok(d) if d.ping().await.is_ok() => {
                    pass("Connection up — forwarded Docker socket responds");
                    connected = true;
                }
                Ok(_) => {
                    problems += 1;
                    fail("Forwarded socket not responding", "run `orbit down` then `orbit up`");
                }
                Err(_) => {
                    problems += 1;
                    fail("Forwarded socket missing", "run `orbit up`");
                }
            }
        }
    } else {
        note("orbit is not up yet", "run `orbit up` (or `orbit setup`) to delegate Docker");
    }

    // [✓] Docker routing
    let ctx = docker_ctx::current_context().await.unwrap_or_default();
    if ctx == cfg.context_name {
        pass("Docker is routed to the host");
    } else {
        note(
            &format!("Docker is on `{ctx}` (local)"),
            "run `orbit up` to delegate to the host",
        );
    }

    // Remote specs, when connected — a taste of what you're borrowing.
    let mut specs = None;
    if connected {
        if let Ok(sock) = config::local_docker_socket() {
            if let Some(r) = metrics::remote_metrics(&sock).await {
                println!(
                    "\n  {} host has {} cores · {} · {} images · {} running",
                    "◇".cyan(),
                    r.ncpu,
                    metrics::fmt_gib(r.mem_total),
                    r.images,
                    r.running,
                );
                specs = Some(cfg.ssh_target());
            }
        }
    }

    verdict(problems, specs.map(|_| cfg.ssh_target()))
}

fn verdict(problems: u32, host: Option<String>) -> Result<()> {
    println!();
    if problems == 0 {
        let where_ = host
            .map(|h| format!(" — Docker runs on {h}"))
            .unwrap_or_default();
        println!(
            "{} {}{}",
            "•".green().bold(),
            "No issues found! orbit is ready".green().bold(),
            where_
        );
    } else {
        println!(
            "{} {}",
            "•".yellow().bold(),
            format!("{problems} issue(s) found — see the fixes above").yellow()
        );
    }
    Ok(())
}

fn pass(msg: &str) {
    println!("  {} {msg}", "[✓]".green().bold());
}
fn fail(msg: &str, fix: &str) {
    println!("  {} {}", "[✗]".red().bold(), msg.red());
    println!("      {} {fix}", "→".dimmed());
}
fn note(msg: &str, fix: &str) {
    println!("  {} {msg}", "[!]".yellow().bold());
    println!("      {} {fix}", "→".dimmed());
}
