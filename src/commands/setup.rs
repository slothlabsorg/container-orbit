//! `orbit setup` — the zero-friction, guided path. Discovers a host, sets up the
//! SSH key (authorizing it if needed), links, brings orbit up, and proves it works
//! with an end-to-end self-test. Aims to get you delegating Docker in ~2 minutes.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::process::Stdio;

use crate::commands::{link, up};
use crate::config::{Config, DEFAULT_CONTEXT};
use crate::host::HostKind;
use crate::net_scan;
use crate::ssh;
use crate::util;

pub async fn run(
    host_arg: Option<String>,
    user_arg: Option<String>,
    port: u16,
    yes: bool,
    no_test: bool,
) -> Result<()> {
    util::header("orbit setup");
    println!("  Let's point your Docker at a beefier machine. This takes ~2 minutes.\n");

    // 1. Host address --------------------------------------------------------
    let host = match host_arg {
        Some(h) => h,
        None => pick_host(yes)?,
    };

    // 2. SSH user ------------------------------------------------------------
    let default_user = util::run("whoami", &[]).await.unwrap_or_else(|_| "root".into());
    let user = match user_arg {
        Some(u) => u,
        None if yes => default_user.clone(),
        None => inquire::Text::new("SSH username on the host:")
            .with_default(&default_user)
            .with_help_message("The macOS/Linux login on the beefy machine")
            .prompt()
            .context("cancelled")?,
    };

    let target = format!("{user}@{host}");
    let cfg = Config {
        host_user: user.clone(),
        host_addr: host.clone(),
        ssh_port: port,
        adapter: HostKind::Unix,
        remote_socket: "/var/run/docker.sock".into(),
        context_name: DEFAULT_CONTEXT.to_string(),
        previous_context: None,
    };

    // 3. SSH key + authorization --------------------------------------------
    util::step("Preparing the orbit SSH key…");
    let pubkey = ssh::ensure_key().await?;
    util::ok("key ready");

    util::step(&format!("Checking SSH access to {target}…"));
    if ssh::test_connection(&cfg).await.is_err() {
        authorize_key(&cfg, &pubkey, yes).await?;
    } else {
        util::ok("the orbit key is already authorized");
    }

    // 4. Link (detect socket + create context) ------------------------------
    println!();
    link::run(&target, port, None).await?;

    // 5. Up ------------------------------------------------------------------
    println!();
    up::run(false).await?;

    // 6. Self-test -----------------------------------------------------------
    if !no_test {
        println!();
        if let Err(e) = self_test().await {
            util::warn(&format!(
                "self-test could not complete ({e:#}). orbit is up regardless — try:\n\
                 docker run -d -p 8080:80 nginx && curl localhost:8080"
            ));
        }
    }

    util::header("You're set");
    println!(
        "  Docker now runs on {}. Use it normally:\n",
        target.bold()
    );
    println!("      {}  docker build / run / compose — all on the host", "›".dimmed());
    println!("      {}  published ports (-p) appear on your localhost", "›".dimmed());
    println!("      {}  {}  keep it running across logins", "›".dimmed(), "orbit service install".cyan());
    println!("      {}  {}  connect an AI assistant", "›".dimmed(), "orbit mcp".cyan());
    println!("      {}  {}  stop and restore local docker", "›".dimmed(), "orbit down".cyan());
    util::funding_note();
    Ok(())
}

/// Discover hosts on the LAN and let the user pick, or type one in.
fn pick_host(yes: bool) -> Result<String> {
    if yes {
        anyhow::bail!("--yes needs --host <addr> (nothing to pick non-interactively)");
    }
    util::step("Scanning your LAN for machines with SSH open…");
    let candidates = futures_lite_block(net_scan::scan(22));

    let manual = "✎ Enter an address manually".to_string();
    let rescan = "↻ Rescan".to_string();

    let mut options: Vec<String> = candidates.iter().map(|c| c.ip.to_string()).collect();
    if options.is_empty() {
        util::warn("no SSH hosts found automatically — enter the address manually.");
        return prompt_manual_host();
    }
    options.push(manual.clone());
    options.push(rescan.clone());

    let choice = inquire::Select::new("Which machine should run Docker?", options)
        .with_help_message("Pick the beefy machine on your network")
        .prompt()
        .context("cancelled")?;

    if choice == manual {
        prompt_manual_host()
    } else if choice == rescan {
        pick_host(false)
    } else {
        Ok(choice)
    }
}

fn prompt_manual_host() -> Result<String> {
    inquire::Text::new("Host address (IP or hostname):")
        .with_help_message("e.g. 192.168.1.42 or gamer.local")
        .prompt()
        .context("cancelled")
        .map(|s| s.trim().to_string())
}

/// Authorize the orbit public key on the host. Tries `ssh-copy-id` interactively
/// (works in a real terminal), then falls back to a copy-pasteable manual command.
async fn authorize_key(cfg: &Config, pubkey: &str, yes: bool) -> Result<()> {
    util::warn("the orbit key isn't authorized on the host yet — let's fix that.");

    if !yes {
        // Attempt ssh-copy-id with inherited stdio so the password prompt works.
        util::step("Installing the key (you may be asked for the host password)…");
        let pub_path = crate::config::ssh_key_path()?.with_extension("pub");
        let status = tokio::process::Command::new("ssh-copy-id")
            .args([
                "-i",
                &pub_path.to_string_lossy(),
                "-p",
                &cfg.ssh_port.to_string(),
                &cfg.ssh_target(),
            ])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .ok();

        if status.map(|s| s.success()).unwrap_or(false) && ssh::test_connection(cfg).await.is_ok() {
            util::ok("key installed — SSH works");
            return Ok(());
        }
    }

    // Fallback: the host may have password auth disabled. Guide the manual step.
    println!();
    util::warn("couldn't install the key automatically (password login may be disabled).");
    println!("  Run this {} the host, or paste the key into its ~/.ssh/authorized_keys:\n", "on".italic());
    println!(
        "      {}\n",
        format!(
            "mkdir -p ~/.ssh && chmod 700 ~/.ssh && echo '{}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys",
            pubkey.trim()
        )
        .cyan()
    );

    if yes {
        anyhow::bail!("key not authorized and --yes was set; authorize it and re-run");
    }

    loop {
        let _ = inquire::Text::new("Press Enter once the key is added (or type 'skip' to abort):")
            .with_default("")
            .prompt();
        if ssh::test_connection(cfg).await.is_ok() {
            util::ok("SSH works now");
            return Ok(());
        }
        util::warn("still can't connect — check the address/user and that the line was added exactly.");
    }
}

/// Prove transparency: run a tiny container on the host and curl it via localhost.
async fn self_test() -> Result<()> {
    util::step("Running an end-to-end self-test (nginx on the host → curl localhost)…");
    let port = free_local_port().unwrap_or(8080);
    let name = "orbit-selftest";
    docker_quiet(&["rm", "-f", name]).await;

    // Pull first (shows progress) so the run below is instant.
    println!("  pulling a small test image (nginx:alpine)…");
    docker(&["pull", "nginx:alpine"]).await?;

    docker(&[
        "run", "-d", "--rm", "-p", &format!("{port}:80"), "--name", name, "nginx:alpine",
    ])
    .await
    .context("could not start the test container")?;

    // Give the reconciler a moment to open the forward.
    let mut ok = false;
    for _ in 0..15 {
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        if curl_ok(port).await {
            ok = true;
            break;
        }
    }
    docker_quiet(&["rm", "-f", name]).await;

    if ok {
        util::ok(&format!(
            "self-test passed — a container on the host answered on localhost:{port}"
        ));
        Ok(())
    } else {
        anyhow::bail!("the test container didn't answer on localhost:{port}")
    }
}

async fn docker(args: &[&str]) -> Result<()> {
    let status = tokio::process::Command::new("docker")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .context("failed to run docker — is it installed?")?;
    if !status.success() {
        anyhow::bail!("`docker {}` failed", args.join(" "));
    }
    Ok(())
}

/// Fire-and-forget docker call with all output suppressed (for cleanup).
async fn docker_quiet(args: &[&str]) {
    let _ = tokio::process::Command::new("docker")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

async fn curl_ok(port: u16) -> bool {
    tokio::process::Command::new("curl")
        .args(["-s", "-o", "/dev/null", "--max-time", "5", &format!("http://localhost:{port}")])
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

fn free_local_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
}

/// Run a future to completion on the current runtime from a sync context.
fn futures_lite_block<F: std::future::Future>(fut: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(fut))
}
