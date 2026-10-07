//! AppImage scanner.
//!
//! Strategy: walk a small set of well-known user/system directories for files
//! whose extension is `.AppImage` *and* whose magic bytes match the AppImage
//! signature (`0x41 0x49` + type byte at offset 8, after the ELF magic). Name
//! and version are parsed from the filename heuristic; size comes from file
//! metadata.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use anyhow::Result;
use walkdir::WalkDir;

use crate::domain::package::{AppKind, InstalledPackage, PackageSource};
use crate::domain::scanner::{ScanReport, Scanner};

pub struct AppImageScanner {
    dirs: Vec<PathBuf>,
}

impl AppImageScanner {
    pub fn new() -> Self {
        Self { dirs: dirs() }
    }
}

impl Default for AppImageScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Scanner for AppImageScanner {
    fn source(&self) -> PackageSource {
        PackageSource::AppImage
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        // Always available: a dir walk with no matches simply yields nothing.
        Box::pin(async { true })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
        let dirs = self.dirs.clone();
        Box::pin(async move { scan(dirs).await.map(ScanReport::ok) })
    }
}

/// Directories scanned for `.AppImage` files. Kept explicit and tight — Scope
/// never scans arbitrary paths from the frontend. Exposed for status reporting.
pub fn search_directories() -> Vec<String> {
    dirs().iter().map(|p| p.display().to_string()).collect()
}

fn dirs() -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from("/opt"), PathBuf::from("/usr/local/bin")];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        out.push(home.join("Applications"));
        out.push(home.join("apps"));
        out.push(home.join("AppImages"));
        out.push(home.join("Downloads"));
        out.push(home.join(".local/bin"));
    }
    out.dedup();
    out
}

async fn scan(dirs: Vec<PathBuf>) -> Result<Vec<InstalledPackage>> {
    Ok(tokio::task::spawn_blocking(move || {
        let mut packages = Vec::new();
        for dir in dirs {
            for entry in WalkDir::new(&dir)
                .max_depth(3)
                .into_iter()
                .filter_entry(|e| !is_hidden(e.path()))
                .filter_map(Result::ok)
            {
                let path = entry.path();
                if path.is_file() && has_appimage_extension(path) && is_appimage(path) {
                    if let Some(package) = build_package(path) {
                        packages.push(package);
                    }
                }
            }
        }
        packages
    })
    .await
    .unwrap_or_default())
}

fn is_hidden(name: &Path) -> bool {
    name.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}

fn has_appimage_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("appimage"))
        .unwrap_or(false)
}

/// AppImage magic: ELF header + "AI" + type byte (1 or 2) at offset 8..11.
fn is_appimage(path: &Path) -> bool {
    use std::io::Read as _;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 11];
    if file.read_exact(&mut buf).is_err() {
        return false;
    }
    &buf[0..4] == b"\x7fELF"
        && buf[8] == 0x41
        && buf[9] == 0x49
        && (buf[10] == 0x01 || buf[10] == 0x02)
}

fn build_package(path: &Path) -> Option<InstalledPackage> {
    let filename = path.file_name()?.to_string_lossy().to_string();
    let name = extract_name(&filename);
    let version = extract_version(&filename);

    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let mut pkg =
        InstalledPackage::new(PackageSource::AppImage, path.to_string_lossy().to_string());
    pkg.name = name;
    pkg.version = version;
    pkg.size_bytes = size_bytes;
    pkg.app_kind = AppKind::Gui; // AppImages are GUI bundles by definition
    Some(pkg)
}

/// Strip the `.AppImage` suffix and version/arch tags from a filename.
fn extract_name(filename: &str) -> String {
    let stem = trim_appimage_suffix(filename);
    // Remove trailing version-ish / arch / "x86_64" / "linux" from the end.
    let cleaned = match name_tag_regex() {
        Some(re) => re.replace(stem, "").to_string(),
        None => stem.to_string(),
    };
    let cleaned = cleaned.trim_end_matches(['-', '_', '.']).to_string();
    if cleaned.is_empty() {
        stem.to_string()
    } else {
        cleaned
    }
}

/// Compiled once for the process; `None` if the pattern is somehow invalid.
fn name_tag_regex() -> Option<&'static regex::Regex> {
    static RE: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)[-_]?(v?\d[\d.]*|x86_64|amd64|aarch64|arm64|linux).*$").ok()
    })
    .as_ref()
}

/// Pull a leading version-looking token out of the filename.
fn extract_version(filename: &str) -> String {
    let stem = trim_appimage_suffix(filename);
    let Some(re) = version_regex() else {
        return "unknown".to_string();
    };
    match re.captures(stem).and_then(|c| c.get(1)) {
        Some(m) => m.as_str().to_string(),
        None => "unknown".to_string(),
    }
}

/// Compiled once for the process; `None` if the pattern is somehow invalid.
fn version_regex() -> Option<&'static regex::Regex> {
    static RE: std::sync::OnceLock<Option<regex::Regex>> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[_-]v?(\d+(?:\.\d+){1,3})").ok())
        .as_ref()
}

fn trim_appimage_suffix(filename: &str) -> &str {
    filename
        .trim_end_matches(".AppImage")
        .trim_end_matches(".appimage")
}
