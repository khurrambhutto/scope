//! Signed self-update checks and installation for packaged Scope builds.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::operations::{AuthMethod, OperationResult};
use crate::domain::system;

pub const RELEASES_URL: &str = "https://github.com/khurrambhutto/scope/releases";
pub const API_LATEST_URL: &str = "https://api.github.com/repos/khurrambhutto/scope/releases/latest";
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(620);
const CHECKSUMS_NAME: &str = "SHA256SUMS";
const SIGNATURE_NAME: &str = "SHA256SUMS.minisig";
const UPDATE_PUBLIC_KEY: Option<&str> = option_env!("SCOPE_UPDATE_PUBLIC_KEY");

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
        matches!(self, Self::AppImage | Self::Deb | Self::Rpm)
    }
}

pub fn classify(
    bundle: Option<String>,
    appimage_var: Option<OsString>,
    deb_marker: bool,
    rpm_marker: bool,
) -> InstallKind {
    if let Some(bundle) = bundle.as_deref().map(str::to_lowercase) {
        match bundle.as_str() {
            "deb" => return InstallKind::Deb,
            "rpm" => return InstallKind::Rpm,
            "appimage" => return InstallKind::AppImage,
            _ => {}
        }
    }
    if appimage_var.is_some() {
        InstallKind::AppImage
    } else if deb_marker {
        InstallKind::Deb
    } else if rpm_marker {
        InstallKind::Rpm
    } else {
        InstallKind::Unknown
    }
}

pub fn detect() -> InstallKind {
    let deb_marker = Path::new("/usr/share/doc/scope").is_dir()
        || Path::new("/usr/share/doc/scope-gpui").is_dir();
    classify(
        std::env::var("SCOPE_BUNDLE").ok(),
        std::env::var_os("APPIMAGE"),
        deb_marker,
        false,
    )
}

pub fn strip_v(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag).trim()
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    let (Ok(current), Ok(latest)) = (
        Version::parse(strip_v(current)),
        Version::parse(strip_v(latest)),
    ) else {
        return false;
    };
    latest > current
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

    fn named_asset(&self, name: &str) -> Option<&ReleaseAsset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

fn package_arch(kind: InstallKind, arch: &str) -> Option<&'static str> {
    match (kind, arch) {
        (InstallKind::Deb, "x86_64") => Some("amd64"),
        (InstallKind::Deb, "aarch64") => Some("arm64"),
        (InstallKind::Rpm | InstallKind::AppImage, "x86_64") => Some("x86_64"),
        (InstallKind::Rpm | InstallKind::AppImage, "aarch64") => Some("aarch64"),
        _ => None,
    }
}

fn expected_asset_name(version: &str, kind: InstallKind, arch: &str) -> Option<String> {
    let arch = package_arch(kind, arch)?;
    match kind {
        InstallKind::Deb => Some(format!("scope_{version}_{arch}.deb")),
        InstallKind::Rpm => Some(format!("scope-{version}.{arch}.rpm")),
        InstallKind::AppImage => Some(format!("Scope-{version}-{arch}.AppImage")),
        InstallKind::Unknown => None,
    }
}

pub fn select_asset<'a>(
    release: &'a GithubRelease,
    kind: InstallKind,
    arch: &str,
) -> Option<&'a ReleaseAsset> {
    release.named_asset(&expected_asset_name(release.version(), kind, arch)?)
}

#[derive(Debug, Clone)]
pub struct UpdateCheck {
    pub current: String,
    pub latest: String,
    pub notes: String,
    pub url: String,
    pub kind: InstallKind,
    pub can_self_update: bool,
    pub(crate) artifact_name: Option<String>,
    pub(crate) checksums_url: Option<String>,
    pub(crate) signature_url: Option<String>,
}

pub fn check_update(
    current: &str,
    release: &GithubRelease,
    kind: InstallKind,
) -> Option<UpdateCheck> {
    if !is_newer(current, release.version()) {
        return None;
    }
    let asset = select_asset(release, kind, std::env::consts::ARCH);
    let checksums = release.named_asset(CHECKSUMS_NAME);
    let signature = release.named_asset(SIGNATURE_NAME);
    let verifiable = UPDATE_PUBLIC_KEY.is_some()
        && asset.is_some()
        && checksums.is_some()
        && signature.is_some();
    Some(UpdateCheck {
        current: current.into(),
        latest: release.version().into(),
        notes: release.body.clone().unwrap_or_default(),
        url: asset
            .map(|item| item.browser_download_url.clone())
            .unwrap_or_else(|| RELEASES_URL.into()),
        kind,
        can_self_update: kind.can_self_update() && verifiable,
        artifact_name: asset.map(|item| item.name.clone()),
        checksums_url: checksums.map(|item| item.browser_download_url.clone()),
        signature_url: signature.map(|item| item.browser_download_url.clone()),
    })
}

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

async fn download(url: &str, dest: &Path) -> anyhow::Result<()> {
    let dest = dest.to_string_lossy().into_owned();
    let out = system::capture_output(
        "curl",
        &["-fSL", "--max-time", "600", "-o", &dest, url],
        DOWNLOAD_TIMEOUT,
    )
    .await?;
    if out.success {
        Ok(())
    } else {
        anyhow::bail!("download failed: {}", out.stderr.trim())
    }
}

/// A package held in a private temporary directory after all checks passed.
pub struct VerifiedArtifact {
    _temp_dir: tempfile::TempDir,
    path: PathBuf,
}

pub async fn download_verified(
    check: &UpdateCheck,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> anyhow::Result<VerifiedArtifact> {
    if !check.can_self_update {
        anyhow::bail!("this release is not configured for verified self-update")
    }
    let name = check
        .artifact_name
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("release is missing its package asset"))?;
    let checksums_url = check
        .checksums_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("release is missing {CHECKSUMS_NAME}"))?;
    let signature_url = check
        .signature_url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("release is missing {SIGNATURE_NAME}"))?;
    let temp_dir = tempfile::Builder::new().prefix("scope-update-").tempdir()?;
    let artifact_path = temp_dir.path().join(name);
    let manifest_path = temp_dir.path().join(CHECKSUMS_NAME);
    let signature_path = temp_dir.path().join(SIGNATURE_NAME);
    download(&check.url, &artifact_path).await?;
    download(checksums_url, &manifest_path).await?;
    download(signature_url, &signature_path).await?;
    on_line("[scope] downloaded update and signed checksum manifest");
    verify_manifest(&manifest_path, &signature_path, UPDATE_PUBLIC_KEY)?;
    verify_checksum(&artifact_path, name, &manifest_path)?;
    on_line("[scope] minisign signature and SHA-256 checksum verified");
    Ok(VerifiedArtifact {
        _temp_dir: temp_dir,
        path: artifact_path,
    })
}

fn verify_manifest(manifest: &Path, signature: &Path, key: Option<&str>) -> anyhow::Result<()> {
    let key = key.ok_or_else(|| anyhow::anyhow!("update public key is not embedded"))?;
    let key = if key.lines().count() > 1 {
        PublicKey::decode(key)
    } else {
        PublicKey::from_base64(key.trim())
    }
    .map_err(|error| anyhow::anyhow!("invalid update public key: {error}"))?;
    let signature = Signature::from_file(signature)
        .map_err(|error| anyhow::anyhow!("invalid update signature: {error}"))?;
    key.verify(&std::fs::read(manifest)?, &signature, false)
        .map_err(|error| anyhow::anyhow!("checksum signature verification failed: {error}"))
}

fn verify_checksum(artifact: &Path, name: &str, manifest: &Path) -> anyhow::Result<()> {
    let manifest = std::fs::read_to_string(manifest)?;
    let matches: Vec<&str> = manifest
        .lines()
        .filter_map(|line| line.split_once(char::is_whitespace))
        .filter_map(|(hash, file)| {
            (file.trim_start().trim_start_matches('*') == name).then_some(hash)
        })
        .collect();
    let [expected] = matches.as_slice() else {
        anyhow::bail!("checksum manifest must contain exactly one entry for {name}")
    };
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        anyhow::bail!("checksum manifest contains an invalid SHA-256 digest")
    }
    let mut file = std::fs::File::open(artifact)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if !format!("{:x}", hasher.finalize()).eq_ignore_ascii_case(expected) {
        anyhow::bail!("downloaded artifact checksum does not match the signed manifest")
    }
    Ok(())
}

pub async fn install(
    kind: InstallKind,
    artifact: &VerifiedArtifact,
    appimage_target: Option<&Path>,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let file = artifact.path.to_string_lossy().into_owned();
    match kind {
        InstallKind::Deb => {
            system::run_elevated(
                "dpkg",
                &["-i", &file],
                AuthMethod::Pkexec,
                Duration::from_secs(300),
                on_line,
            )
            .await
        }
        InstallKind::Rpm => {
            system::run_elevated(
                "rpm",
                &["-U", &file],
                AuthMethod::Pkexec,
                Duration::from_secs(300),
                on_line,
            )
            .await
        }
        InstallKind::AppImage => {
            let Some(target) = appimage_target else {
                return failed(
                    "Cannot locate the running AppImage to replace.",
                    "missing $APPIMAGE",
                );
            };
            on_line(&format!(
                "[scope] Installing AppImage to {}",
                target.display()
            ));
            match atomic_replace_appimage(&artifact.path, target) {
                Ok(()) => OperationResult {
                    success: true,
                    message: "AppImage updated. Restart Scope to use the new version.".into(),
                    logs: format!("atomically replaced {}", target.display()),
                    exit_code: Some(0),
                },
                Err(error) => failed(
                    format!("AppImage install failed: {error}"),
                    format!("atomic replacement error: {error}"),
                ),
            }
        }
        InstallKind::Unknown => failed(
            format!("This install can't update itself. Download it from {RELEASES_URL}"),
            "unknown install kind",
        ),
    }
}

fn atomic_replace_appimage(source: &Path, target: &Path) -> std::io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "AppImage has no parent directory",
        )
    })?;
    let mut staged = tempfile::Builder::new()
        .prefix(".scope-update-")
        .tempfile_in(parent)?;
    std::io::copy(&mut std::fs::File::open(source)?, &mut staged)?;
    staged.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    staged.as_file().sync_all()?;
    staged.persist(target).map_err(|error| error.error)?;
    std::fs::File::open(parent)?.sync_all()
}

fn failed(message: impl Into<String>, logs: impl Into<String>) -> OperationResult {
    OperationResult {
        success: false,
        message: message.into(),
        logs: logs.into(),
        exit_code: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(assets: &[(&str, &str)]) -> GithubRelease {
        GithubRelease {
            tag_name: "v0.3.0".into(),
            body: Some("notes".into()),
            assets: assets
                .iter()
                .map(|(name, url)| ReleaseAsset {
                    name: (*name).into(),
                    browser_download_url: (*url).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn semver_comparison_rejects_invalid_versions() {
        assert!(is_newer("0.2.0", "v0.3.0"));
        assert!(!is_newer("1.2.5", "1.2.x"));
    }

    #[test]
    fn select_asset_requires_exact_version_architecture_and_kind() {
        let release = release(&[
            ("scope_0.3.0_amd64.deb", "https://x/good"),
            ("scope_9.9.9_amd64.deb", "https://x/bad"),
        ]);
        assert_eq!(
            select_asset(&release, InstallKind::Deb, "x86_64").map(|asset| asset.name.as_str()),
            Some("scope_0.3.0_amd64.deb")
        );
    }

    #[test]
    fn checksum_accepts_one_exact_filename_entry() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("scope.deb");
        let manifest = dir.path().join(CHECKSUMS_NAME);
        std::fs::write(&artifact, b"scope").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"scope"));
        std::fs::write(&manifest, format!("{digest}  scope.deb\n")).unwrap();
        assert!(verify_checksum(&artifact, "scope.deb", &manifest).is_ok());
    }

    #[test]
    fn checksum_rejects_tampered_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("scope.deb");
        let manifest = dir.path().join(CHECKSUMS_NAME);
        std::fs::write(&artifact, b"tampered").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"scope"));
        std::fs::write(&manifest, format!("{digest}  scope.deb\n")).unwrap();
        assert!(verify_checksum(&artifact, "scope.deb", &manifest).is_err());
    }

    #[test]
    fn minisign_verification_rejects_legacy_signatures() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join(CHECKSUMS_NAME);
        let signature = dir.path().join(SIGNATURE_NAME);
        std::fs::write(&manifest, b"test").unwrap();
        std::fs::write(
            &signature,
            "untrusted comment: signature from minisign secret key\nRWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=\ntrusted comment: timestamp:1555779966\tfile:test\nQtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==\n",
        )
        .unwrap();
        let error = verify_manifest(
            &manifest,
            &signature,
            Some("RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("signature algorithm"));
    }

    #[test]
    fn appimage_replacement_replaces_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("download");
        let target = dir.path().join("Scope.AppImage");
        std::fs::write(&source, b"new").unwrap();
        std::fs::write(&target, b"old").unwrap();
        atomic_replace_appimage(&source, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
    }

    #[test]
    fn classify_and_capabilities_cover_known_kinds() {
        assert_eq!(
            classify(Some("deb".into()), None, false, false),
            InstallKind::Deb
        );
        assert!(InstallKind::Deb.can_self_update());
        assert!(!InstallKind::Unknown.can_self_update());
    }
}
