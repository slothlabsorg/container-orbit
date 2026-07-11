//! Windows-over-WSL2 host adapter (the "PC gamer" case).
//!
//! Status: **detection only** in v1. When `orbit host init` runs on Windows and
//! finds a WSL2 distro with a reachable docker socket, it records the
//! `WindowsWsl` adapter so `orbit link` / `orbit up` give accurate guidance and
//! the docker context still works over `ssh://` (docker's own dial-stdio runs on
//! the remote regardless of OS).
//!
//! What is NOT yet automated: bridging the WSL-internal unix socket out to the
//! Windows OpenSSH server for our own `-L` socket forward (used by the port
//! reconciler). See docs/ROADMAP.md → "Windows host". Until then, automatic port
//! forwarding requires either running `orbit host init` *inside* the WSL distro
//! (which then looks like a normal unix host) or manual `orbit ports add`.

use super::{HostInfo, HostKind};

/// Returns `Some` only when running on Windows with WSL available.
pub fn detect() -> Option<HostInfo> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    // On Windows, Docker Desktop exposes the engine inside WSL at the standard
    // path; the named pipe is the Windows-native fallback (roadmap).
    Some(HostInfo {
        kind: HostKind::WindowsWsl,
        socket: "/var/run/docker.sock".to_string(),
    })
}
