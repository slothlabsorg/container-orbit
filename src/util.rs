//! Small shared helpers: process spawning and pretty terminal output.

use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use std::process::Output;
use tokio::process::Command;

/// Run a command to completion, returning trimmed stdout. Errors include stderr.
pub async fn run(program: &str, args: &[&str]) -> Result<String> {
    let out = capture(program, args).await?;
    if !out.status.success() {
        bail!(
            "`{} {}` failed (exit {}):\n{}",
            program,
            args.join(" "),
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Run a command and return the raw `Output` (caller inspects status/streams).
pub async fn capture(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("failed to spawn `{program}` — is it installed and on PATH?"))
}

/// True if the command exits 0. Swallows spawn errors as `false`.
pub async fn succeeds(program: &str, args: &[&str]) -> bool {
    match capture(program, args).await {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

// ---- terminal output -------------------------------------------------------

pub fn step(msg: &str) {
    println!("{} {}", "▸".cyan().bold(), msg);
}

pub fn ok(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

pub fn warn(msg: &str) {
    println!("{} {}", "!".yellow().bold(), msg.yellow());
}

pub fn info(label: &str, value: &str) {
    println!("  {:<14} {}", format!("{label}:").dimmed(), value);
}

pub fn header(msg: &str) {
    println!("\n{}", msg.bold().underline());
}

/// The SlothLabs funding page.
pub const FUNDING_URL: &str = "https://slothlabs.org/pricing";

/// A gentle one-line reminder shown when orbit connects/disconnects, that this
/// is built with love and free — with a pointer to the funding page.
pub fn funding_note() {
    println!(
        "\n{} built with {} by SlothLabs — free & open source. Support: {}",
        "♥".magenta(),
        "love".magenta(),
        FUNDING_URL.cyan().underline()
    );
}
