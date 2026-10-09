//! Read-only Steam library scanner.
//!
//! Steam keeps installed-library manifests in text VDF files and its app type
//! metadata in the binary `appinfo.vdf` cache. Scope only reads those files;
//! it never invokes Steam or changes its state.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::future::Future;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;

use anyhow::Result;
use steam_vdf_parser::{parse_appinfo, parse_text};

use crate::domain::package::{
    AppKind, InstalledPackage, PackageSource, SteamUpdateStatus,
};
use crate::domain::scanner::{ScanReport, Scanner};

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_LIBRARYFOLDERS_BYTES: u64 = 1024 * 1024;
const MAX_APPINFO_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFESTS_PER_LIBRARY: usize = 5_000;

const STATE_FLAG_UPDATE_REQUIRED: u32 = 1 << 1;
const STATE_FLAG_UPDATE_RUNNING: u32 = 1 << 8;
const STATE_FLAG_UPDATE_PAUSED: u32 = 1 << 9;
const STATE_FLAG_DOWNLOADING: u32 = 1 << 20;
const STATE_FLAG_STAGING: u32 = 1 << 21;
const STATE_FLAG_COMMITTING: u32 = 1 << 22;
const STATE_FLAG_ACTIVE_UPDATE: u32 = STATE_FLAG_UPDATE_RUNNING
    | STATE_FLAG_DOWNLOADING
    | STATE_FLAG_STAGING
    | STATE_FLAG_COMMITTING;

pub struct SteamScanner {
    roots: Vec<PathBuf>,
}

impl SteamScanner {
    pub fn new() -> Self {
        Self { roots: steam_roots() }
    }
}

impl Default for SteamScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for SteamScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Steam
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move {
            self.roots
                .iter()
                .any(|root| root.join("steamapps").is_dir())
        })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
        let roots = self.roots.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || scan_roots(&roots))
                .await
                .map_err(|error| anyhow::anyhow!("Steam scan task failed: {error}"))
        })
    }
}

/// Known Steam-installation paths before resolving aliases. Steam itself may
/// make several native paths symlink to the same installation.
fn known_roots(home: &Path, data_home: &Path) -> Vec<PathBuf> {
    let flatpak = home.join(".var/app/com.valvesoftware.Steam");
    vec![
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".steam/debian-installation"),
        data_home.join("Steam"),
        flatpak.join("data/Steam"),
        flatpak.join(".steam/steam"),
    ]
}

fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let data_home = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    available_roots(known_roots(&home, &data_home))
}

fn available_roots(candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.join("steamapps").is_dir())
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

fn scan_roots(roots: &[PathBuf]) -> ScanReport {
    let mut packages = HashMap::new();
    let mut warnings = Vec::new();

    for root in roots {
        let libraries = library_roots(root, &mut warnings);
        let manifests = libraries
            .iter()
            .flat_map(|library| manifests_in(library))
            .collect::<Vec<_>>();
        if manifests.is_empty() {
            continue;
        }

        let classifications = appinfo_classifications(root, &mut warnings);
        for manifest in manifests {
            let mut package = InstalledPackage::new(PackageSource::Steam, &manifest.app_id);
            package.name = manifest.name;
            package.version = manifest.build_id;
            package.size_bytes = manifest.size_bytes;
            package.app_kind = classifications
                .get(&manifest.app_id)
                .copied()
                .unwrap_or(AppKind::Unknown);
            package.has_update = manifest.has_update;
            package.steam_update_status = Some(manifest.steam_update_status);

            // Roots and library folders retain Steam's configured order. A
            // duplicate AppID keeps the first manifest's update state; a later
            // copy may safely provide type metadata missing from it.
            packages
                .entry(manifest.app_id)
                .and_modify(|existing: &mut InstalledPackage| {
                    if existing.app_kind == AppKind::Unknown && package.app_kind != AppKind::Unknown {
                        existing.app_kind = package.app_kind;
                    }
                })
                .or_insert(package);
        }
    }

    let mut packages = packages.into_values().collect::<Vec<_>>();
    packages.sort_by(|a, b| a.package_id.cmp(&b.package_id));
    warnings.sort();
    warnings.dedup();
    ScanReport {
        packages,
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
    }
}

fn library_roots(root: &Path, warnings: &mut Vec<String>) -> Vec<PathBuf> {
    let mut libraries = Vec::new();
    let mut seen = HashSet::new();
    insert_library(root.to_path_buf(), &mut libraries, &mut seen);

    let libraryfolders = root.join("steamapps/libraryfolders.vdf");
    if !libraryfolders.is_file() {
        return libraries;
    }
    let text = match read_bounded(&libraryfolders, MAX_LIBRARYFOLDERS_BYTES) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ()),
        Err(_) => Err(()),
    };
    let Ok(text) = text else {
        warnings.push(format!(
            "Steam library metadata could not be read from {}",
            root.display()
        ));
        return libraries;
    };
    let Ok(vdf) = parse_text(&text) else {
        warnings.push(format!(
            "Steam library metadata could not be parsed from {}",
            root.display()
        ));
        return libraries;
    };
    let Some(entries) = vdf.as_obj() else {
        warnings.push(format!(
            "Steam library metadata was invalid in {}",
            root.display()
        ));
        return libraries;
    };

    for (_, entry) in entries.iter() {
        let Some(path) = entry.get_str(&["path"]) else {
            continue;
        };
        insert_library(PathBuf::from(path), &mut libraries, &mut seen);
    }
    libraries
}

fn insert_library(path: PathBuf, libraries: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    let Ok(path) = path.canonicalize() else {
        return;
    };
    if path.join("steamapps").is_dir() && seen.insert(path.clone()) {
        libraries.push(path);
    }
}

fn manifests_in(library: &Path) -> Vec<Manifest> {
    let steamapps = library.join("steamapps");
    let Ok(entries) = fs::read_dir(&steamapps) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let app_id = name
                .strip_prefix("appmanifest_")?
                .strip_suffix(".acf")?;
            let app_id = canonical_app_id(app_id)?.to_owned();
            Some((path, app_id))
        })
        .take(MAX_MANIFESTS_PER_LIBRARY)
        .filter_map(|(path, app_id)| parse_manifest(&path, &app_id, library))
        .collect()
}

fn parse_manifest(path: &Path, file_app_id: &str, library: &Path) -> Option<Manifest> {
    let bytes = read_bounded(path, MAX_MANIFEST_BYTES).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let vdf = parse_text(text).ok()?;
    let app_id = vdf.get_str(&["appid"]).and_then(canonical_app_id)?;
    if app_id != file_app_id {
        return None;
    }
    let install_dir = vdf.get_str(&["installdir"])?;
    if !safe_install_dir(install_dir)
        || !library
            .join("steamapps/common")
            .join(install_dir)
            .is_dir()
    {
        // Steam can retain manifests after uninstall. Directory evidence also
        // retains an installed game during an interrupted pending update when
        // Steam temporarily changes StateFlags.
        return None;
    }
    let name = vdf.get_str(&["name"])?;
    if name.is_empty() {
        return None;
    }

    let size_bytes = vdf
        .get_str(&["SizeOnDisk"])
        .and_then(|size| size.parse().ok())
        .unwrap_or(0);
    let build_id = vdf
        .get_str(&["buildid"])
        .map(str::to_owned)
        .unwrap_or_default();
    let (has_update, steam_update_status) = steam_update_status(vdf.get_str(&["StateFlags"]));

    Some(Manifest {
        app_id: app_id.to_owned(),
        name: name.to_owned(),
        build_id,
        size_bytes,
        has_update,
        steam_update_status,
    })
}

fn steam_update_status(raw_state_flags: Option<&str>) -> (bool, SteamUpdateStatus) {
    let Some(raw_state_flags) = raw_state_flags else {
        return (false, SteamUpdateStatus::Unknown);
    };
    if raw_state_flags.is_empty()
        || !raw_state_flags
            .as_bytes()
            .iter()
            .all(u8::is_ascii_digit)
    {
        return (false, SteamUpdateStatus::Unknown);
    }
    let Ok(state_flags) = raw_state_flags.parse::<u32>() else {
        return (false, SteamUpdateStatus::Unknown);
    };
    if state_flags == 0 {
        return (false, SteamUpdateStatus::Unknown);
    }
    if state_flags & STATE_FLAG_UPDATE_REQUIRED == 0 {
        return (false, SteamUpdateStatus::NoUpdateRecorded);
    }
    if state_flags & STATE_FLAG_UPDATE_PAUSED != 0 {
        return (true, SteamUpdateStatus::Paused);
    }
    if state_flags & STATE_FLAG_ACTIVE_UPDATE != 0 {
        return (true, SteamUpdateStatus::InProgress);
    }
    (true, SteamUpdateStatus::Pending)
}

fn appinfo_classifications(root: &Path, warnings: &mut Vec<String>) -> HashMap<String, AppKind> {
    let path = root.join("appcache/appinfo.vdf");
    let bytes = match read_bounded(&path, MAX_APPINFO_BYTES) {
        Ok(bytes) => bytes,
        Err(_) => {
            warnings.push(format!(
                "Steam app metadata could not be read from {}",
                root.display()
            ));
            return HashMap::new();
        }
    };
    let vdf = match parse_appinfo(&bytes) {
        Ok(vdf) => vdf,
        Err(_) => {
            warnings.push(format!(
                "Steam app metadata could not be parsed from {}",
                root.display()
            ));
            return HashMap::new();
        }
    };
    let Some(apps) = vdf.as_obj() else {
        return HashMap::new();
    };

    apps
        .iter()
        .filter_map(|(app_id, app)| {
            let app_id = canonical_app_id(app_id)?;
            let app_type = app.get_str(&["appinfo", "common", "type"])?;
            Some((app_id.to_owned(), app_kind_from_type(app_type)))
        })
        .collect()
}

fn app_kind_from_type(app_type: &str) -> AppKind {
    let app_type = app_type.trim();
    if app_type.eq_ignore_ascii_case("game") || app_type.eq_ignore_ascii_case("demo") {
        AppKind::Game
    } else if app_type.eq_ignore_ascii_case("application") {
        AppKind::Gui
    } else {
        AppKind::Unknown
    }
}

fn canonical_app_id(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 10
        || (bytes.len() > 1 && bytes[0] == b'0')
        || !bytes.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    value.parse::<u32>().ok().filter(|id| *id != 0)?;
    Some(value)
}

fn safe_install_dir(value: &str) -> bool {
    !value.is_empty()
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn read_bounded(path: &Path, maximum: u64) -> std::io::Result<Vec<u8>> {
    let file = File::open(path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Steam metadata is not a regular file",
        ));
    }

    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Steam metadata file is too large",
        ));
    }
    Ok(bytes)
}

struct Manifest {
    app_id: String,
    name: String,
    build_id: String,
    size_bytes: u64,
    has_update: bool,
    steam_update_status: SteamUpdateStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(app_id: u32, name: &str, install_dir: &str, state_flags: u32) -> String {
        format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\"{app_id}\"\n\t\"name\"\t\"{name}\"\n\t\"StateFlags\"\t\"{state_flags}\"\n\t\"installdir\"\t\"{install_dir}\"\n\t\"SizeOnDisk\"\t\"42\"\n\t\"buildid\"\t\"99\"\n}}\n"
        )
    }

    fn appinfo(entries: &[(u32, &str)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x0756_4428u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        for (app_id, app_type) in entries {
            let mut vdf = Vec::new();
            vdf.extend_from_slice(b"\0appinfo\0\0common\0\x01type\0");
            vdf.extend_from_slice(app_type.as_bytes());
            vdf.extend_from_slice(b"\0\x08\x08");

            bytes.extend_from_slice(&app_id.to_le_bytes());
            bytes.extend_from_slice(&(60u32 + vdf.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&[0; 60]);
            bytes.extend_from_slice(&vdf);
        }
        bytes.extend_from_slice(&[0; 68]);
        bytes
    }

    fn root() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("steamapps/common")).unwrap();
        root
    }

    fn manifest_with_state_flags(
        app_id: u32,
        name: &str,
        install_dir: &str,
        state_flags: Option<&str>,
    ) -> String {
        let state_flags = state_flags.map_or_else(String::new, |value| {
            format!("\t\"StateFlags\"\t\"{value}\"\n")
        });
        format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\"{app_id}\"\n\t\"name\"\t\"{name}\"\n{state_flags}\t\"installdir\"\t\"{install_dir}\"\n\t\"SizeOnDisk\"\t\"42\"\n\t\"buildid\"\t\"99\"\n}}\n"
        )
    }

    fn scanned_package_with_state_flags(state_flags: Option<&str>) -> InstalledPackage {
        let root = root();
        fs::create_dir_all(root.path().join("steamapps/common/State")).unwrap();
        fs::write(
            root.path().join("steamapps/appmanifest_42.acf"),
            manifest_with_state_flags(42, "State", "State", state_flags),
        )
        .unwrap();

        let mut packages = scan_roots(&[root.path().to_path_buf()]).packages;
        assert_eq!(packages.len(), 1);
        packages.pop().unwrap()
    }

    #[test]
    fn pending_flags_mark_a_directory_backed_game_as_updated() {
        let package = scanned_package_with_state_flags(Some("6"));

        assert!(package.has_update);
        assert_eq!(
            package.steam_update_status,
            Some(SteamUpdateStatus::Pending)
        );
        assert_eq!(package.update_version, None);
    }

    #[test]
    fn paused_flags_mark_a_directory_backed_game_as_paused() {
        let package = scanned_package_with_state_flags(Some("518"));

        assert!(package.has_update);
        assert_eq!(
            package.steam_update_status,
            Some(SteamUpdateStatus::Paused)
        );
    }

    #[test]
    fn paused_flags_win_over_active_update_flags() {
        let flags = (STATE_FLAG_UPDATE_REQUIRED | STATE_FLAG_UPDATE_PAUSED | STATE_FLAG_DOWNLOADING)
            .to_string();
        let package = scanned_package_with_state_flags(Some(&flags));

        assert!(package.has_update);
        assert_eq!(
            package.steam_update_status,
            Some(SteamUpdateStatus::Paused)
        );
    }

    #[test]
    fn active_update_flags_mark_an_update_in_progress() {
        for (flags, label) in [
            (
                STATE_FLAG_UPDATE_REQUIRED | STATE_FLAG_UPDATE_RUNNING,
                "running",
            ),
            (
                STATE_FLAG_UPDATE_REQUIRED | STATE_FLAG_DOWNLOADING,
                "downloading",
            ),
            (STATE_FLAG_UPDATE_REQUIRED | STATE_FLAG_STAGING, "staging"),
            (
                STATE_FLAG_UPDATE_REQUIRED | STATE_FLAG_COMMITTING,
                "committing",
            ),
        ] {
            let flags = flags.to_string();
            let package = scanned_package_with_state_flags(Some(&flags));

            assert!(package.has_update, "{label}");
            assert_eq!(
                package.steam_update_status,
                Some(SteamUpdateStatus::InProgress),
                "{label}"
            );
        }
    }

    #[test]
    fn update_started_without_an_active_flag_stays_pending() {
        let flags = (STATE_FLAG_UPDATE_REQUIRED | (1 << 10)).to_string();
        let package = scanned_package_with_state_flags(Some(&flags));

        assert!(package.has_update);
        assert_eq!(
            package.steam_update_status,
            Some(SteamUpdateStatus::Pending)
        );
    }

    #[test]
    fn valid_non_update_flags_do_not_claim_an_update() {
        for (state_flags, label) in [("4", "fully installed"), ("131072", "validating")] {
            let package = scanned_package_with_state_flags(Some(state_flags));

            assert!(!package.has_update, "{label}");
            assert_eq!(
                package.steam_update_status,
                Some(SteamUpdateStatus::NoUpdateRecorded),
                "{label}"
            );
        }
    }

    #[test]
    fn missing_or_invalid_flags_are_unknown_and_not_updates() {
        for state_flags in [None, Some("not-a-number"), Some("4294967296"), Some("0")] {
            let package = scanned_package_with_state_flags(state_flags);

            assert!(!package.has_update);
            assert_eq!(
                package.steam_update_status,
                Some(SteamUpdateStatus::Unknown)
            );
        }
    }

    #[test]
    fn a_rescan_clears_a_previously_pending_update() {
        let root = root();
        fs::create_dir_all(root.path().join("steamapps/common/State")).unwrap();
        let manifest_path = root.path().join("steamapps/appmanifest_42.acf");
        fs::write(&manifest_path, manifest(42, "State", "State", 6)).unwrap();
        let first = scan_roots(&[root.path().to_path_buf()])
            .packages
            .pop()
            .unwrap();
        assert!(first.has_update);
        assert_eq!(
            first.steam_update_status,
            Some(SteamUpdateStatus::Pending)
        );

        fs::write(&manifest_path, manifest(42, "State", "State", 4)).unwrap();
        let second = scan_roots(&[root.path().to_path_buf()])
            .packages
            .pop()
            .unwrap();
        assert!(!second.has_update);
        assert_eq!(
            second.steam_update_status,
            Some(SteamUpdateStatus::NoUpdateRecorded)
        );
    }

    #[test]
    fn duplicate_manifests_keep_the_first_update_state() {
        let first_root = root();
        let second_root = root();
        for root in [&first_root, &second_root] {
            fs::create_dir_all(root.path().join("steamapps/common/State")).unwrap();
        }
        fs::write(
            first_root.path().join("steamapps/appmanifest_42.acf"),
            manifest(42, "First", "State", 6),
        )
        .unwrap();
        fs::write(
            second_root.path().join("steamapps/appmanifest_42.acf"),
            manifest(42, "Second", "State", 4),
        )
        .unwrap();

        let report = scan_roots(&[
            first_root.path().to_path_buf(),
            second_root.path().to_path_buf(),
        ]);
        assert_eq!(report.packages.len(), 1);
        let package = &report.packages[0];
        assert_eq!(package.name, "First");
        assert!(package.has_update);
        assert_eq!(
            package.steam_update_status,
            Some(SteamUpdateStatus::Pending)
        );
    }

    #[test]
    fn discovers_libraryfolders_and_rejects_stale_manifests() {
        let root = root();
        let library = root.path().join("library");
        fs::create_dir_all(library.join("steamapps/common/Installed")).unwrap();
        fs::create_dir_all(library.join("steamapps")).unwrap();
        fs::write(library.join("steamapps/appmanifest_44.acf"), "not VDF").unwrap();
        fs::write(
            root.path().join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"1\" {{ \"path\" \"{}\" }}\n}}",
                library.display()
            ),
        )
        .unwrap();
        fs::write(
            library.join("steamapps/appmanifest_42.acf"),
            manifest(42, "Installed", "Installed", 4),
        )
        .unwrap();
        fs::write(
            library.join("steamapps/appmanifest_43.acf"),
            manifest(43, "Stale", "Missing", 4),
        )
        .unwrap();
        fs::create_dir_all(root.path().join("appcache")).unwrap();
        fs::write(root.path().join("appcache/appinfo.vdf"), appinfo(&[(42, "game")])).unwrap();

        let report = scan_roots(&[root.path().to_path_buf()]);
        assert_eq!(report.packages.len(), 1);
        assert_eq!(report.packages[0].package_id, "42");
        assert_eq!(report.packages[0].app_kind, AppKind::Game);
    }

    #[test]
    fn keeps_directory_backed_game_during_pending_update() {
        let root = root();
        fs::create_dir_all(root.path().join("steamapps/common/Pending")).unwrap();
        fs::write(
            root.path().join("steamapps/appmanifest_1062520.acf"),
            manifest(1_062_520, "Pending", "Pending", 1024),
        )
        .unwrap();
        fs::create_dir_all(root.path().join("appcache")).unwrap();
        fs::write(
            root.path().join("appcache/appinfo.vdf"),
            appinfo(&[(1_062_520, "game")]),
        )
        .unwrap();

        let report = scan_roots(&[root.path().to_path_buf()]);
        assert_eq!(report.packages[0].app_kind, AppKind::Game);
        assert_eq!(report.packages[0].version, "99");
    }

    #[test]
    fn retains_directory_backed_manifest_as_unknown_without_appinfo() {
        let root = root();
        fs::create_dir_all(root.path().join("steamapps/common/Unknown")).unwrap();
        fs::write(
            root.path().join("steamapps/appmanifest_3405340.acf"),
            manifest(3_405_340, "Unknown", "Unknown", 4),
        )
        .unwrap();

        let report = scan_roots(&[root.path().to_path_buf()]);
        assert_eq!(report.packages.len(), 1);
        assert_eq!(report.packages[0].app_kind, AppKind::Unknown);
        assert!(report.warning.is_some());
    }

    #[test]
    fn classifies_binary_appinfo_by_common_type_only() {
        let root = root();
        fs::create_dir_all(root.path().join("appcache")).unwrap();
        fs::write(
            root.path().join("appcache/appinfo.vdf"),
            appinfo(&[(10, "game"), (11, "demo"), (12, "application"), (13, "tool")]),
        )
        .unwrap();

        let kinds = appinfo_classifications(root.path(), &mut Vec::new());
        assert_eq!(kinds.get("10"), Some(&AppKind::Game));
        assert_eq!(kinds.get("11"), Some(&AppKind::Game));
        assert_eq!(kinds.get("12"), Some(&AppKind::Gui));
        assert_eq!(kinds.get("13"), Some(&AppKind::Unknown));
    }

    #[test]
    fn discovers_flatpak_root_and_deduplicates_native_alias() {
        let home = tempfile::tempdir().unwrap();
        let flatpak_root = home
            .path()
            .join(".var/app/com.valvesoftware.Steam/data/Steam");
        fs::create_dir_all(flatpak_root.join("steamapps/common/Flatpak Game")).unwrap();
        fs::create_dir_all(flatpak_root.join("appcache")).unwrap();
        fs::write(
            flatpak_root.join("steamapps/appmanifest_42.acf"),
            manifest(42, "Flatpak Game", "Flatpak Game", 4),
        )
        .unwrap();
        fs::write(
            flatpak_root.join("appcache/appinfo.vdf"),
            appinfo(&[(42, "game")]),
        )
        .unwrap();

        let native_alias = home.path().join(".steam/steam");
        fs::create_dir_all(native_alias.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&flatpak_root, &native_alias).unwrap();

        let roots = available_roots(known_roots(home.path(), &home.path().join("data")));
        assert_eq!(roots.len(), 1);
        let report = scan_roots(&roots);
        assert_eq!(report.packages.len(), 1);
        assert_eq!(report.packages[0].package_id, "42");
        assert_eq!(report.packages[0].app_kind, AppKind::Game);
    }

    #[test]
    fn canonical_app_ids_and_install_dirs_are_strict() {
        assert_eq!(canonical_app_id("42"), Some("42"));
        assert_eq!(canonical_app_id("0042"), None);
        assert_eq!(canonical_app_id("0"), None);
        assert!(safe_install_dir("A Game"));
        assert!(!safe_install_dir("../outside"));
        assert!(!safe_install_dir("/absolute"));
    }
}
