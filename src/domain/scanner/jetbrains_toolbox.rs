//! Read-only JetBrains Toolbox scanner.
//!
//! Toolbox records its installed IDEs below its data directory.  We inspect
//! only the small metadata files around those installations; this scanner never
//! starts Toolbox or an IDE, and never traverses an IDE's bundled contents.

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::future::Future;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::domain::package::{AppKind, InstalledPackage, PackageSource};
use crate::domain::scanner::{ScanReport, Scanner};

const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const MAX_ROOTS: usize = 16;
const MAX_APP_PRODUCTS: usize = 256;
const MAX_CHANNELS_PER_PRODUCT: usize = 64;
const MAX_BUILDS_PER_CHANNEL: usize = 128;
const MAX_CHANNEL_FILES: usize = 256;

/// Discovers IDEs managed by a local JetBrains Toolbox installation.
pub struct JetBrainsToolboxScanner {
    roots: Vec<PathBuf>,
}

impl JetBrainsToolboxScanner {
    pub fn new() -> Self {
        Self {
            roots: toolbox_root_candidates(),
        }
    }
}

impl Default for JetBrainsToolboxScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for JetBrainsToolboxScanner {
    fn source(&self) -> PackageSource {
        PackageSource::JetBrainsToolbox
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let roots = self.roots.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || !available_roots(roots).is_empty())
                .await
                .unwrap_or(false)
        })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
        let roots = self.roots.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || scan_roots(roots))
                .await
                .map_err(|error| anyhow!("JetBrains Toolbox scan task failed: {error}"))
        })
    }
}

/// Derive paths from the process environment without touching the filesystem.
/// A relative `XDG_DATA_HOME` is intentionally ignored: XDG data homes must be
/// absolute, and accepting one would make the source depend on the CWD.
fn toolbox_root_candidates() -> Vec<PathBuf> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };

    let fallback = home.join(".local/share/JetBrains/Toolbox");
    let mut candidates = vec![fallback];
    if let Some(data_home) = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        candidates.push(data_home.join("JetBrains/Toolbox"));
    }
    candidates
}

/// Canonicalize existing roots so the normal XDG fallback and a symlinked XDG
/// path cannot produce duplicate IDE rows.
fn available_roots(candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .take(MAX_ROOTS)
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.is_dir())
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn scan_roots(candidates: Vec<PathBuf>) -> ScanReport {
    let mut packages = BTreeMap::new();
    let mut warnings = Vec::new();

    for root in available_roots(candidates) {
        let mut installations = Vec::new();
        let mut seen_installations = HashSet::new();

        collect_apps_root(
            &root.join("apps"),
            &mut installations,
            &mut seen_installations,
            &mut warnings,
        );
        if let Some(legacy_apps) = legacy_apps_root(&root, &mut warnings) {
            collect_apps_root(
                &legacy_apps,
                &mut installations,
                &mut seen_installations,
                &mut warnings,
            );
        }
        collect_channel_installations(
            &root,
            &mut installations,
            &mut seen_installations,
            &mut warnings,
        );

        for installation in installations {
            match package_from_installation(&installation) {
                Ok(package) => {
                    packages
                        .entry(package.package_id.clone())
                        .or_insert(package);
                }
                Err(error) => warnings.push(format!(
                    "JetBrains Toolbox installation at {} was skipped: {error}",
                    installation.display()
                )),
            }
        }
    }

    let mut packages = packages.into_values().collect::<Vec<_>>();
    packages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.package_id.cmp(&right.package_id))
    });
    warnings.sort();
    warnings.dedup();
    ScanReport {
        packages,
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
    }
}

/// Inspect direct modern installations and the legacy
/// `<product>/ch-*/<build>` layout. This deliberately uses fixed directory
/// levels instead of walking recursively into IDE installations.
fn collect_apps_root(
    apps_root: &Path,
    installations: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let Ok(apps_root) = apps_root.canonicalize() else {
        return;
    };
    if !apps_root.is_dir() {
        return;
    }

    for product in bounded_dirs(&apps_root, MAX_APP_PRODUCTS, warnings) {
        let Ok(product) = product.canonicalize() else {
            continue;
        };
        if !is_within(&product, &apps_root) {
            warnings.push(format!(
                "JetBrains Toolbox apps entry escaped {}",
                apps_root.display()
            ));
            continue;
        }

        let has_product_info = product.join("product-info.json").is_file();
        if has_product_info {
            let _ = insert_if_installation(&product, installations, seen);
        }
        let mut saw_legacy_channel = false;
        for channel in bounded_dirs(&product, MAX_CHANNELS_PER_PRODUCT, warnings) {
            let is_channel = channel
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("ch-"));
            if !is_channel {
                continue;
            }
            saw_legacy_channel = true;
            let Ok(channel) = channel.canonicalize() else {
                continue;
            };
            if !is_within(&channel, &product) {
                warnings.push(format!(
                    "JetBrains Toolbox legacy channel escaped {}",
                    product.display()
                ));
                continue;
            }
            for build in bounded_dirs(&channel, MAX_BUILDS_PER_CHANNEL, warnings) {
                let Ok(build) = build.canonicalize() else {
                    continue;
                };
                if !is_within(&build, &channel) {
                    warnings.push(format!(
                        "JetBrains Toolbox legacy build escaped {}",
                        channel.display()
                    ));
                    continue;
                }
                if build.join("product-info.json").is_file() {
                    let _ = insert_if_installation(&build, installations, seen);
                } else {
                    warnings.push(format!(
                        "JetBrains Toolbox legacy build at {} had no product-info.json",
                        build.display()
                    ));
                }
            }
        }
        if !has_product_info && !saw_legacy_channel {
            warnings.push(format!(
                "JetBrains Toolbox app entry at {} had no product-info.json",
                product.display()
            ));
        }
    }
}

fn insert_if_installation(
    path: &Path,
    installations: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> bool {
    if path.join("product-info.json").is_file() && seen.insert(path.to_path_buf()) {
        installations.push(path.to_path_buf());
        true
    } else {
        false
    }
}

/// Older Toolbox settings point at the Toolbox base, whose `apps` child holds
/// the legacy `<product>/ch-*/<build>` installation layout.
fn legacy_apps_root(root: &Path, warnings: &mut Vec<String>) -> Option<PathBuf> {
    let settings = root.join(".settings.json");
    if !settings.is_file() {
        return None;
    }
    let value: serde_json::Value = match read_json(&settings) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!(
                "JetBrains Toolbox settings at {} could not be read: {error}",
                settings.display()
            ));
            return None;
        }
    };
    let path = value
        .get("install_location")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)?;
    if !path.is_absolute() {
        warnings.push(format!(
            "JetBrains Toolbox settings at {} contained a relative install location",
            settings.display()
        ));
        return None;
    }
    let base = match path.canonicalize() {
        Ok(path) if path.is_dir() => path,
        _ => {
            warnings.push(format!(
                "JetBrains Toolbox settings at {} referenced an unavailable install location",
                settings.display()
            ));
            return None;
        }
    };
    match base.join("apps").canonicalize() {
        Ok(apps) if apps.is_dir() => Some(apps),
        _ => {
            warnings.push(format!(
                "JetBrains Toolbox settings at {} referenced an unavailable apps directory",
                settings.display()
            ));
            None
        }
    }
}

/// Channels are the authoritative record for modern custom installation roots.
fn collect_channel_installations(
    root: &Path,
    installations: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let channels = root.join("channels");
    for entry in bounded_entries(&channels, MAX_CHANNEL_FILES, warnings) {
        let channel_file = entry.path();
        if channel_file.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let value: serde_json::Value = match read_json(&channel_file) {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!(
                    "JetBrains Toolbox channel metadata at {} could not be read: {error}",
                    channel_file.display()
                ));
                continue;
            }
        };
        let Some(installation) = value
            .get("channel")
            .and_then(|channel| channel.get("installationDirectory"))
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
        else {
            warnings.push(format!(
                "JetBrains Toolbox channel metadata at {} had no installation directory",
                channel_file.display()
            ));
            continue;
        };
        if !installation.is_absolute() {
            warnings.push(format!(
                "JetBrains Toolbox channel metadata at {} had a relative installation directory",
                channel_file.display()
            ));
            continue;
        }
        match installation.canonicalize() {
            Ok(path) if path.is_dir() && path.join("product-info.json").is_file() => {
                let _ = insert_if_installation(&path, installations, seen);
            }
            Ok(path) if path.is_dir() => warnings.push(format!(
                "JetBrains Toolbox channel metadata at {} referenced an installation without product-info.json",
                channel_file.display()
            )),
            _ => warnings.push(format!(
                "JetBrains Toolbox channel metadata at {} referenced an unavailable installation",
                channel_file.display()
            )),
        }
    }
}

fn bounded_dirs(root: &Path, limit: usize, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    bounded_entries(root, limit, warnings)
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

fn bounded_entries(root: &Path, limit: usize, warnings: &mut Vec<String>) -> Vec<fs::DirEntry> {
    let mut entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            if root.exists() {
                warnings.push(format!(
                    "JetBrains Toolbox could not read {}: {error}",
                    root.display()
                ));
            }
            return Vec::new();
        }
    };
    let mut bounded = Vec::new();
    for _ in 0..limit {
        match entries.next() {
            Some(Ok(entry)) => bounded.push(entry),
            Some(Err(error)) => warnings.push(format!(
                "JetBrains Toolbox could not read an entry in {}: {error}",
                root.display()
            )),
            None => return bounded,
        }
    }
    if entries.next().is_some() {
        warnings.push(format!(
            "JetBrains Toolbox scan of {} was limited to {limit} entries",
            root.display()
        ));
    }
    bounded
}

fn package_from_installation(root: &Path) -> Result<InstalledPackage> {
    let root = root
        .canonicalize()
        .map_err(|error| anyhow!("could not canonicalize root: {error}"))?;
    if !root.is_dir() {
        return Err(anyhow!("installation root is not a directory"));
    }
    let metadata: ProductInfo = serde_json::from_value(read_json(&root.join("product-info.json"))?)
        .map_err(|error| anyhow!("invalid product-info.json: {error}"))?;

    let name = required_field(&metadata.name, "name")?;
    let release = required_field(&metadata.version, "version")?;
    let build = required_field(&metadata.build_number, "buildNumber")?;
    let mut launcher_error = None;
    let mut saw_linux_launcher = false;
    for launcher in metadata
        .launch
        .iter()
        .filter(|launch| launch.os.eq_ignore_ascii_case("linux"))
    {
        saw_linux_launcher = true;
        match verified_relative_file(&root, Path::new(&launcher.launcher_path), "launcherPath") {
            Ok(_) => {
                launcher_error = None;
                break;
            }
            Err(error) if launcher_error.is_none() => launcher_error = Some(error),
            Err(_) => {}
        }
    }
    if !saw_linux_launcher {
        return Err(anyhow!("metadata has no Linux launcher"));
    }
    if let Some(error) = launcher_error {
        return Err(error);
    }

    let mut package = InstalledPackage::new(
        PackageSource::JetBrainsToolbox,
        root.to_string_lossy().to_string(),
    );
    package.name = name.to_owned();
    package.version = format!("{release} ({build})");
    package.display_version = Some(release.to_owned());
    package.app_kind = AppKind::Gui;
    package.update_note = Some("Manage updates and uninstall in JetBrains Toolbox.".to_string());
    if let Some(icon_path) = metadata.svg_icon_path.as_deref() {
        if let Ok(icon) = verified_relative_file(&root, Path::new(icon_path), "svgIconPath") {
            package.icon = Some(crate::domain::icons::icon_url(&icon));
        }
    }
    Ok(package)
}

fn required_field<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    (!value.is_empty())
        .then_some(value)
        .ok_or_else(|| anyhow!("metadata field {field} was empty"))
}

/// Resolve only a simple relative metadata path, and verify that canonical
/// resolution remains under the exact installation root.
fn verified_relative_file(root: &Path, path: &Path, field: &str) -> Result<PathBuf> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(anyhow!(
            "metadata field {field} was not a safe relative path"
        ));
    }
    let resolved = root
        .join(path)
        .canonicalize()
        .map_err(|_| anyhow!("metadata field {field} did not name an existing file"))?;
    if !is_within(&resolved, root) || !resolved.is_file() {
        return Err(anyhow!(
            "metadata field {field} did not name a file inside the installation"
        ));
    }
    Ok(resolved)
}

fn is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root) && path != root
}

fn read_json(path: &Path) -> Result<serde_json::Value> {
    let bytes = read_bounded(path, MAX_METADATA_BYTES)?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    // Check before opening so named pipes are rejected without blocking. Check
    // the opened file again because the path can be replaced between the two
    // operations.
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(anyhow!("path was not a regular file"));
    }
    if metadata.len() > limit {
        return Err(anyhow!("file exceeded {limit}-byte limit"));
    }

    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(anyhow!("path was not a regular file"));
    }
    if metadata.len() > limit {
        return Err(anyhow!("file exceeded {limit}-byte limit"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    let mut bounded_file = file.take(
        limit
            .checked_add(1)
            .ok_or_else(|| anyhow!("file size limit overflowed"))?,
    );
    bounded_file.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(anyhow!("file exceeded {limit}-byte limit"));
    }
    Ok(bytes)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductInfo {
    name: String,
    version: String,
    build_number: String,
    #[serde(default)]
    svg_icon_path: Option<String>,
    #[serde(default)]
    launch: Vec<Launch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Launch {
    os: String,
    launcher_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product_info(name: &str, version: &str, build: &str, launch: &str, icon: &str) -> String {
        format!(
            r#"{{"name":"{name}","version":"{version}","buildNumber":"{build}","svgIconPath":"{icon}","launch":[{{"os":"Linux","arch":"amd64","launcherPath":"{launch}"}}]}}"#
        )
    }

    fn write_install(root: &Path, name: &str, version: &str, build: &str) {
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin/idea"), "").unwrap();
        fs::write(root.join("bin/idea.svg"), "<svg/>").unwrap();
        fs::write(
            root.join("product-info.json"),
            product_info(name, version, build, "bin/idea", "bin/idea.svg"),
        )
        .unwrap();
    }

    #[test]
    fn discovers_modern_legacy_and_custom_installations() {
        let toolbox = tempfile::tempdir().unwrap();
        let modern = toolbox.path().join("apps/pycharm");
        let legacy_toolbox = toolbox.path().join("legacy-toolbox");
        let legacy = legacy_toolbox.join("apps/rider/ch-0/241.1");
        let custom_parent = tempfile::tempdir().unwrap();
        let custom = custom_parent.path().join("DataGrip");
        write_install(&modern, "PyCharm", "2025.1", "251.1");
        write_install(&legacy, "Rider", "2024.3", "243.2");
        write_install(&custom, "DataGrip", "2025.2", "252.3");
        fs::write(
            toolbox.path().join(".settings.json"),
            format!(r#"{{"install_location":"{}"}}"#, legacy_toolbox.display()),
        )
        .unwrap();
        fs::create_dir_all(toolbox.path().join("channels")).unwrap();
        fs::write(
            toolbox.path().join("channels/datagrip.json"),
            format!(
                r#"{{"channel":{{"installationDirectory":"{}"}},"tool":{{"toolId":"datagrip"}}}}"#,
                custom.display()
            ),
        )
        .unwrap();
        fs::write(
            toolbox.path().join("channels/pycharm.json"),
            format!(
                r#"{{"channel":{{"installationDirectory":"{}"}},"tool":{{"toolId":"pycharm"}}}}"#,
                modern.display()
            ),
        )
        .unwrap();

        let report = scan_roots(vec![toolbox.path().to_path_buf()]);
        assert!(report.warning.is_none());
        assert_eq!(report.packages.len(), 3);
        assert_eq!(
            report
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            ["DataGrip", "PyCharm", "Rider"]
        );
        let pycharm = report
            .packages
            .iter()
            .find(|package| package.name == "PyCharm")
            .unwrap();
        assert_eq!(
            pycharm.package_id,
            modern.canonicalize().unwrap().display().to_string()
        );
        assert_eq!(pycharm.version, "2025.1 (251.1)");
        assert_eq!(pycharm.display_version.as_deref(), Some("2025.1"));
        assert_eq!(pycharm.app_kind, AppKind::Gui);
        assert!(pycharm.icon.is_some());
    }

    #[test]
    fn canonical_root_aliases_deduplicate_but_distinct_installs_remain() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        write_install(
            &first.path().join("apps/idea"),
            "IntelliJ IDEA",
            "2025.1",
            "251.1",
        );
        write_install(
            &second.path().join("apps/idea"),
            "IntelliJ IDEA",
            "2025.1",
            "251.1",
        );
        let alias_parent = tempfile::tempdir().unwrap();
        let alias = alias_parent.path().join("toolbox-alias");
        std::os::unix::fs::symlink(first.path(), &alias).unwrap();

        let report = scan_roots(vec![
            first.path().to_path_buf(),
            alias,
            second.path().to_path_buf(),
        ]);
        assert_eq!(report.packages.len(), 2);
        assert_ne!(report.packages[0].package_id, report.packages[1].package_id);
    }

    #[test]
    fn rejects_bad_metadata_without_discarding_valid_installations() {
        let toolbox = tempfile::tempdir().unwrap();
        let apps = toolbox.path().join("apps");
        write_install(&apps.join("good"), "Good IDE", "2025.1", "251.1");
        fs::create_dir_all(apps.join("missing")).unwrap();

        let stale = apps.join("stale");
        fs::create_dir_all(&stale).unwrap();
        fs::write(
            stale.join("product-info.json"),
            product_info("Stale", "2025.1", "251.1", "bin/missing", "bin/icon.svg"),
        )
        .unwrap();

        let non_linux = apps.join("non-linux");
        fs::create_dir_all(&non_linux).unwrap();
        fs::write(
            non_linux.join("product-info.json"),
            r#"{"name":"Windows IDE","version":"2025.1","buildNumber":"251.1","launch":[{"os":"Windows","launcherPath":"bin/idea.exe"}]}"#,
        )
        .unwrap();

        let malformed = apps.join("malformed");
        fs::create_dir_all(&malformed).unwrap();
        fs::write(malformed.join("product-info.json"), "{").unwrap();

        let traversal = apps.join("traversal");
        fs::create_dir_all(&traversal).unwrap();
        fs::write(
            traversal.join("product-info.json"),
            product_info("Traversal", "2025.1", "251.1", "../outside", "bin/icon.svg"),
        )
        .unwrap();

        let report = scan_roots(vec![toolbox.path().to_path_buf()]);
        assert_eq!(report.packages.len(), 1);
        assert_eq!(report.packages[0].name, "Good IDE");
        let warning = report.warning.unwrap();
        for rejected in [apps.join("missing"), stale, non_linux, malformed, traversal] {
            assert!(
                warning.contains(rejected.to_string_lossy().as_ref()),
                "missing warning for {}",
                rejected.display()
            );
        }
    }

    #[test]
    fn bounded_reads_warn_on_truncation_and_reject_non_regular_metadata() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("first"), "").unwrap();
        fs::write(root.path().join("second"), "").unwrap();

        let mut warnings = Vec::new();
        assert_eq!(bounded_entries(root.path(), 1, &mut warnings).len(), 1);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains(root.path().to_string_lossy().as_ref())));
        assert!(read_bounded(root.path(), MAX_METADATA_BYTES).is_err());
    }
}
