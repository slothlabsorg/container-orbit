//! `orbit link <user@host>` — run on the client (laptop).
//!
//! Generates the orbit SSH key, installs it on the host if needed, detects the
//! remote docker socket and adapter, then creates the `orbit` docker context.

use anyhow::{Context, Result};

use crate::config::{self, Config, DEFAULT_CONTEXT};
use crate::docker_ctx;
use crate::host::HostKind;
use crate::ssh;
use crate::util;

pub async fn run(target: &str, port: u16, socket_override: Option<String>) -> Result<()> {
    util::header("orbit link");

    let (user, host) = parse_target(target).await?;

    // Provisional config so ssh helpers know where to connect.
    let mut cfg = Config {
        host_user: user.clone(),
        host_addr: host.clone(),
        ssh_port: port,
        adapter: HostKind::Unix,
        remote_socket: socket_override
            .clone()
            .unwrap_or_else(|| "/var/run/docker.sock".into()),
        context_name: DEFAULT_CONTEXT.to_string(),
        previous_context: None,
    };

    // 1. SSH key.
    util::step("Ensuring the orbit SSH key…");
    let pubkey = ssh::ensure_key().await?;
    util::ok("key ready");

    // 2. Connectivity — install the key if the probe fails.
    util::step(&format!("Connecting to {}…", cfg.ssh_target()));
    if ssh::test_connection(&cfg).await.is_err() {
        util::warn(
            "orbit key not authorized yet — installing it (you'll be asked for the host password).",
        );
        install_key(&cfg).await?;
        ssh::test_connection(&cfg)
            .await
            .context("still cannot connect after installing the key")?;
    }
    util::ok("SSH connection works");

    // 3. Detect remote adapter + socket.
    let os = ssh::remote_exec(&cfg, "uname -s 2>/dev/null || echo unknown")
        .await
        .unwrap_or_default();
    cfg.adapter = if os.contains("Darwin") || os.contains("Linux") {
        HostKind::Unix
    } else {
        HostKind::WindowsWsl
    };
    if socket_override.is_none() {
        if let Some(found) = detect_remote_socket(&cfg).await {
            cfg.remote_socket = found;
        }
    }
    util::ok(&format!(
        "host is {} — adapter {}, socket {}",
        os.trim(),
        cfg.adapter,
        cfg.remote_socket
    ));

    // 4. Confirm the remote daemon socket is actually there.
    let has_socket = ssh::remote_exec(&cfg, &format!("test -S {} && echo yes", cfg.remote_socket))
        .await
        .map(|o| o.trim() == "yes")
        .unwrap_or(false);
    if has_socket {
        util::ok("remote docker socket is present");
    } else {
        util::warn(&format!(
            "could not confirm a docker socket at {} — is Docker running on the host? \
             Continuing; `orbit up` will tell you for sure.",
            cfg.remote_socket
        ));
    }

    // 5. Create the docker context pointing at the (to-be) forwarded local socket.
    util::step(&format!("Creating docker context `{}`…", cfg.context_name));
    docker_ctx::create_or_update(
        &cfg.context_name,
        &cfg.docker_endpoint(),
        &format!("orbit → {}", cfg.ssh_target()),
    )
    .await?;
    util::ok("context created (activated on `orbit up`)");

    cfg.save()?;
    util::ok(&format!(
        "saved config to {}",
        config::config_path()?.display()
    ));

    util::header("Next");
    println!(
        "  Run {} to delegate Docker and forward ports.",
        "orbit up".bold_green()
    );
    let _ = pubkey; // already installed above
    Ok(())
}

/// Split `user@host` / `host`. Without a user, use the current local username.
async fn parse_target(target: &str) -> Result<(String, String)> {
    if let Some((u, h)) = target.split_once('@') {
        Ok((u.to_string(), h.to_string()))
    } else {
        let user = util::run("whoami", &[])
            .await
            .unwrap_or_else(|_| "root".into());
        Ok((user, target.to_string()))
    }
}

async fn install_key(cfg: &Config) -> Result<()> {
    let pub_path = config::ssh_key_path()?.with_extension("pub");
    util::run(
        "ssh-copy-id",
        &[
            "-i",
            &pub_path.to_string_lossy(),
            "-p",
            &cfg.ssh_port.to_string(),
            &cfg.ssh_target(),
        ],
    )
    .await
    .context("ssh-copy-id failed")?;
    Ok(())
}

/// Probe the host for the first existing docker socket among the usual paths.
async fn detect_remote_socket(cfg: &Config) -> Option<String> {
    let probe = r#"for s in /var/run/docker.sock /run/docker.sock "$HOME/.docker/run/docker.sock" "$HOME/.orbstack/run/docker.sock" "$HOME/.colima/default/docker.sock"; do [ -S "$s" ] && echo "$s" && break; done"#;
    match ssh::remote_exec(cfg, probe).await {
        Ok(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

trait BoldGreen {
    fn bold_green(&self) -> String;
}
impl BoldGreen for str {
    fn bold_green(&self) -> String {
        use owo_colors::OwoColorize;
        format!("{}", self.bold().green())
    }
}
