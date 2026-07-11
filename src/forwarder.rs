//! The port reconciler — the piece that makes remote Docker feel local.
//!
//! It talks to the remote daemon through the forwarded unix socket (set up by
//! `ssh::start_master`), watches container events, and keeps the set of SSH TCP
//! forwards in sync with the set of published container ports. Net effect:
//! `docker run -p 8080:80` on the host ⇒ `curl localhost:8080` works on the laptop.

use anyhow::{Context, Result};
use bollard::container::ListContainersOptions;
use bollard::system::EventsOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashSet;
use std::path::Path;

use crate::config::Config;
use crate::ssh;

/// Connect bollard to the locally-forwarded docker socket.
pub fn connect(socket: &Path) -> Result<Docker> {
    Docker::connect_with_unix(&socket.to_string_lossy(), 120, bollard::API_DEFAULT_VERSION)
        .with_context(|| {
            format!(
                "connecting to forwarded docker socket at {}",
                socket.display()
            )
        })
}

/// The set of published host ports across all running containers.
async fn desired_ports(docker: &Docker) -> Result<HashSet<u16>> {
    let opts = ListContainersOptions::<String> {
        all: false,
        ..Default::default()
    };
    let containers = docker.list_containers(Some(opts)).await?;
    let mut ports = HashSet::new();
    for c in containers {
        for p in c.ports.unwrap_or_default() {
            if let Some(public) = p.public_port {
                ports.insert(public);
            }
        }
    }
    Ok(ports)
}

/// Bring active SSH forwards in line with `desired`, mutating `current`.
async fn reconcile(cfg: &Config, current: &mut HashSet<u16>, desired: HashSet<u16>) {
    for &port in desired
        .difference(current)
        .cloned()
        .collect::<Vec<_>>()
        .iter()
    {
        match ssh::add_forward(cfg, port).await {
            Ok(()) => {
                current.insert(port);
                println!("  + forwarding localhost:{port} → host:{port}");
            }
            Err(e) => eprintln!("  ! could not forward {port}: {e}"),
        }
    }
    for &port in current
        .difference(&desired)
        .cloned()
        .collect::<Vec<_>>()
        .iter()
    {
        let _ = ssh::cancel_forward(cfg, port).await;
        current.remove(&port);
        println!("  - stopped forwarding {port}");
    }
}

/// Run the reconciler until the process is asked to stop. Performs an initial
/// reconcile, then reacts to every container event.
pub async fn run(cfg: &Config, socket: &Path) -> Result<()> {
    let docker = connect(socket)?;
    docker
        .ping()
        .await
        .context("forwarded docker socket is not responding")?;

    let mut current: HashSet<u16> = HashSet::new();
    reconcile(
        cfg,
        &mut current,
        desired_ports(&docker).await.unwrap_or_default(),
    )
    .await;

    let mut events = docker.events(Some(EventsOptions::<String> {
        ..Default::default()
    }));

    while let Some(event) = events.next().await {
        match event {
            Ok(_) => {
                // Cheap to recompute; the container list is the source of truth.
                let desired = desired_ports(&docker)
                    .await
                    .unwrap_or_else(|_| current.clone());
                reconcile(cfg, &mut current, desired).await;
            }
            Err(e) => {
                eprintln!("! docker event stream error: {e}");
                break;
            }
        }
    }

    // Stream ended (daemon gone / connection dropped): drop our forwards.
    for &port in current.clone().iter() {
        let _ = ssh::cancel_forward(cfg, port).await;
    }
    Ok(())
}

/// Snapshot of the currently-published ports — used by `orbit status`/`ports`.
pub async fn list_published(socket: &Path) -> Result<Vec<u16>> {
    let docker = connect(socket)?;
    let mut v: Vec<u16> = desired_ports(&docker).await?.into_iter().collect();
    v.sort_unstable();
    Ok(v)
}
