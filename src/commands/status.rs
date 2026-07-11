//! `orbit status` — link, connection, forwarded ports, remote resource usage.

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::config;
use crate::docker_ctx;
use crate::forwarder;
use crate::ssh;
use crate::util;

pub async fn run() -> Result<()> {
    let cfg = config::require_linked()?;
    util::header("orbit status");

    // Link
    util::info("host", &cfg.ssh_target());
    util::info("adapter", &cfg.adapter.to_string());
    util::info("endpoint", &cfg.docker_endpoint());
    util::info("remote sock", &cfg.remote_socket);

    // Docker context
    let ctx = docker_ctx::current_context()
        .await
        .unwrap_or_else(|_| "?".into());
    let ctx_line = if ctx == cfg.context_name {
        format!("{ctx} {}", "(active — docker → host)".green())
    } else {
        format!("{ctx} {}", "(local engine)".dimmed())
    };
    util::info("docker ctx", &ctx_line);

    // Connection
    let master = ssh::master_alive(&cfg).await;
    util::info("ssh master", &state(master));
    util::info("forwarder", &state(daemon_alive()));

    if !master {
        println!();
        util::warn("not connected — run `orbit up`.");
        return Ok(());
    }

    // Remote resources + ports (through the forwarded socket).
    print_remote().await;
    Ok(())
}

fn state(up: bool) -> String {
    if up {
        format!("{}", "running".green())
    } else {
        format!("{}", "stopped".red())
    }
}

fn daemon_alive() -> bool {
    let Ok(path) = config::pid_file() else {
        return false;
    };
    let Ok(pid) = std::fs::read_to_string(&path) else {
        return false;
    };
    let pid = pid.trim();
    !pid.is_empty()
        && std::process::Command::new("kill")
            .args(["-0", pid])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
}

async fn print_remote() {
    let socket = match config::local_docker_socket() {
        Ok(s) => s,
        Err(_) => return,
    };

    if let Ok(docker) = forwarder::connect(&socket) {
        if let Ok(info) = docker.info().await {
            util::header("Remote engine");
            util::info("version", info.server_version.as_deref().unwrap_or("?"));
            util::info("CPUs", &info.ncpu.unwrap_or(0).to_string());
            util::info("memory", &fmt_gib(info.mem_total.unwrap_or(0)));
            util::info(
                "containers",
                &format!(
                    "{} running / {} total",
                    info.containers_running.unwrap_or(0),
                    info.containers.unwrap_or(0)
                ),
            );
            util::info("images", &info.images.unwrap_or(0).to_string());
        }
    }

    util::header("Forwarded ports");
    match forwarder::list_published(&socket).await {
        Ok(ports) if !ports.is_empty() => {
            for p in ports {
                println!("  localhost:{p} {} host:{p}", "→".dimmed());
            }
        }
        Ok(_) => println!(
            "  {}",
            "none — start a container with -p to see it here".dimmed()
        ),
        Err(e) => util::warn(&format!("could not read ports: {e}")),
    }
}

fn fmt_gib(bytes: i64) -> String {
    if bytes <= 0 {
        return "?".into();
    }
    format!("{:.1} GiB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}
