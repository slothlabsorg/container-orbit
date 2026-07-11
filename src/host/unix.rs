//! Unix-socket host adapter (macOS / Linux). Covers Docker Desktop, OrbStack,
//! Rancher Desktop, colima — anything exposing a unix domain docker socket.

use super::{HostInfo, HostKind};
use anyhow::{bail, Result};

/// Candidate socket paths, in priority order. The first that exists wins.
/// `/var/run/docker.sock` is the de-facto standard and is usually symlinked to
/// whatever engine is active (Docker Desktop, OrbStack, colima, …).
const CANDIDATES: &[&str] = &["/var/run/docker.sock", "/run/docker.sock"];

pub fn detect() -> Result<HostInfo> {
    // Standard locations first.
    for path in CANDIDATES {
        if std::path::Path::new(path).exists() {
            return Ok(HostInfo {
                kind: HostKind::Unix,
                socket: (*path).to_string(),
            });
        }
    }

    // Per-user engine sockets (OrbStack / Docker Desktop) when /var/run isn't symlinked.
    if let Some(home) = dirs::home_dir() {
        for rel in [
            ".docker/run/docker.sock",
            ".orbstack/run/docker.sock",
            ".colima/default/docker.sock",
        ] {
            let p = home.join(rel);
            if p.exists() {
                return Ok(HostInfo {
                    kind: HostKind::Unix,
                    socket: p.to_string_lossy().into_owned(),
                });
            }
        }
    }

    bail!(
        "no docker socket found on this host. Is Docker/OrbStack/Rancher running?\n\
         Looked at {} and ~/.docker, ~/.orbstack, ~/.colima.",
        CANDIDATES.join(", ")
    )
}
