//! Machine + connection metrics for the status view and the `orbit up`
//! foreground dashboard: local specs, remote engine info, and how much RAM /
//! CPU load is being carried by the host instead of your laptop.

use std::path::Path;

use bollard::container::{ListContainersOptions, StatsOptions};
use futures_util::StreamExt;
use owo_colors::OwoColorize;

use crate::config::Config;
use crate::forwarder;
use crate::ssh;
use crate::util;

pub struct LocalSpecs {
    pub hostname: String,
    pub os_arch: String,
    pub cores: usize,
    pub mem_bytes: u64,
}

pub fn local_specs() -> LocalSpecs {
    let hostname = std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "this-machine".into());
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    LocalSpecs {
        hostname,
        os_arch: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        cores,
        mem_bytes: local_mem_bytes(),
    }
}

fn local_mem_bytes() -> u64 {
    // macOS
    if let Ok(o) = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
        if let Ok(v) = String::from_utf8_lossy(&o.stdout).trim().parse::<u64>() {
            if v > 0 {
                return v;
            }
        }
    }
    // Linux: /proc/meminfo MemTotal (kB)
    if let Ok(txt) = std::fs::read_to_string("/proc/meminfo") {
        for line in txt.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                if let Some(kb) = rest.split_whitespace().next().and_then(|s| s.parse::<u64>().ok()) {
                    return kb * 1024;
                }
            }
        }
    }
    0
}

pub struct RemoteMetrics {
    pub version: String,
    pub ncpu: i64,
    pub mem_total: i64,
    pub images: i64,
    pub containers: i64,
    pub running: i64,
    /// Sum of memory currently used by running containers on the host (bytes).
    pub offloaded_mem: u64,
}

/// Query the remote engine through the forwarded socket. Best-effort.
pub async fn remote_metrics(socket: &Path) -> Option<RemoteMetrics> {
    let docker = forwarder::connect(socket).ok()?;
    let info = docker.info().await.ok()?;

    // Sum memory used by running containers (one-shot stats, capped for speed).
    let mut offloaded_mem = 0u64;
    if let Ok(list) = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
    {
        for c in list.into_iter().take(40) {
            let Some(id) = c.id else { continue };
            let mut stream = docker.stats(
                &id,
                Some(StatsOptions {
                    stream: false,
                    one_shot: true,
                }),
            );
            if let Some(Ok(s)) = stream.next().await {
                offloaded_mem += s.memory_stats.usage.unwrap_or(0);
            }
        }
    }

    Some(RemoteMetrics {
        version: info.server_version.unwrap_or_else(|| "?".into()),
        ncpu: info.ncpu.unwrap_or(0),
        mem_total: info.mem_total.unwrap_or(0),
        images: info.images.unwrap_or(0),
        containers: info.containers.unwrap_or(0),
        running: info.containers_running.unwrap_or(0),
        offloaded_mem,
    })
}

/// The host's 1-minute load average, via `ssh <host> uptime`. Best-effort.
pub async fn remote_load(cfg: &Config) -> Option<String> {
    let out = ssh::remote_exec(cfg, "uptime").await.ok()?;
    // "... load average: 0.52, 0.48, 0.44"  /  "... load averages: 1.20 1.10 0.95"
    let after = out.split("average").nth(1)?; // handles average/averages
    let after = after.trim_start_matches('s').trim_start_matches(':').trim();
    let first = after
        .split(|c: char| c == ',' || c.is_whitespace())
        .find(|t| !t.is_empty())?;
    Some(first.to_string())
}

pub fn fmt_gib(bytes: i64) -> String {
    if bytes <= 0 {
        return "?".into();
    }
    format!("{:.1} GiB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

pub fn fmt_gib_u(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

/// Print a compact CLI dashboard header: this machine, the host, routing, and
/// what's being offloaded. Used at the top of `orbit up --foreground`.
pub async fn print_dashboard(cfg: &Config) {
    let local = local_specs();
    let socket = crate::config::local_docker_socket().ok();
    let remote = match &socket {
        Some(s) => remote_metrics(s).await,
        None => None,
    };
    let load = remote_load(cfg).await;

    let bar = "─".repeat(64);
    println!("{}", bar.cyan());
    println!(
        "  {}   {} · {} · {} cores · {}",
        "this machine".bold(),
        local.hostname.white(),
        local.os_arch,
        local.cores,
        fmt_gib_u(local.mem_bytes),
    );

    if let Some(r) = &remote {
        println!(
            "  {}        {} · docker {} · {} images · {} running",
            "the host".bold(),
            cfg.ssh_target().white(),
            r.version,
            r.images,
            r.running,
        );
        println!(
            "  {}        docker → {} context · socket {}",
            "routing".bold(),
            cfg.context_name,
            cfg.remote_socket,
        );
        let load_str = load.as_deref().unwrap_or("?");
        println!(
            "  {}     {} · {} cores · load {} · your laptop stays free {}",
            "offloading".bold().magenta(),
            format!("{} containers", r.running).magenta(),
            r.ncpu,
            load_str,
            "♥".magenta(),
        );
        let _ = &r.mem_total;
        let _ = r.containers;
        if r.offloaded_mem > 0 {
            println!(
                "  {}     {} of {} used by containers on the host",
                " ".repeat(9),
                fmt_gib_u(r.offloaded_mem).magenta(),
                fmt_gib(r.mem_total),
            );
        }
    } else {
        println!(
            "  {}        {} · (bring it up to read remote metrics)",
            "the host".bold(),
            cfg.ssh_target().white(),
        );
    }
    println!("{}", bar.cyan());
    let _ = util::FUNDING_URL;
}
