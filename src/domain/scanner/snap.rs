//! Snap scanner.
//!
//! Strategy: parse `snap list` for installed snaps and skip base/runtime snaps
//! (core, bare, snapd, gtk-*, gnome-*).
//!
//! Two platform realities shape this scanner:
//!
//! 1. Every `snap` subcommand talks to `snapd` over the system bus, and a busy,
//!    restarting, or unreachable daemon makes them hang instead of fail. Each
//!    invocation is bounded by the much shorter `SNAP_TIMEOUT`, so a wedged
//!    daemon surfaces as a per-source error rather than stalling the whole scan.
//! 2. `snap list` cannot report sizes, and measuring the mounted
//!    `/snap/<name>/current` tree means walking every file in the snap (one
//!    `du` process per snap in the naive form). Sizes come from a single listing
//!    of `/var/lib/snapd/snaps/<name>_<revision>.snap` — the compressed image
//!    snapd actually stores and mounts — and only snaps with no image on disk
//!    fall back to one batched `du` pass.

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::domain::package::{AppKind, InstalledPackage, PackageSource};
use crate::domain::scanner::{ScanReport, Scanner};
use crate::domain::system::{capture_output, capture_stdout, which, SystemError};

pub struct SnapScanner;

/// Bound for every `snap` invocation. Snapd answers over the system bus, so a
/// request can block far longer than reading local files ever would.
const SNAP_TIMEOUT: Duration = Duration::from_secs(8);

/// Cap on the fallback `du` pass for snaps with no image file on disk.
const MOUNTED_SIZE_TIMEOUT: Duration = Duration::from_secs(5);

/// Directory holding the revision images snapd mounts at `/snap/<name>/current`.
const SNAPS_DIR: &str = "/var/lib/snapd/snaps";

impl Scanner for SnapScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Snap
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { which("snap") && Path::new("/var/lib/snapd").exists() })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
        Box::pin(async { scan().await.map(ScanReport::ok) })
    }

    fn check_updates<'a>(
        &'a self,
        report: &'a mut ScanReport,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(check_updates(report))
    }
}

/// One parsed row of `snap list`.
struct SnapRow {
    name: String,
    version: String,
    revision: String,
}

async fn scan() -> Result<Vec<InstalledPackage>> {
    // Whitespace columns: Name Version Rev Tracking Publisher Notes
    let output = capture_stdout("snap", &["list"], SNAP_TIMEOUT)
        .await
        .context("snap list")?;

    let mut rows = Vec::new();
    for line in output.lines().skip(1) {
        let mut fields = line.split_whitespace();
        let (Some(name), Some(version), Some(revision), Some(_)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if is_runtime(name) {
            continue;
        }
        rows.push(SnapRow {
            name: name.to_string(),
            version: version.to_string(),
            revision: revision.to_string(),
        });
    }

    let sizes = snap_sizes(&rows).await;

    let mut packages = Vec::with_capacity(rows.len());
    for row in rows {
        let size = sizes.get(&row.name).copied().unwrap_or(0);
        packages.push(package_from_row(row, size, Path::new("/snap")));
    }

    Ok(packages)
}

fn package_from_row(row: SnapRow, size_bytes: u64, snap_root: &Path) -> InstalledPackage {
    let display_version = super::snap_version::display_version(snap_root, &row.name, &row.revision);
    let mut pkg = InstalledPackage::new(PackageSource::Snap, row.name);
    pkg.name = pkg.package_id.clone();
    pkg.version = row.version;
    pkg.display_version = display_version;
    pkg.size_bytes = size_bytes;
    pkg.app_kind = if has_snap_command(&pkg.package_id) {
        AppKind::Cli
    } else {
        AppKind::Unknown
    };
    pkg
}

/// On-disk size of every installed snap, keyed by snap name.
///
/// The fast path is a single `read_dir` of snapd's image directory: the
/// `<name>_<revision>.snap` image is what snapd stores and mounts, so its length
/// is the space the snap actually occupies. Snaps without a matching image fall
/// back to one batched `du` over their mounted trees.
async fn snap_sizes(rows: &[SnapRow]) -> HashMap<String, u64> {
    let images = read_snap_images();
    let mut sizes = HashMap::with_capacity(rows.len());
    let mut unmeasured = Vec::new();

    for row in rows {
        match images.get(&image_stem(&row.name, &row.revision)) {
            Some(bytes) => {
                sizes.insert(row.name.clone(), *bytes);
            }
            None => unmeasured.push(row.name.clone()),
        }
    }

    if !unmeasured.is_empty() {
        sizes.extend(measure_mounted(&unmeasured).await);
    }
    sizes
}

/// Snapd stores each revision as `<name>_<revision>.snap`.
fn image_stem(name: &str, revision: &str) -> String {
    format!("{name}_{revision}")
}

/// Index the `.snap` images snapd has on disk, keyed by image stem.
fn read_snap_images() -> HashMap<String, u64> {
    let mut images = HashMap::new();
    let Ok(entries) = std::fs::read_dir(SNAPS_DIR) else {
        return images;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("snap") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if let Ok(metadata) = entry.metadata() {
            images.insert(stem.to_string(), metadata.len());
        }
    }
    images
}

/// Fallback for snaps with no image file: one `du -sbL` over every mounted
/// `/snap/<name>/current`. `du` exits non-zero when it meets an unreadable file
/// (some snaps ship broken symlinks) while still printing totals for the paths it
/// could read, so stdout is parsed regardless of exit status.
async fn measure_mounted(names: &[String]) -> HashMap<String, u64> {
    let targets: Vec<std::path::PathBuf> = names
        .iter()
        .map(|name| Path::new("/snap").join(name).join("current"))
        .filter(|path| path.exists())
        .collect();
    if targets.is_empty() {
        return HashMap::new();
    }

    let mut args = vec!["-sbL".to_string()];
    args.extend(
        targets
            .iter()
            .map(|path| path.to_string_lossy().into_owned()),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let Ok(out) = capture_output("du", &args, MOUNTED_SIZE_TIMEOUT).await else {
        return HashMap::new();
    };

    parse_du_totals(&out.stdout)
}

/// Parse `du` totals into a snap-name to bytes map.
///
/// `du` prints `<bytes>\t<path>` for each requested path. Only lines carrying
/// both a parsable size and a `/snap/<name>/current` path are kept, which makes
/// this tolerant of partial output and of `du` diagnostics that leak onto stdout.
/// Snap names cannot contain spaces, so whitespace splitting is unambiguous here.
fn parse_du_totals(stdout: &str) -> HashMap<String, u64> {
    let mut sizes = HashMap::new();
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let (Some(bytes), Some(path)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(bytes) = bytes.parse::<u64>() else {
            continue;
        };
        if let Some(name) = path
            .strip_prefix("/snap/")
            .and_then(|rest| rest.strip_suffix("/current"))
        {
            sizes.insert(name.to_string(), bytes);
        }
    }
    sizes
}

/// Run `snap refresh --list` and mark snaps that have available updates.
async fn check_updates(report: &mut ScanReport) {
    if report.packages.is_empty() {
        return;
    }
    let result = capture_stdout("snap", &["refresh", "--list"], SNAP_TIMEOUT).await;
    apply_update_result(report, result);
}

fn apply_update_result(report: &mut ScanReport, result: Result<String, SystemError>) {
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            let message = format!("Snap update check failed: {error}");
            for package in &mut report.packages {
                super::update_status::unavailable(package, &mut report.warning, &message);
            }
            return;
        }
    };
    for package in &mut report.packages {
        super::update_status::checked(package);
    }

    let indexes: HashMap<String, usize> = report
        .packages
        .iter()
        .enumerate()
        .map(|(index, package)| (package.package_id.clone(), index))
        .collect();
    for line in output.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(name), Some(version), Some(_), Some(_)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if let Some(pkg) = indexes
            .get(name)
            .and_then(|index| report.packages.get_mut(*index))
        {
            pkg.has_update = true;
            pkg.update_version = (!version.is_empty()).then(|| version.to_string());
        }
    }
}

fn is_runtime(name: &str) -> bool {
    name == "snapd"
        || name == "bare"
        || name.starts_with("core")
        || name.starts_with("gtk-")
        || name.starts_with("gnome-")
        || name.ends_with("-gtk3")
}

fn has_snap_command(name: &str) -> bool {
    Path::new(&format!("/snap/bin/{name}")).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_displays_release_version_without_replacing_snap_version() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root
            .path()
            .join("code/268/usr/share/code/resources/app/package.json");
        std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        std::fs::write(manifest, r#"{"name":"Code","version":"1.141.0"}"#).unwrap();
        let pkg = package_from_row(
            SnapRow {
                name: "code".into(),
                version: "2a59476c".into(),
                revision: "268".into(),
            },
            530,
            root.path(),
        );
        assert_eq!(pkg.version_label(), "1.141.0");
        assert_eq!(pkg.version, "2a59476c");
    }

    #[test]
    fn missing_app_metadata_falls_back_to_the_snap_version() {
        let root = tempfile::tempdir().unwrap();
        let pkg = package_from_row(
            SnapRow {
                name: "code".into(),
                version: "2a59476c".into(),
                revision: "268".into(),
            },
            530,
            root.path(),
        );
        assert_eq!(pkg.version_label(), "2a59476c");
        assert_eq!(pkg.display_version, None);
    }

    #[test]
    fn image_stem_matches_snapd_layout() {
        assert_eq!(image_stem("code", "263"), "code_263");
        assert_eq!(
            image_stem("desktop-security-center", "12"),
            "desktop-security-center_12"
        );
    }

    /// `du` can exit non-zero (unreadable files inside a snap) while still
    /// printing totals for the paths it could read, so the parser must accept
    /// clean totals mixed with diagnostics.
    #[test]
    fn du_totals_keep_snap_paths_and_skip_noise() {
        let stdout = "1234437173\t/snap/vlc/current\n4096\t/snap/code/current\n\
                      du: cannot access '/snap/vlc/current/share/x.svg': No such file or directory\n";
        let sizes = parse_du_totals(stdout);
        assert_eq!(sizes.get("vlc"), Some(&1_234_437_173));
        assert_eq!(sizes.get("code"), Some(&4096));
        assert_eq!(sizes.len(), 2);
    }

    #[test]
    fn du_totals_ignore_unrelated_paths_and_bad_sizes() {
        let sizes = parse_du_totals("not-a-size\t/snap/foo/current\n12\t/elsewhere/foo\n\n");
        assert!(sizes.is_empty());
    }

    /// The scrubber must keep real apps while hiding runtimes and base snaps.
    #[test]
    fn runtime_scrub_covers_snap_internals() {
        for runtime in [
            "snapd",
            "bare",
            "core22",
            "gtk-common-themes",
            "gnome-46-2404",
        ] {
            assert!(
                is_runtime(runtime),
                "{runtime} should be treated as a runtime"
            );
        }
        for app in ["firefox", "code", "vlc", "onlyoffice-desktopeditors"] {
            assert!(!is_runtime(app), "{app} should be treated as an app");
        }
    }

    fn pending_package(version: &str) -> InstalledPackage {
        let mut package = InstalledPackage::new(PackageSource::Snap, "firefox");
        package.version = version.into();
        package.update_note = Some("Checking for updates...".into());
        package
    }

    #[test]
    fn failed_update_result_retains_packages_and_reports_unknown_status() {
        let mut report = ScanReport::ok(vec![pending_package("157.0.1-1")]);
        let error = SystemError::Timeout {
            program: "snap".into(),
            timeout: SNAP_TIMEOUT,
        };

        apply_update_result(&mut report, Err(error));

        assert_eq!(report.packages.len(), 1);
        assert_eq!(
            report.packages[0].update_note.as_deref(),
            Some("Snap update check failed: snap timed out after 8s")
        );
        assert_eq!(
            report.warning.as_deref(),
            report.packages[0].update_note.as_deref()
        );
        assert!(!report.packages[0].has_update);
    }

    #[test]
    fn successful_empty_update_result_clears_pending_status() {
        let mut report = ScanReport::ok(vec![pending_package("157.0.1-1")]);

        apply_update_result(
            &mut report,
            Ok("Name Version Rev Tracking Publisher Notes\n".into()),
        );

        assert_eq!(report.packages[0].update_note, None);
        assert!(!report.packages[0].has_update);
        assert_eq!(report.warning, None);
    }

    #[test]
    fn refresh_list_version_is_the_target_and_installed_version_stays_raw() {
        let mut report = ScanReport::ok(vec![pending_package("157.0.1-1")]);

        apply_update_result(
            &mut report,
            Ok("Name Version Rev Tracking Publisher Notes\nfirefox 158.0.1-1 9113 latest/stable canonical -\n".into()),
        );

        assert_eq!(report.packages[0].version, "157.0.1-1");
        assert_eq!(
            report.packages[0].update_version.as_deref(),
            Some("158.0.1-1")
        );
        assert!(report.packages[0].has_update);
    }

    #[test]
    fn refresh_revision_update_stays_marked_when_release_version_is_unchanged() {
        let mut report = ScanReport::ok(vec![pending_package("1.0.0")]);

        apply_update_result(
            &mut report,
            Ok("Name Version Rev Tracking Publisher Notes\nfirefox 1.0.0 42 latest/stable publisher -\n".into()),
        );

        assert!(report.packages[0].has_update);
        assert_eq!(report.packages[0].version, "1.0.0");
        assert_eq!(report.packages[0].update_version.as_deref(), Some("1.0.0"));
    }
}
