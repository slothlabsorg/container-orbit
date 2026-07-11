//! Host adapters — abstract *how* the remote Docker socket is located and exposed.
//!
//! v1 ships the unix adapter (macOS / Linux), which covers the Mac→Mac focus.
//! Windows-over-WSL2 is detected and recorded; full socket bridging is on the
//! roadmap (see docs/ROADMAP.md).

pub mod unix;
pub mod windows_wsl;

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    /// macOS or Linux exposing a unix domain socket.
    Unix,
    /// Windows with Docker reachable inside a WSL2 distro.
    WindowsWsl,
}

impl fmt::Display for HostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostKind::Unix => write!(f, "unix (macOS/Linux)"),
            HostKind::WindowsWsl => write!(f, "windows-wsl2"),
        }
    }
}

/// What `orbit host init` learns about the local machine when it plays host.
#[derive(Debug, Clone)]
pub struct HostInfo {
    pub kind: HostKind,
    /// Absolute path of the docker socket on this machine.
    pub socket: String,
}

/// Detect, on the machine running `orbit host init`, which adapter applies and
/// where its docker socket lives.
pub fn detect_local() -> anyhow::Result<HostInfo> {
    if let Some(info) = windows_wsl::detect() {
        return Ok(info);
    }
    unix::detect()
}

impl HostKind {
    /// Whether forwarding the remote socket as a unix→unix `-L` tunnel is
    /// supported for this adapter in the current build.
    pub fn supports_socket_forward(&self) -> bool {
        matches!(self, HostKind::Unix)
    }
}
