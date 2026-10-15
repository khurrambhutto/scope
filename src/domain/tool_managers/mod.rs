//! User-owned tool environments and tracked Cargo installations.
//! Inventory, operations, and inspection share the same installation identity.

mod cargo;
mod javascript;
mod javascript_commands;
mod javascript_config;
mod javascript_safety;
mod javascript_updates;
pub mod operations;
mod pipx;
mod python_updates;
mod safety;
pub use safety::validate;
mod updates;
mod uv;

use crate::domain::{
    package::{AppKind, InstalledPackage, PackageSource},
    system,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

/// Captured context and receipt identity. All fields come from local inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInstallation {
    pub source: PackageSource,
    pub manager: PathBuf,
    pub root: PathBuf,
    pub environment: Option<PathBuf>,
    pub executables: Vec<PathBuf>,
    pub env: BTreeMap<String, String>,
    pub receipt_hash: String,
    #[serde(default)]
    pub protection_reason: Option<String>,
    pub update: UpdateRecipe,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateRecipe {
    Uv {
        requirements: Vec<String>,
        constraints: Vec<String>,
        overrides: Vec<String>,
        resolver_config: Option<String>,
    },
    Pipx {
        package: String,
        pip_args: Vec<String>,
        pinned: bool,
        index_source: bool,
    },
    Cargo {
        origin: String,
        options: Vec<String>,
        version_req: Option<String>,
    },
    Javascript {
        package_name: String,
        bin: PathBuf,
        node: Option<PathBuf>,
        requirement: String,
        registry_source: bool,
    },
    Unsupported(String),
}

pub fn is_tool_source(source: PackageSource) -> bool {
    matches!(
        source,
        PackageSource::Uv
            | PackageSource::Pipx
            | PackageSource::Cargo
            | PackageSource::Npm
            | PackageSource::Bun
    )
}

pub fn manager_name(source: PackageSource) -> &'static str {
    match source {
        PackageSource::Uv => "uv",
        PackageSource::Pipx => "pipx",
        PackageSource::Cargo => "cargo",
        PackageSource::Npm => "npm",
        PackageSource::Bun => "bun",
        _ => "",
    }
}

pub async fn inventory(source: PackageSource) -> Result<Vec<InstalledPackage>> {
    let manager = system::resolve(manager_name(source))
        .ok_or_else(|| anyhow::anyhow!("Tool manager is unavailable"))?;
    match source {
        PackageSource::Uv => uv::inventory(manager, None).await,
        PackageSource::Pipx => pipx::inventory(manager, None).await,
        PackageSource::Cargo => cargo::inventory(manager, None).await,
        PackageSource::Npm | PackageSource::Bun => {
            javascript::inventory(source, manager, None).await
        }
        _ => bail!("Not a tool manager"),
    }
}

pub async fn inventory_recorded(installation: &ToolInstallation) -> Result<Vec<InstalledPackage>> {
    match installation.source {
        PackageSource::Uv => uv::inventory(installation.manager.clone(), Some(installation)).await,
        PackageSource::Pipx => {
            pipx::inventory(installation.manager.clone(), Some(installation)).await
        }
        PackageSource::Cargo => {
            cargo::inventory(installation.manager.clone(), Some(installation)).await
        }
        PackageSource::Npm | PackageSource::Bun => {
            javascript::inventory(
                installation.source,
                installation.manager.clone(),
                Some(installation),
            )
            .await
        }
        _ => bail!("Not a tool manager"),
    }
}

pub async fn check_updates(packages: &mut [InstalledPackage]) -> Option<String> {
    updates::check(packages).await
}

pub(super) fn package(
    id: &str,
    version: &str,
    mut installation: ToolInstallation,
) -> Result<InstalledPackage> {
    validate_package_name(installation.source, id)?;
    let mut pkg = InstalledPackage::new(installation.source, id);
    pkg.key = format!(
        "{}:{}:{}",
        installation.source.id(),
        fingerprint(installation.root.as_os_str().as_encoded_bytes()),
        id
    );
    pkg.name = id.into();
    pkg.version = version.into();
    pkg.app_kind = if installation.executables.is_empty() {
        AppKind::Unknown
    } else {
        AppKind::Cli
    };
    pkg.terminal = pkg.app_kind == AppKind::Cli;
    installation.protection_reason = validate(&installation).err().map(|error| error.to_string());
    pkg.tool_installation = Some(Box::new(installation));
    Ok(pkg)
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 256
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        bail!("Invalid tool or executable name");
    }
    Ok(())
}

pub(super) fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| anyhow::anyhow!("User home is unavailable"))
}

/// Read only bounded metadata, never an unbounded environment or executable.
pub(super) async fn read(path: &Path) -> Result<String> {
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(path)
        .await
        .context("Tool receipt could not be read")?;
    let mut bytes = Vec::new();
    file.take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > 2 * 1024 * 1024 {
        bail!("Tool receipt exceeds the metadata limit");
    }
    String::from_utf8(bytes).context("Tool receipt is not UTF-8")
}

pub(super) fn fingerprint(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) async fn query(manager: &Path, args: &[&str]) -> Result<String> {
    let output = system::capture_output_context(
        manager.to_str().context("Manager path is not UTF-8")?,
        args,
        &[],
        Some(&home()?),
        Duration::from_secs(30),
    )
    .await?;
    if !output.success {
        bail!(
            "{} query failed: {}",
            manager.file_name().unwrap_or_default().to_string_lossy(),
            output.stderr.trim()
        );
    }
    Ok(output.stdout)
}

pub(super) async fn query_installation(
    installation: &ToolInstallation,
    program: &Path,
    args: &[&str],
) -> Result<String> {
    let envs = installation
        .env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect::<Vec<_>>();
    let output = system::capture_output_context(
        program.to_str().context("Executable path is not UTF-8")?,
        args,
        &envs,
        Some(&working_directory(installation)?),
        Duration::from_secs(30),
    )
    .await?;
    if !output.success {
        bail!("Tool metadata query failed");
    }
    Ok(output.stdout)
}

pub(super) fn working_directory(installation: &ToolInstallation) -> Result<PathBuf> {
    if installation.source == PackageSource::Bun {
        Ok(installation.root.clone())
    } else {
        home()
    }
}

#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod tests;

/// JavaScript scopes are package identities, never executable path components.
pub fn validate_package_name(source: PackageSource, name: &str) -> Result<()> {
    if matches!(source, PackageSource::Npm | PackageSource::Bun) {
        if let Some(scoped) = name.strip_prefix('@') {
            let (scope, package) = scoped
                .split_once('/')
                .context("Invalid scoped package name")?;
            validate_name(scope)?;
            return validate_name(package);
        }
    }
    validate_name(name)
}
