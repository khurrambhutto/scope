//! Preview and install local Debian archives through APT, without executing
//! archive content during inspection. Repository configuration stays with APT.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    apt_transaction, probe, AuthMethod, Operation, OperationPlan, OperationResult, OperationTarget,
    PlanStep,
};
use crate::domain::{package::PackageSource, safety, system};

const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebInstallPlan {
    pub source: PathBuf,
    pub sha256: String,
    pub package_id: String,
    pub version: String,
    pub architecture: String,
    pub installed_version: Option<String>,
}

/// A private snapshot prevents changes to a file in Downloads while APT reads
/// it. The same bytes supply metadata, simulation, and installation.
struct Archive {
    _directory: tempfile::TempDir,
    path: PathBuf,
    sha256: String,
}

impl Archive {
    fn copy(source: &Path) -> Result<Self> {
        if !source.is_absolute() || !std::fs::symlink_metadata(source)?.file_type().is_file() {
            bail!("Open a regular .deb file, not a symlink or directory.");
        }
        let input = std::fs::File::open(source)?;
        if input.metadata()?.len() > MAX_ARCHIVE_BYTES {
            bail!("The Debian archive exceeds the 8 GiB inspection limit.");
        }
        let directory = tempfile::Builder::new().prefix("scope-deb-").tempdir()?;
        let path = directory.path().join("package.deb");
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut hash = Sha256::new();
        let mut input = input.take(MAX_ARCHIVE_BYTES + 1);
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            if bytes > MAX_ARCHIVE_BYTES {
                bail!("The Debian archive exceeds the inspection limit.");
            }
            hash.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        output.sync_all()?;
        Ok(Self {
            _directory: directory,
            path,
            sha256: format!("{:x}", hash.finalize()),
        })
    }

    fn argument(&self) -> Result<&str> {
        self.path
            .to_str()
            .context("The temporary package path is not UTF-8")
    }
}

async fn query(program: &str, args: &[&str]) -> Result<String> {
    let output =
        system::capture_output_env(program, args, &[("LC_ALL", "C")], QUERY_TIMEOUT).await?;
    if !output.success {
        bail!("{program} failed: {}", output.stderr.trim());
    }
    Ok(output.stdout)
}

fn metadata(stdout: &str) -> Result<(String, String, String)> {
    let mut lines = stdout.lines();
    let package = lines.next().unwrap_or_default();
    let version = lines.next().unwrap_or_default();
    let architecture = lines.next().unwrap_or_default();
    if package.len() < 2
        || !package.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        || !package
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"+.-".contains(&c))
        || version.is_empty()
        || version.starts_with('-')
        || version.chars().any(char::is_whitespace)
        || architecture.is_empty()
        || !architecture
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        bail!("The archive has invalid Debian package metadata.");
    }
    ensure_allowed(package)?;
    Ok((package.into(), version.into(), architecture.into()))
}

fn ensure_allowed(package: &str) -> Result<()> {
    let protection = safety::check_package(PackageSource::Apt, package);
    if protection.protected {
        bail!(
            "Cannot install a protected package: {}",
            protection.reason.unwrap_or_default()
        );
    }
    Ok(())
}

async fn installed_version(package: &str) -> Result<Option<String>> {
    let state = probe::probe_package(PackageSource::Apt, package, None).await?;
    if !state.present {
        return Ok(None);
    }
    Ok(Some(
        state
            .version
            .filter(|v| !v.trim().is_empty())
            .context("Cannot verify the installed version")?,
    ))
}

async fn transaction(archive: &Archive, package: &str) -> Result<apt_transaction::AptTransaction> {
    let output = query(
        "apt-get",
        &["-s", "install", "-y", "--", archive.argument()?],
    )
    .await?;
    let transaction = apt_transaction::parse(&output, Operation::Install, package)?;
    // Installing new dependencies is allowed; replacing protected installed
    // packages is not, even when APT introduces that change indirectly.
    validate_transaction(&transaction)?;
    Ok(transaction)
}

fn validate_transaction(transaction: &apt_transaction::AptTransaction) -> Result<()> {
    for upgraded in &transaction.upgrades {
        ensure_allowed(upgraded)?;
    }
    Ok(())
}

pub async fn preview(source: &Path) -> Result<OperationPlan> {
    let source = std::path::absolute(source)?;
    let archive = Archive::copy(&source)?;
    let output = query(
        "dpkg-deb",
        &[
            "--show",
            "--showformat=${Package}\n${Version}\n${Architecture}\n",
            archive.argument()?,
        ],
    )
    .await?;
    let (package_id, version, architecture) = metadata(&output)?;
    query("dpkg", &["--validate-version", &version]).await?;
    let host = query("dpkg", &["--print-architecture"]).await?;
    if architecture != "all" && architecture != host.trim() {
        bail!(
            "This package is built for {architecture}, but this computer uses {}.",
            host.trim()
        );
    }
    let installed_version = installed_version(&package_id).await?;
    if let Some(current) = &installed_version {
        let newer = system::capture_output(
            "dpkg",
            &["--compare-versions", &version, "gt", current],
            QUERY_TIMEOUT,
        )
        .await?;
        if !newer.success {
            bail!("Version {current} is already installed. Scope installs only newer versions.");
        }
    }
    let transaction = transaction(&archive, &package_id).await?;
    let steps = vec![
        PlanStep {
            description: format!("Install {package_id} {version} system-wide with APT. Only install files from publishers you trust. Package scripts run with administrator privileges."),
            command_summary: format!("pkexec env DEBIAN_FRONTEND=noninteractive apt-get install -y -- {:?} (verified private copy)", source),
        },
        PlanStep {
            description: format!("APT installs: {}. Upgrades: {}. Removes: {}.", list(&transaction.installs), list(&transaction.upgrades), list(&transaction.removals)),
            command_summary: "apt-get -s install (checked again before installation)".into(),
        },
        PlanStep {
            description: "The package will be managed by APT. Updates use your configured repositories, including any repository this package sets up. If none provides this package, open a newer .deb in Scope. Scope does not add repositories or signing keys. The downloaded file is kept.".into(),
            command_summary: String::new(),
        },
    ];
    Ok(OperationPlan {
        plan_id: super::new_plan_id(),
        operation: Operation::Install,
        display_name: package_id.clone(),
        current_version: installed_version.clone().unwrap_or_default(),
        target_version: version.clone(),
        requires_auth: true,
        auth_method: AuthMethod::Pkexec,
        protected: false,
        protection_reason: None,
        steps,
        created_at_ms: super::now_ms(),
        apt_transaction_fingerprint: Some(transaction.fingerprint),
        snap_details: None,
        app_data: vec![],
        target: OperationTarget::DebInstall {
            plan: Box::new(DebInstallPlan {
                source,
                sha256: archive.sha256,
                package_id,
                version,
                architecture,
                installed_version,
            }),
        },
    })
}

fn list(packages: &[String]) -> String {
    if packages.is_empty() {
        "none".into()
    } else {
        packages.join(", ")
    }
}

async fn verified_archive(plan: &DebInstallPlan) -> Result<Archive> {
    ensure_allowed(&plan.package_id)?;
    let archive = Archive::copy(&plan.source)?;
    if archive.sha256 != plan.sha256 {
        bail!("The .deb file changed since preview. Open it again.");
    }
    validate_installed_state(
        plan.installed_version.as_deref(),
        installed_version(&plan.package_id).await?.as_deref(),
    )?;
    Ok(archive)
}

fn validate_installed_state(expected: Option<&str>, actual: Option<&str>) -> Result<()> {
    if expected != actual {
        bail!("The installed package changed since preview. Open the .deb again.");
    }
    Ok(())
}

pub async fn revalidate(plan: &DebInstallPlan) -> Result<()> {
    verified_archive(plan).await?;
    Ok(())
}

pub async fn apply(
    plan: &OperationPlan,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> Result<OperationResult> {
    let OperationTarget::DebInstall { plan: install } = &plan.target else {
        bail!("Not a Debian installation plan");
    };
    if plan.operation != Operation::Install || plan.protected {
        bail!("Not an allowed installation plan");
    }
    if plan.apt_transaction_fingerprint.is_none() {
        bail!("The installation plan has no verified APT transaction. Preview again.");
    }
    let archive = verified_archive(install).await?;
    let transaction = transaction(&archive, &install.package_id).await?;
    if plan.apt_transaction_fingerprint.as_deref() != Some(transaction.fingerprint.as_str()) {
        bail!("The APT transaction changed since preview. Open the .deb again.");
    }
    let mut result = system::run_elevated(
        "apt-get",
        &["install", "-y", "--", archive.argument()?],
        AuthMethod::Pkexec,
        INSTALL_TIMEOUT,
        on_line,
    )
    .await;
    if result.success {
        if installed_version(&install.package_id).await?.as_deref() != Some(&install.version) {
            bail!("APT finished, but the installed version could not be confirmed. Rescan to check the package.");
        }
        result.message = format!(
            "Installed {} {}. Future updates use configured APT repositories.",
            install.package_id, install.version
        );
    }
    Ok(result)
}

#[cfg(test)]
#[path = "deb_install_tests.rs"]
mod tests;
