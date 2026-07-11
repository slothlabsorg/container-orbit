//! Thin wrapper over `docker context`. This is the compatibility layer: by
//! managing a standard context we interoperate with Docker Desktop, Rancher
//! Desktop, OrbStack and colima without wrapping the `docker` binary itself.

use anyhow::{Context, Result};

use crate::util;

/// Name of the context currently in use (e.g. `desktop-linux`, `orbit`).
pub async fn current_context() -> Result<String> {
    util::run("docker", &["context", "show"])
        .await
        .context("could not read current docker context — is docker installed?")
}

pub async fn context_exists(name: &str) -> bool {
    util::succeeds("docker", &["context", "inspect", name]).await
}

/// Create the orbit context, or update its endpoint if it already exists.
pub async fn create_or_update(name: &str, endpoint: &str, description: &str) -> Result<()> {
    let host_arg = format!("host={endpoint}");
    if context_exists(name).await {
        util::run(
            "docker",
            &["context", "update", name, "--docker", &host_arg],
        )
        .await?;
    } else {
        util::run(
            "docker",
            &[
                "context",
                "create",
                name,
                "--description",
                description,
                "--docker",
                &host_arg,
            ],
        )
        .await?;
    }
    Ok(())
}

pub async fn use_context(name: &str) -> Result<()> {
    tracing::debug!(context = %name, "docker context use");
    util::run("docker", &["context", "use", name])
        .await
        .with_context(|| format!("could not switch to docker context `{name}`"))?;
    Ok(())
}
