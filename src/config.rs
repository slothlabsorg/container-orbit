//! Persistent config at `~/.config/orbit/config.toml` plus derived runtime paths.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::host::HostKind;

pub const DEFAULT_CONTEXT: &str = "orbit";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// SSH user on the host (e.g. `dany`).
    pub host_user: String,
    /// Host address — IP or resolvable name on the LAN.
    pub host_addr: String,
    /// SSH port on the host.
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    /// Which host adapter exposes the remote docker socket.
    pub adapter: HostKind,
    /// Absolute path to the docker socket on the host.
    pub remote_socket: String,
    /// Name of the docker context orbit manages.
    #[serde(default = "default_context")]
    pub context_name: String,
    /// Docker context that was active before `orbit up`, restored on `orbit down`.
    #[serde(default)]
    pub previous_context: Option<String>,
}

fn default_ssh_port() -> u16 {
    22
}
fn default_context() -> String {
    DEFAULT_CONTEXT.to_string()
}

impl Config {
    /// `user@host` target string used by ssh/docker.
    pub fn ssh_target(&self) -> String {
        format!("{}@{}", self.host_user, self.host_addr)
    }

    /// Docker context endpoint: the locally-forwarded unix socket.
    ///
    /// We deliberately do NOT use `ssh://user@host` — that makes docker run
    /// `docker system dial-stdio` on the remote, which needs the `docker` binary
    /// on the remote's non-interactive SSH PATH (a common breakage). Instead we
    /// forward the remote daemon socket and point docker straight at it.
    pub fn docker_endpoint(&self) -> String {
        match local_docker_socket() {
            Ok(p) => format!("unix://{}", p.display()),
            Err(_) => "unix://<unavailable>".to_string(),
        }
    }

    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let raw = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "no orbit config at {} — run `orbit link <user@host>` first",
                path.display()
            )
        })?;
        toml::from_str(&raw).context("config.toml is malformed")
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(&path, raw).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

// ---- paths -----------------------------------------------------------------

/// `~/.orbit` — everything lives here. We avoid the platform config dir on
/// purpose: macOS's `~/Library/Application Support` contains a space, which is
/// hostile to unix socket paths (`unix://…` endpoints, `ssh -L` specs).
pub fn config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".orbit"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// `~/.config/orbit/run` — sockets, pidfiles, logs. Created on demand.
pub fn run_dir() -> Result<PathBuf> {
    let dir = config_dir()?.join("run");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// SSH ControlMaster control socket.
pub fn control_socket() -> Result<PathBuf> {
    Ok(run_dir()?.join("control.sock"))
}

/// Local unix socket where the remote docker socket is forwarded.
pub fn local_docker_socket() -> Result<PathBuf> {
    Ok(run_dir()?.join("docker.sock"))
}

/// PID of the running `orbit up` daemon.
pub fn pid_file() -> Result<PathBuf> {
    Ok(run_dir()?.join("orbit.pid"))
}

/// Path to the orbit-managed SSH key pair.
pub fn ssh_key_path() -> Result<PathBuf> {
    let dir = config_dir()?.join("keys");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("id_orbit_ed25519"))
}

/// Fail early with a friendly message if config is missing.
pub fn require_linked() -> Result<Config> {
    match Config::load() {
        Ok(c) => Ok(c),
        Err(_) => bail!("not linked to a host yet — run `orbit link <user@host>` first"),
    }
}
