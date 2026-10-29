//! APT/dpkg scanner.
//!
//! Read manual packages first, then add installed owners of the shared visible
//! desktop catalog before checking updates. Automatic desktop apps retain their
//! installation provenance; other dependencies stay outside this inventory.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::future::Future;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::domain::package::{AppKind, InstalledPackage, PackageSource};
use crate::domain::scanner::{ScanReport, Scanner};
use crate::domain::system::{capture_stdout, which, SystemError, SCAN_TIMEOUT};

pub struct AptScanner;

impl Scanner for AptScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Apt
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { which("dpkg-query") && which("apt-mark") })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
        Box::pin(async { scan().await.map(ScanReport::ok) })
    }

    fn scan_desktop_apps<'a>(
        &'a self,
        report: &'a mut ScanReport,
        desktop: &'a crate::domain::desktop_entries::DesktopIndex,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let apps: Vec<_> = desktop.visible_apps().collect();
            super::apt_desktop::extend(report, &apps).await
        })
    }

    fn check_updates<'a>(
        &'a self,
        report: &'a mut ScanReport,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(check_updates(report))
    }
}

// Field separator unlikely to appear in package metadata.
const SEP: &str = "\x1f";

async fn scan() -> Result<Vec<InstalledPackage>> {
    let manual = capture_stdout("apt-mark", &["showmanual"], SCAN_TIMEOUT)
        .await
        .context("read manually-installed packages")?;
    let manual: HashSet<String> = manual
        .lines()
        .filter_map(|l| {
            let trimmed = l.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect();
    if manual.is_empty() {
        return Ok(Vec::new());
    }

    // One dpkg-query over all manual names. Installed-Size is in KiB.
    // ${binary:Summary} truncates the description to one line; perfect for the UI.
    // Section lets us reject library and metapackage rows before command detection.
    let format = format!(
        "${{Package}}{SEP}${{Version}}{SEP}${{Installed-Size}}{SEP}${{binary:Summary}}{SEP}${{Section}}{SEP}\\n"
    );
    let mut args: Vec<String> = vec!["-W".into(), format!("-f={format}")];
    args.extend(manual);

    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = capture_stdout("dpkg-query", &argv, Duration::from_secs(20))
        .await
        .context("query dpkg metadata for manual packages")?;

    let mut packages = Vec::new();
    // dpkg-query emits one row per architecture for multiarch packages (e.g.
    // libc6:amd64 and libc6:i386). Keep the first row per package name so
    // every package key stays unique.
    let mut seen: HashSet<String> = HashSet::new();
    for line in output.lines() {
        let mut fields = line.split(SEP);
        let (Some(name), Some(version), Some(kib)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !seen.insert(name.to_string()) {
            continue;
        }
        let summary = fields.next().unwrap_or("");
        let section = fields.next().unwrap_or("");

        let mut pkg = InstalledPackage::new(PackageSource::Apt, name);
        pkg.name = pkg.package_id.clone();
        pkg.version = version.to_string();
        pkg.size_bytes = kib.parse().unwrap_or(0) * 1024;
        if !summary.is_empty() {
            pkg.description = Some(summary.to_string());
        }
        pkg.app_kind = classify(&pkg.name, section);
        packages.push(pkg);
    }
    Ok(packages)
}

/// Best-effort CLI classification from files owned by a manually installed
/// package. Desktop launchers are classified later by desktop-entry enrichment.
fn classify(name: &str, section: &str) -> AppKind {
    if !is_user_cli_section(section) {
        return AppKind::Unknown;
    }
    let files = crate::domain::package_files::list(name);
    let has_service = files.iter().any(|file| is_system_service_path(file));
    let has_command = files.iter().any(|file| {
        is_public_command_path(file)
            && fs::metadata(file).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    });
    classify_from_signals(has_command, has_service)
}

fn classify_from_signals(has_public_command: bool, has_system_service: bool) -> AppKind {
    if has_system_service {
        AppKind::Unknown
    } else if has_public_command {
        AppKind::Cli
    } else {
        AppKind::Unknown
    }
}

fn is_user_cli_section(section: &str) -> bool {
    matches!(
        section.rsplit('/').next().unwrap_or(section),
        "devel"
            | "editors"
            | "education"
            | "games"
            | "graphics"
            | "net"
            | "python"
            | "science"
            | "shells"
            | "sound"
            | "text"
            | "utils"
            | "vcs"
            | "video"
            | "web"
            | "x11"
    )
}

fn is_public_command_path(file: &Path) -> bool {
    // Deliberately omit sbin: those paths are primarily system-administration
    // commands and daemons, not apps or everyday user CLI tools.
    let Some(parent) = Path::new(file).parent().and_then(Path::to_str) else {
        return false;
    };
    matches!(
        parent,
        "/usr/bin" | "/bin" | "/usr/local/bin" | "/usr/games"
    )
}

fn is_system_service_path(file: &Path) -> bool {
    file.starts_with("/etc/init.d")
        || file.starts_with("/lib/systemd/system")
        || file.starts_with("/usr/lib/systemd/system")
}

/// Run `apt list --upgradable` and mark packages that have available updates.
async fn check_updates(report: &mut ScanReport) {
    if report.packages.is_empty() {
        return;
    }
    let result = capture_stdout("apt", &["list", "--upgradable"], Duration::from_secs(30)).await;
    apply_update_result(report, result);
}

fn apply_update_result(report: &mut ScanReport, result: Result<String, SystemError>) {
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            let message = format!("APT update check failed: {error}");
            for package in &mut report.packages {
                super::update_status::unavailable(package, &mut report.warning, &message);
            }
            return;
        }
    };
    for package in &mut report.packages {
        super::update_status::checked(package);
    }

    // Line format: "package/suite candidate_version arch [upgradable from: old_version]"
    // Or:          "package/arch candidate_version arch [held]"
    let Some(re) = upgrade_regex() else {
        return;
    };

    let indexes: HashMap<String, usize> = report
        .packages
        .iter()
        .enumerate()
        .map(|(index, package)| (package.package_id.clone(), index))
        .collect();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line == "Listing..." || line.starts_with("WARNING:") {
            continue;
        }
        if let Some(caps) = re.captures(line) {
            let name = &caps[1];
            let candidate = &caps[3];
            let architecture = line.split_whitespace().nth(2).unwrap_or("");
            if let Some(pkg) = indexes
                .get(&format!("{name}:{architecture}"))
                .or_else(|| indexes.get(name))
                .and_then(|index| report.packages.get_mut(*index))
            {
                pkg.has_update = true;
                pkg.update_version = Some(candidate.to_string());
            }
        }
    }
}

/// Compiled once for the process; `None` if the pattern is somehow invalid.
fn upgrade_regex() -> Option<&'static regex::Regex> {
    static RE: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^(\S+)/(\S+)\s+(\S+)\s+\S+\s+\[upgradable from: (\S+)\]").ok()
    })
    .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_sections_are_user_facing_not_system_infrastructure() {
        for section in ["utils", "x11", "web", "vcs", "universe/net"] {
            assert!(is_user_cli_section(section), "{section} should be included");
        }
        for section in [
            "libs",
            "libdevel",
            "metapackages",
            "oldlibs",
            "database",
            "admin",
        ] {
            assert!(
                !is_user_cli_section(section),
                "{section} should be excluded"
            );
        }
    }

    #[test]
    fn public_commands_are_distinguished_from_services_and_libraries() {
        assert!(is_public_command_path(Path::new("/usr/bin/wl-copy")));
        assert!(is_public_command_path(Path::new("/usr/games/steam")));
        assert!(!is_public_command_path(Path::new(
            "/usr/sbin/wpa_supplicant"
        )));
        assert!(!is_public_command_path(Path::new("/usr/lib/libexample.so")));
        assert!(is_system_service_path(Path::new(
            "/lib/systemd/system/example.service"
        )));
        assert!(is_system_service_path(Path::new("/etc/init.d/example")));
        assert!(!is_system_service_path(Path::new("/usr/bin/example")));
    }

    #[test]
    fn cli_classification_requires_a_candidate_section_and_no_system_service() {
        assert_eq!(classify_from_signals(true, false), AppKind::Cli);
        assert_eq!(classify_from_signals(false, false), AppKind::Unknown);
        assert_eq!(classify_from_signals(true, true), AppKind::Unknown);
    }

    fn pending_package() -> InstalledPackage {
        let mut package = InstalledPackage::new(PackageSource::Apt, "firefox");
        package.version = "1.0".into();
        package.update_note = Some("Checking for updates...".into());
        package
    }

    #[test]
    fn failed_update_result_retains_packages_and_reports_unknown_status() {
        let mut report = ScanReport::ok(vec![pending_package()]);
        let error = SystemError::Timeout {
            program: "apt".into(),
            timeout: Duration::from_secs(30),
        };

        apply_update_result(&mut report, Err(error));

        assert_eq!(report.packages.len(), 1);
        assert_eq!(
            report.packages[0].update_note.as_deref(),
            Some("APT update check failed: apt timed out after 30s")
        );
        assert_eq!(
            report.warning.as_deref(),
            report.packages[0].update_note.as_deref()
        );
        assert!(!report.packages[0].has_update);
    }

    #[test]
    fn successful_empty_update_result_clears_pending_status() {
        let mut report = ScanReport::ok(vec![pending_package()]);

        apply_update_result(&mut report, Ok("Listing...\n".into()));

        assert_eq!(report.packages[0].update_note, None);
        assert!(!report.packages[0].has_update);
        assert_eq!(report.warning, None);
    }

    #[test]
    fn automatic_desktop_apps_receive_native_update_candidates() {
        let mut package = InstalledPackage::new(PackageSource::Apt, "gnome-calculator:amd64");
        package.apt_automatic = true;
        package.app_kind = AppKind::Gui;
        package.version = "1.0".into();
        let mut report = ScanReport::ok(vec![package]);
        apply_update_result(
            &mut report,
            Ok("gnome-calculator/stable 2.0 amd64 [upgradable from: 1.0]\n".into()),
        );
        assert!(report.packages[0].has_update);
        assert_eq!(report.packages[0].update_version.as_deref(), Some("2.0"));
        assert!(report.packages[0].apt_automatic);
        assert!(
            crate::domain::operations::OperationTarget::from_package(&report.packages[0]).is_ok()
        );
    }

    #[test]
    fn successful_update_result_maps_the_candidate_version() {
        let mut report = ScanReport::ok(vec![pending_package()]);

        apply_update_result(
            &mut report,
            Ok("Listing...\nfirefox/stable 2.0 amd64 [upgradable from: 1.0]\n".into()),
        );

        assert!(report.packages[0].has_update);
        assert_eq!(report.packages[0].update_version.as_deref(), Some("2.0"));
        assert_eq!(report.packages[0].update_note, None);
    }
}
