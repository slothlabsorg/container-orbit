//! `orbit host setup` — friendly host-side preparation, and `orbit host add-key`
//! to authorize a client's public key without needing password SSH.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::io::Write;

use crate::commands::host_init;
use crate::util;

pub async fn run() -> Result<()> {
    // Reuse the detection + join-string output.
    host_init::run().await?;

    util::header("Authorizing a client");
    println!("  Easiest: on your laptop run {} — it installs the key for you.", "orbit setup".cyan());
    println!(
        "  If password SSH is disabled here, have the client share its key and run:\n\n      {}\n",
        "orbit host add-key \"ssh-ed25519 AAAA… orbit\"".cyan()
    );
    Ok(())
}

/// Append a client's public key to this machine's ~/.ssh/authorized_keys.
pub async fn add_key(pubkey: &str) -> Result<()> {
    util::header("orbit host add-key");

    let key = pubkey.trim();
    if !looks_like_key(key) {
        anyhow::bail!(
            "that doesn't look like an SSH public key.\n\
             Expected something like: ssh-ed25519 AAAA... comment"
        );
    }

    let home = dirs::home_dir().context("cannot determine home directory")?;
    let ssh_dir = home.join(".ssh");
    std::fs::create_dir_all(&ssh_dir).with_context(|| format!("creating {}", ssh_dir.display()))?;
    set_mode(&ssh_dir, 0o700);

    let auth = ssh_dir.join("authorized_keys");
    let existing = std::fs::read_to_string(&auth).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == key) {
        util::ok("key is already authorized — nothing to do");
        return Ok(());
    }

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&auth)
        .with_context(|| format!("opening {}", auth.display()))?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(f)?;
    }
    writeln!(f, "{key}")?;
    set_mode(&auth, 0o600);

    util::ok(&format!("authorized the key in {}", auth.display()));
    util::info("clients can now", "orbit link / orbit setup without a password");
    Ok(())
}

fn looks_like_key(s: &str) -> bool {
    let mut parts = s.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(kind), Some(body)) => {
            (kind.starts_with("ssh-") || kind.starts_with("ecdsa-") || kind.starts_with("sk-"))
                && body.len() > 20
        }
        _ => false,
    }
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_mode(_path: &std::path::Path, _mode: u32) {}
