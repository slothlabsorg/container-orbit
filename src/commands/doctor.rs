//! `orbit doctor` — run through the setup and report actionable problems.

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::config::{self, Config};
use crate::docker_ctx;
use crate::forwarder;
use crate::ssh;
use crate::util;

pub async fn run() -> Result<()> {
    util::header("orbit doctor");
    let mut problems = 0u32;

    // docker present
    if util::succeeds("docker", &["version", "--format", "{{.Client.Version}}"]).await {
        pass("docker CLI is installed");
    } else {
        problems += 1;
        fail(
            "docker CLI not found",
            "install Docker / OrbStack / Rancher Desktop",
        );
    }

    // config
    let cfg = match Config::load() {
        Ok(c) => {
            pass(&format!("linked to {}", c.ssh_target()));
            c
        }
        Err(_) => {
            fail("not linked", "run `orbit link <user@host>`");
            return done(problems + 1);
        }
    };

    // ssh
    if ssh::test_connection(&cfg).await.is_ok() {
        pass("SSH to host works");
    } else {
        problems += 1;
        fail(
            "cannot SSH to host",
            "check the host is on, SSH/Remote-Login is enabled, and the key is authorized",
        );
    }

    // remote docker socket present (checked over ssh — no remote PATH needed)
    let socket_ok = ssh::remote_exec(&cfg, &format!("test -S {} && echo yes", cfg.remote_socket))
        .await
        .map(|o| o.trim() == "yes")
        .unwrap_or(false);
    if socket_ok {
        pass(&format!("remote docker socket present ({})", cfg.remote_socket));
    } else {
        problems += 1;
        fail(
            "remote docker socket not found",
            "is Docker running on the host? re-run `orbit link` to re-detect it",
        );
    }

    // master + forwarded socket
    if ssh::master_alive(&cfg).await {
        pass("SSH master connection is up");
        if let Ok(sock) = config::local_docker_socket() {
            match forwarder::connect(&sock) {
                Ok(d) if d.ping().await.is_ok() => pass("forwarded docker socket responds"),
                Ok(_) => {
                    problems += 1;
                    fail(
                        "forwarded socket not responding",
                        "run `orbit down` then `orbit up`",
                    );
                }
                Err(_) => {
                    problems += 1;
                    fail("forwarded socket missing", "run `orbit up`");
                }
            }
        }
    } else {
        note("SSH master is down — run `orbit up` to connect");
    }

    // active context
    let ctx = docker_ctx::current_context().await.unwrap_or_default();
    if ctx == cfg.context_name {
        pass("docker is currently pointing at the host");
    } else {
        note(&format!(
            "docker is on `{ctx}` (local). Run `orbit up` to delegate."
        ));
    }

    done(problems)
}

fn done(problems: u32) -> Result<()> {
    println!();
    if problems == 0 {
        util::ok("all good");
    } else {
        util::warn(&format!("{problems} problem(s) found — see fixes above"));
    }
    Ok(())
}

fn pass(msg: &str) {
    println!("  {} {msg}", "✓".green().bold());
}
fn fail(msg: &str, fix: &str) {
    println!("  {} {}", "✗".red().bold(), msg.red());
    println!("      {} {fix}", "fix:".dimmed());
}
fn note(msg: &str) {
    println!("  {} {msg}", "•".yellow().bold());
}
