//! Self-update check, download, and install for the GPUI shell.
//!
//! The old Tauri build used `tauri-plugin-updater` against `latest.json` with
//! minisign signatures. GPUI has no webview plugin host, so this module talks
//! directly to the GitHub Releases API over HTTPS (via `curl`, like every
//! other external command in `crate::domain::system`), picks the asset
//! matching this install kind, and installs it:
//!
//! * `.deb` → `pkexec dpkg -i <file>`
//! * `.rpm` → `pkexec rpm -U <file>`
//! * `.AppImage` → replace the `$APPIMAGE` file in place
//! * anything else → manual download from the Releases page
//!
//! All decisions that can be pure (`classify`, version compare, asset pick)
//! are pure so they stay unit-testable. Only `fetch_latest`, `download`, and
//! `install` touch the network / filesystem / privilege boundary.

use std::ffi::OsString;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::operations::{AuthMethod, OperationResult};
use crate::domain::system;

pub const RELEASES_URL: &str = "https://github.com/khurrambhutto/scope/releases";
pub const API_LATEST_URL: &str =
    "https://api.github.com/repos/khurrambhutto/scope/releases/latest";

const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT_SECS: &str = "600";

/// How this copy of Scope was installed. Only these three can self-update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallKind {
    AppImage,
    Deb,
    Rpm,
    Unknown,
}

impl InstallKind {
    pub fn can_self_update(self) -> bool {
        match self {
            InstallKind::AppImage | InstallKind::Deb | InstallKind::Rpm => true,
            InstallKind::Unknown => false,
        }
    }
}

/// Pure classifier so the decision is unit-testable without touching the real
/// process environment (env mutation is process-global and racy).
///
/// `bundle` is the `SCOPE_BUNDLE` marker baked in at packaging time
/// (`deb`/`rpm`/`appimage`), `appimage_var` is `$APPIMAGE`, and
/// `deb_marker` reports whether this looks like a dpkg install
/// (e.g. `/usr/share/doc/scope` exists).
pub fn classify(
    bundle: Option<String>,
    appimage_var: Option<OsString>,
    deb_marker: bool,
    rpm_marker: bool,
) -> InstallKind {
    if let Some(b) = bundle.as_deref().map(str::to_lowercase) {
        match b.as_str() {
            "deb" => return InstallKind::Deb,
            "rpm" => return InstallKind::Rpm,
            "appimage" => return InstallKind::AppImage,
            _ => {}
        }
    }
    if appimage_var.is_some() {
        return InstallKind::AppImage;
    }
    if deb_marker {
        return InstallKind::Deb;
    }
    if rpm_marker {
        return InstallKind::Rpm;
    }
    InstallKind::Unknown
}

/// Detect how this copy was installed from the live environment.
pub fn detect() -> InstallKind {
    let bundle = std::env::var("SCOPE_BUNDLE").ok();
    let appimage_var = std::env::var_os("APPIMAGE");
    // dpkg installs drop a doc dir; rpm installs drop /usr/share/doc/scope* too,
    // so check dpkg's own database first via a cheap filesystem probe only
    // (no command execution on the UI path).
    let deb_marker = std::path::Path::new("/usr/share/doc/scope").is_dir()
        || std::path::Path::new("/usr/share/doc/scope-gpui").is_dir();
    let rpm_marker = false;
    classify(bundle, appimage_var, deb_marker, rpm_marker)
}

/// Strip a leading `v` from a git tag (`v0.3.0` → `0.3.0`).
pub fn strip_v(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag).trim()
}

fn parse_parts(version: &str) -> (Vec<u64>, Option<String>) {
    let version = strip_v(version);
    // Split off pre-release/build metadata: `1.2.3-rc.1+build` → core `1.2.3`.
    let core_end = version.find(['-', '+']).unwrap_or(version.len());
    let (core, suffix) = version.split_at(core_end);
    let numbers = core
        .split('.')
        .map(|p| p.trim().parse::<u64>().unwrap_or(0))
        .collect();
    let suffix = if suffix.is_empty() {
        None
    } else {
        Some(suffix.to_string())
    };
    (numbers, suffix)
}

/// True when `latest` is newer than `current`.
///
/// Numeric core comparison first; a stable release beats a pre-release with
/// the same core; different pre-release strings compare lexicographically
/// (good enough for update prompting, never for ordering writes).
pub fn is_newer(current: &str, latest: &str) -> bool {
    let (cur_nums, cur_suffix) = parse_parts(current);
    let (lat_nums, lat_suffix) = parse_parts(latest);
    let width = cur_nums.len().max(lat_nums.len()).max(1);
    for i in 0..width {
        let c = cur_nums.get(i).copied().unwrap_or(0);
        let l = lat_nums.get(i).copied().unwrap_or(0);
        if l != c {
            return l > c;
        }
    }
    match (cur_suffix, lat_suffix) {
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (Some(a), Some(b)) => b > a,
        (None, None) => false,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub body: Option<String>,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

impl GithubRelease {
    pub fn version(&self) -> &str {
        strip_v(&self.tag_name)
    }
}

/// Pick the download asset matching this install kind:
/// `.deb` → deb, `.rpm` → rpm, `.AppImage` → appimage.
pub fn select_asset(
    release: &GithubRelease,
    kind: InstallKind,
) -> Option<&ReleaseAsset> {
    let want = match kind {
        InstallKind::Deb => ".deb",
        InstallKind::Rpm => ".rpm",
        InstallKind::AppImage => ".appimage",
        InstallKind::Unknown => return None,
    };
    release
        .assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(want))
}

/// Outcome of a check: enough for the banner to render without more I/O.
#[derive(Debug, Clone)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: String,
    pub notes: String,
    pub url: String,
    pub kind: InstallKind,
    pub can_self_update: bool,
}

pub fn check_update(
    current: &str,
    release: &GithubRelease,
    kind: InstallKind,
) -> Option<UpdateCheck> {
    if !is_newer(current, release.version()) {
        return None;
    }
    let asset = select_asset(release, kind);
    Some(UpdateCheck {
        current: current.to_string(),
        latest: release.version().to_string(),
        notes: release.body.clone().unwrap_or_default(),
        url: asset
            .map(|a| a.browser_download_url.clone())
            .unwrap_or_else(|| RELEASES_URL.to_string()),
        kind,
        can_self_update: kind.can_self_update() && asset.is_some(),
    })
}

/// Fetch the latest GitHub release via `curl` (no new HTTP dependency).
pub async fn fetch_latest() -> anyhow::Result<GithubRelease> {
    let body = system::capture_stdout(
        "curl",
        &[
            "-fsSL",
            "--max-time",
            "20",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: scope-updater",
            API_LATEST_URL,
        ],
        FETCH_TIMEOUT,
    )
    .await?;
    Ok(serde_json::from_str(&body)?)
}

/// Download `url` to `dest` via `curl`. Streams progress lines to `on_line`.
pub async fn download(
    url: &str,
    dest: &std::path::Path,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> anyhow::Result<()> {
    let dest_str = dest.to_string_lossy().to_string();
    let out = system::capture_output(
        "curl",
        &[
            "-fSL",
            "--max-time",
            DOWNLOAD_TIMEOUT_SECS,
            "-o",
            &dest_str,
            url,
        ],
        Duration::from_secs(620),
    )
    .await?;
    if out.success {
        on_line(&format!("[scope] downloaded {}", dest_str));
        Ok(())
    } else {
        anyhow::bail!("download failed: {}", out.stderr.trim())
    }
}

/// Install a downloaded artifact. `appimage_target` is `$APPIMAGE` when set.
pub async fn install(
    kind: InstallKind,
    file: &std::path::Path,
    appimage_target: Option<&std::path::Path>,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let file_str = file.to_string_lossy().to_string();
    match kind {
        InstallKind::Deb => {
            system::run_elevated(
                "dpkg",
                &["-i", &file_str],
                AuthMethod::Pkexec,
                Duration::from_secs(300),
                on_line,
            )
            .await
        }
        InstallKind::Rpm => {
            system::run_elevated(
                "rpm",
                &["-U", &file_str],
                AuthMethod::Pkexec,
                Duration::from_secs(300),
                on_line,
            )
            .await
        }
        InstallKind::AppImage => {
            let Some(target) = appimage_target else {
                return OperationResult {
                    success: false,
                    message: "Cannot locate the running AppImage to replace.".into(),
                    logs: "missing $APPIMAGE".into(),
                    exit_code: None,
                };
            };
            on_line(&format!("[scope] Installing AppImage to {}", target.display()));
            match tokio::fs::copy(file, target).await {
                Ok(_) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = tokio::fs::set_permissions(
                            target,
                            std::fs::Permissions::from_mode(0o755),
                        )
                        .await;
                    }
                    OperationResult {
                        success: true,
                        message: "AppImage updated. Restart Scope to use the new version."
                            .into(),
                        logs: format!(
                            "copied {} to {}",
                            file.display(),
                            target.display()
                        ),
                        exit_code: Some(0),
                    }
                }
                Err(e) => OperationResult {
                    success: false,
                    message: format!("AppImage install failed: {e}"),
                    logs: format!("copy error: {e}"),
                    exit_code: None,
                },
            }
        }
        InstallKind::Unknown => OperationResult {
            success: false,
            message: format!(
                "This install can't update itself. Download the new version from {RELEASES_URL}"
            ),
            logs: "unknown install kind".into(),
            exit_code: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_with(assets: &[(&str, &str)]) -> GithubRelease {
        GithubRelease {
            tag_name: "v0.3.0".into(),
            body: Some("notes".into()),
            assets: assets
                .iter()
                .map(|(n, u)| ReleaseAsset {
                    name: (*n).into(),
                    browser_download_url: (*u).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn bundle_marker_wins() {
        assert_eq!(
            classify(Some("deb".into()), None, false, false),
            InstallKind::Deb
        );
        assert_eq!(
            classify(Some("RPM".into()), None, false, false),
            InstallKind::Rpm
        );
        assert_eq!(
            classify(Some("appimage".into()), None, false, false),
            InstallKind::AppImage
        );
    }

    #[test]
    fn appimage_env_fallback() {
        assert_eq!(
            classify(None, Some(OsString::from("/tmp/x.AppImage")), false, false),
            InstallKind::AppImage
        );
    }

    #[test]
    fn unknown_when_nothing_matches() {
        assert_eq!(classify(None, None, false, false), InstallKind::Unknown);
        assert_eq!(
            classify(Some("msi".into()), None, false, false),
            InstallKind::Unknown
        );
    }

    #[test]
    fn deb_marker_fallback() {
        assert_eq!(classify(None, None, true, false), InstallKind::Deb);
    }

    #[test]
    fn version_compare() {
        assert!(is_newer("0.2.0", "0.3.0"));
        assert!(is_newer("0.2.0", "v0.3.0"));
        assert!(!is_newer("0.3.0", "0.3.0"));
        assert!(!is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.2.9", "0.10.0"));
        assert!(is_newer("0.3.0-rc.1", "0.3.0"));
        assert!(!is_newer("0.3.0", "0.3.0-rc.1"));
    }

    #[test]
    fn strip_v_prefix() {
        assert_eq!(strip_v("v0.3.0"), "0.3.0");
        assert_eq!(strip_v("0.3.0"), "0.3.0");
    }

    #[test]
    fn asset_pick_per_kind() {
        let r = release_with(&[
            ("scope_0.3.0_amd64.deb", "https://x/d.deb"),
            ("scope-0.3.0.x86_64.rpm", "https://x/d.rpm"),
            ("Scope-0.3.0-x86_64.AppImage", "https://x/d.AppImage"),
        ]);
        assert_eq!(select_asset(&r, InstallKind::Deb).unwrap().name, "scope_0.3.0_amd64.deb");
        assert_eq!(select_asset(&r, InstallKind::Rpm).unwrap().name, "scope-0.3.0.x86_64.rpm");
        assert_eq!(
            select_asset(&r, InstallKind::AppImage).unwrap().name,
            "Scope-0.3.0-x86_64.AppImage"
        );
        assert!(select_asset(&r, InstallKind::Unknown).is_none());
    }

    #[test]
    fn no_update_when_current() {
        let r = release_with(&[]);
        assert!(check_update("0.3.0", &r, InstallKind::Deb).is_none());
        assert!(check_update("0.4.0", &r, InstallKind::Deb).is_none());
    }

    #[test]
    fn update_check_reports_manual_when_no_asset() {
        let r = release_with(&[]);
        let check = check_update("0.2.0", &r, InstallKind::Deb).unwrap();
        assert_eq!(check.latest, "0.3.0");
        assert!(!check.can_self_update);
        assert_eq!(check.url, RELEASES_URL);
    }

    #[test]
    fn self_update_flags() {
        assert!(InstallKind::Deb.can_self_update());
        assert!(InstallKind::Rpm.can_self_update());
        assert!(InstallKind::AppImage.can_self_update());
        assert!(!InstallKind::Unknown.can_self_update());
    }
}
