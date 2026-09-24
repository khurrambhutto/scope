//! Targeted single-package state probes for apply-time revalidation.
//!
//! Applying a plan must re-check the live system first, but a full `scan_all()`
//! puts seconds — and network calls — between the user's confirmation and the
//! Polkit password dialog. These probes ask one cheap question per source ("is
//! this package still installed, at which version, and does an update still
//! exist?") so the dialog appears almost immediately while keeping the
//! preview-first, backend-validated safety model intact.

use std::time::Duration;

use anyhow::Result;

use crate::package::{InstallScope, PackageSource};
use crate::system::capture_output;

/// Live state of a single package, probed right before an operation runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbedPackage {
    /// Whether the package is currently installed.
    pub present: bool,
    /// Installed version when known. Used to reject plans whose package
    /// changed since preview.
    pub version: Option<String>,
    /// Whether an update is available, when that is cheap to determine (APT).
    /// `None` means unknown and must not be read as "no update".
    pub has_update: Option<bool>,
}

impl ProbedPackage {
    fn absent() -> Self {
        Self {
            present: false,
            version: None,
            has_update: None,
        }
    }

    fn installed(version: Option<String>) -> Self {
        Self {
            present: true,
            version,
            has_update: None,
        }
    }
}

/// Probe timeout for local metadata queries.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// snapd talks to its daemon over D-Bus and can hang; keep the scanner's
/// tighter budget (see `scanner::snap`).
const SNAP_PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// Probe the current state of one package.
///
/// `Err` means "could not verify" — callers must fail closed and refuse to
/// execute the plan rather than assume the package is gone.
pub async fn probe_package(
    source: PackageSource,
    package_id: &str,
    install_scope: Option<InstallScope>,
) -> Result<ProbedPackage> {
    match source {
        PackageSource::Apt => probe_apt(package_id).await,
        PackageSource::Snap => probe_snap(package_id).await,
        PackageSource::Flatpak => probe_flatpak(package_id, install_scope).await,
        PackageSource::AppImage => probe_appimage(package_id).await,
    }
}

async fn probe_apt(pkg: &str) -> Result<ProbedPackage> {
    let out = capture_output(
        "dpkg-query",
        &["-W", "-f=${Status}\t${Version}", pkg],
        PROBE_TIMEOUT,
    )
    .await?;
    if !out.success {
        if is_dpkg_not_found(&out.stderr) {
            return Ok(ProbedPackage::absent());
        }
        anyhow::bail!("dpkg-query failed: {}", out.stderr.trim());
    }
    let Some((status, version)) = parse_dpkg_query(&out.stdout) else {
        anyhow::bail!("unexpected dpkg-query output: {}", out.stdout.trim());
    };
    if !dpkg_status_installed(&status) {
        return Ok(ProbedPackage::absent());
    }
    let mut probed = ProbedPackage::installed(Some(version));
    probed.has_update = apt_has_update(pkg).await;
    Ok(probed)
}

/// Update availability from `apt-cache policy` (local metadata only, no
/// network refresh).
async fn apt_has_update(pkg: &str) -> Option<bool> {
    let out = capture_output("apt-cache", &["policy", pkg], PROBE_TIMEOUT)
        .await
        .ok()?;
    if !out.success {
        return None;
    }
    let (installed, candidate) = parse_apt_policy(&out.stdout);
    match (installed, candidate) {
        (Some(installed), Some(candidate)) => {
            Some(has_update_from_policy(&installed, &candidate))
        }
        _ => None,
    }
}

async fn probe_snap(pkg: &str) -> Result<ProbedPackage> {
    let out = capture_output("snap", &["list", pkg], SNAP_PROBE_TIMEOUT).await?;
    if !out.success {
        if is_snap_not_installed(&out.stderr) {
            return Ok(ProbedPackage::absent());
        }
        anyhow::bail!("snap list failed: {}", out.stderr.trim());
    }
    match parse_snap_list_version(&out.stdout) {
        Some(version) => Ok(ProbedPackage::installed(Some(version))),
        None => anyhow::bail!("unexpected snap list output: {}", out.stdout.trim()),
    }
}

async fn probe_flatpak(app_id: &str, scope: Option<InstallScope>) -> Result<ProbedPackage> {
    // Scope comes from the plan (originally from the scan), never re-guessed.
    let scope_flag = match scope {
        Some(InstallScope::User) => "--user",
        Some(InstallScope::System) | None => "--system",
    };
    let out = capture_output(
        "flatpak",
        &["list", scope_flag, "--app", "--columns=application,version"],
        PROBE_TIMEOUT,
    )
    .await?;
    if !out.success {
        anyhow::bail!("flatpak list failed: {}", out.stderr.trim());
    }
    match find_flatpak_version(&out.stdout, app_id) {
        Some(version) => Ok(ProbedPackage::installed(if version.is_empty() {
            None
        } else {
            Some(version)
        })),
        None => Ok(ProbedPackage::absent()),
    }
}

async fn probe_appimage(path: &str) -> Result<ProbedPackage> {
    match tokio::fs::metadata(path).await {
        Ok(meta) if meta.is_file() => Ok(ProbedPackage::installed(None)),
        Ok(_) => Ok(ProbedPackage::absent()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ProbedPackage::absent()),
        Err(e) => anyhow::bail!("could not stat AppImage '{path}': {e}"),
    }
}

/// Parse `dpkg-query -f='${Status}\t${Version}'` output into (status, version).
fn parse_dpkg_query(stdout: &str) -> Option<(String, String)> {
    let line = stdout.lines().next()?.trim();
    let (status, version) = line.split_once('\t')?;
    Some((status.trim().to_string(), version.trim().to_string()))
}

/// `install ok installed` means actually installed; states like
/// `deinstall ok config-files` or `purge ok not-installed` do not.
fn dpkg_status_installed(status: &str) -> bool {
    status.split_whitespace().last() == Some("installed")
}

/// Parse the `Installed:`/`Candidate:` lines of `apt-cache policy` output.
fn parse_apt_policy(stdout: &str) -> (Option<String>, Option<String>) {
    let mut installed = None;
    let mut candidate = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(v) = trimmed.strip_prefix("Installed:") {
            installed = Some(v.trim().to_string());
        } else if let Some(v) = trimmed.strip_prefix("Candidate:") {
            candidate = Some(v.trim().to_string());
        }
    }
    (installed, candidate)
}

fn has_update_from_policy(installed: &str, candidate: &str) -> bool {
    installed != "(none)" && candidate != "(none)" && installed != candidate
}

/// Extract the version column (2nd) from `snap list <name>` output, skipping
/// the header row.
fn parse_snap_list_version(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("Name"))
        .find_map(|l| l.split_whitespace().nth(1).map(|v| v.to_string()))
}

/// Extract the version column for `app_id` from
/// `flatpak list --columns=application,version` (tab-separated).
fn find_flatpak_version(stdout: &str, app_id: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        let (id, version) = line.split_once('\t')?;
        (id == app_id).then(|| version.trim().to_string())
    })
}

fn is_dpkg_not_found(stderr: &str) -> bool {
    stderr.to_ascii_lowercase().contains("no packages found")
}

fn is_snap_not_installed(stderr: &str) -> bool {
    stderr.to_ascii_lowercase().contains("no matching snaps")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpkg_query_parses_status_and_version() {
        assert_eq!(
            parse_dpkg_query("install ok installed\t1.2.3-4"),
            Some(("install ok installed".into(), "1.2.3-4".into()))
        );
    }

    #[test]
    fn dpkg_query_rejects_output_without_separator() {
        assert_eq!(parse_dpkg_query("install ok installed"), None);
        assert_eq!(parse_dpkg_query(""), None);
    }

    #[test]
    fn dpkg_status_only_counts_real_installs() {
        assert!(dpkg_status_installed("install ok installed"));
        assert!(!dpkg_status_installed("deinstall ok config-files"));
        assert!(!dpkg_status_installed("purge ok not-installed"));
    }

    #[test]
    fn apt_policy_parses_installed_and_candidate() {
        let out = "firefox:\n  Installed: 130.0\n  Candidate: 131.0\n  Version table:\n";
        assert_eq!(
            parse_apt_policy(out),
            (Some("130.0".into()), Some("131.0".into()))
        );
    }

    #[test]
    fn apt_policy_missing_fields_yield_none() {
        assert_eq!(parse_apt_policy("some-package:"), (None, None));
    }

    #[test]
    fn policy_update_flags() {
        assert!(has_update_from_policy("1.0", "2.0"));
        assert!(!has_update_from_policy("1.0", "1.0"));
        assert!(!has_update_from_policy("1.0", "(none)"));
        assert!(!has_update_from_policy("(none)", "2.0"));
    }

    #[test]
    fn snap_list_skips_header_and_reads_version() {
        let out = "Name    Version  Rev  Tracking       Publisher  Notes\n\
                   firefox 130.0-1  4567 latest/stable  mozilla*   -\n";
        assert_eq!(parse_snap_list_version(out), Some("130.0-1".into()));
    }

    #[test]
    fn snap_list_header_only_yields_none() {
        assert_eq!(
            parse_snap_list_version("Name Version Rev Tracking Publisher Notes\n"),
            None
        );
    }

    #[test]
    fn flatpak_row_found_by_exact_id() {
        let out = "org.gnome.Builder\t46.3\norg.gnome.Boxes\t46.1\n";
        assert_eq!(
            find_flatpak_version(out, "org.gnome.Boxes"),
            Some("46.1".into())
        );
        assert_eq!(find_flatpak_version(out, "org.gnome.Box"), None);
    }

    #[test]
    fn flatpak_missing_error_messages_are_detected() {
        assert!(is_dpkg_not_found(
            "dpkg-query: no packages found matching foo"
        ));
        assert!(!is_dpkg_not_found("dpkg-query: error: something broke"));
        assert!(is_snap_not_installed("error: no matching snaps installed"));
        assert!(!is_snap_not_installed(
            "error: cannot communicate with server"
        ));
    }
}
