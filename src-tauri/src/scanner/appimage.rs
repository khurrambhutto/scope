//! AppImage scanner.
//!
//! Strategy: walk a small set of well-known user/system directories for files
//! whose extension is `.AppImage` *and* whose magic bytes match the AppImage
//! signature (`0x41 0x49` + type byte at offset 8, after the ELF magic). Name
//! and version are parsed from the filename heuristic; size comes from file
//! metadata.

use std::future::Future;
use std::pin::Pin;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use tokio::fs;
use walkdir::WalkDir;

use crate::package::{AppKind, InstalledPackage, PackageSource};
use crate::scanner::Scanner;

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

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<InstalledPackage>>> + Send + '_>> {
        let dirs = self.dirs.clone();
        Box::pin(async move { scan(dirs).await })
    }
}

/// Directories scanned for `.AppImage` files. Kept explicit and tight — Scope
/// never scans arbitrary paths from the frontend. Exposed for status reporting.
pub fn search_directories() -> Vec<String> {
    dirs().iter().map(|p| p.display().to_string()).collect()
}

fn dirs() -> Vec<PathBuf> {
    let mut out = vec![
        PathBuf::from("/opt"),
        PathBuf::from("/usr/local/bin"),
    ];
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
    let mut packages = Vec::new();
    for dir in dirs {
        // Walk runs on a blocking thread to avoid stalling the async runtime.
        let dir_clone = dir.clone();
        let candidates = tokio::task::spawn_blocking(move || {
            let mut out: Vec<PathBuf> = Vec::new();
            for entry in WalkDir::new(&dir_clone)
                .max_depth(3)
                .into_iter()
                .filter_entry(|e| !is_hidden(e.path()))
                .filter_map(Result::ok)
            {
                let p = entry.path();
                if p.is_file() && has_appimage_extension(p) {
                    out.push(p.to_path_buf());
                }
            }
            out
        })
        .await
        .unwrap_or_default();

        for path in candidates {
            // Validate magic bytes before reporting.
            if !is_appimage(&path).await {
                continue;
            }
            if let Some(pkg) = build_package(&path).await {
                packages.push(pkg);
            }
        }
    }
    Ok(packages)
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
async fn is_appimage(path: &Path) -> bool {
    use tokio::io::AsyncReadExt;
    let Ok(mut file) = fs::File::open(path).await else {
        return false;
    };
    let mut buf = [0u8; 11];
    if file.read_exact(&mut buf).await.is_err() {
        return false;
    }
    &buf[0..4] == b"\x7fELF" && buf[8] == 0x41 && buf[9] == 0x49 && (buf[10] == 0x01 || buf[10] == 0x02)
}

async fn build_package(path: &Path) -> Option<InstalledPackage> {
    let filename = path.file_name()?.to_string_lossy().to_string();
    let name = extract_name(&filename);
    let version = extract_version(&filename);

    let size_bytes = fs::metadata(path).await.map(|m| m.len()).unwrap_or(0);

    let mut pkg = InstalledPackage::new(PackageSource::AppImage, path.to_string_lossy().to_string());
    pkg.name = name.clone();
    pkg.display_name = Some(name);
    pkg.version = version;
    pkg.size_bytes = size_bytes;
    pkg.app_kind = AppKind::Gui; // AppImages are GUI bundles by definition
    Some(pkg)
}

/// Strip the `.AppImage` suffix and version/arch tags from a filename.
fn extract_name(filename: &str) -> String {
    let stem = trim_appimage_suffix(filename);
    // Remove trailing version-ish / arch / "x86_64" / "linux" from the end.
    let cleaned = regex::Regex::new(r"(?i)[-_]?(v?\d[\d.]*|x86_64|amd64|aarch64|arm64|linux).*$")
        .map(|re| re.replace(stem, "").to_string())
        .unwrap_or_else(|_| stem.to_string());
    let cleaned = cleaned.trim_end_matches(['-', '_', '.']).to_string();
    if cleaned.is_empty() {
        stem.to_string()
    } else {
        cleaned
    }
}

/// Pull a leading version-looking token out of the filename.
fn extract_version(filename: &str) -> String {
    let stem = trim_appimage_suffix(filename);
    let re = match regex::Regex::new(r"[_-]v?(\d+(?:\.\d+){1,3})") {
        Ok(re) => re,
        Err(_) => return "unknown".to_string(),
    };
    match re.captures(stem).and_then(|c| c.get(1)) {
        Some(m) => m.as_str().to_string(),
        None => "unknown".to_string(),
    }
}

fn trim_appimage_suffix(filename: &str) -> &str {
    filename.trim_end_matches(".AppImage").trim_end_matches(".appimage")
}

#[allow(dead_code)]
fn _unused() {
    let _ = Duration::from_secs(1);
}