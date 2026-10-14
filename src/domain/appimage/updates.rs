//! Update discovery from the update information embedded in an AppImage.
//!
//! Like Gear Lever, Scope reads `.upd_info` and supports the two transports
//! publishers actually use: `gh-releases-zsync` (GitHub releases) and `zsync`
//! (a fixed manifest URL). A newer release is detected by comparing the
//! installed file's digest with the digest the publisher's zsync manifest or
//! GitHub asset reports; a release without a digest is an unavailable check,
//! never "up to date". Users cannot type sources; only embedded data is used.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::hashes::Digests;
use crate::domain::package::{InstalledPackage, PackageSource};

const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
const CHECK_BUDGET: Duration = Duration::from_secs(30);
const WORKERS: usize = 2;

/// A newer AppImage the publisher released, resolved during a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppImageUpdate {
    /// HTTPS URL of the new AppImage.
    pub download_url: String,
    /// Digest the downloaded file must match before it is installed.
    pub digest: Digest,
    /// Release tag, when the source has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// Human description of where the update comes from.
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "algorithm", content = "hex", rename_all = "lowercase")]
pub enum Digest {
    Sha1(String),
    Sha256(String),
}

impl Digest {
    pub fn matches(&self, digests: &Digests) -> bool {
        match self {
            Digest::Sha1(hex) => hex.eq_ignore_ascii_case(&digests.sha1),
            Digest::Sha256(hex) => hex.eq_ignore_ascii_case(&digests.sha256),
        }
    }
}

/// A parsed `.upd_info` transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    GitHub {
        owner: String,
        repo: String,
        tag: String,
        pattern: String,
    },
    Zsync {
        url: String,
    },
}

impl Source {
    pub fn parse(info: &str) -> Result<Self> {
        let fields: Vec<&str> = info.trim().split('|').collect();
        match fields.as_slice() {
            ["gh-releases-zsync", owner, repo, tag, pattern] => {
                let valid = |s: &str| {
                    !s.is_empty()
                        && s.chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                };
                if !valid(owner) || !valid(repo) || tag.is_empty() || pattern.is_empty() {
                    bail!("The AppImage's GitHub update information is malformed");
                }
                Ok(Source::GitHub {
                    owner: owner.to_string(),
                    repo: repo.to_string(),
                    tag: tag.to_string(),
                    pattern: pattern.to_string(),
                })
            }
            ["zsync", url] => {
                let parsed = url::Url::parse(url).context("Invalid zsync update URL")?;
                if parsed.scheme() != "https" {
                    bail!("The AppImage's update URL is not HTTPS, so Scope will not use it");
                }
                Ok(Source::Zsync {
                    url: url.to_string(),
                })
            }
            [kind, ..] => bail!("Update information uses '{kind}', which Scope cannot check"),
            [] => bail!("The AppImage has no update information"),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Source::GitHub { owner, repo, .. } => format!("GitHub releases of {owner}/{repo}"),
            Source::Zsync { url } => url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .map(|host| format!("zsync manifest on {host}"))
                .unwrap_or_else(|| "zsync manifest".into()),
        }
    }
}

/// Check every managed AppImage with update information, two at a time,
/// within one overall budget.
pub async fn check_all(packages: &mut [InstalledPackage]) {
    let mut tasks = tokio::task::JoinSet::new();
    let mut pending: Vec<usize> = Vec::new();
    for (index, pkg) in packages.iter_mut().enumerate() {
        if pkg.source != PackageSource::AppImage {
            continue;
        }
        let Some(installation) = pkg.appimage_installation.as_ref() else {
            continue;
        };
        match installation.update_info.as_deref() {
            None => {
                pkg.update_note = Some("This AppImage does not publish update information.".into())
            }
            Some(info) => match Source::parse(info) {
                Ok(_) => pending.push(index),
                Err(error) => pkg.update_note = Some(error.to_string()),
            },
        }
    }
    let deadline = tokio::time::Instant::now() + CHECK_BUDGET;
    let mut next = 0;
    let mut done = vec![false; packages.len()];
    loop {
        while tasks.len() < WORKERS && next < pending.len() {
            let index = pending[next];
            next += 1;
            let Some(installation) = packages[index].appimage_installation.clone() else {
                continue;
            };
            tasks.spawn(async move { (index, check(&installation).await) });
        }
        let joined = match tokio::time::timeout_at(deadline, tasks.join_next()).await {
            Ok(Some(joined)) => joined,
            Ok(None) => break,
            Err(_) => {
                tasks.abort_all();
                for &index in &pending {
                    if !done[index] {
                        packages[index].update_note =
                            Some("Update check timed out. Rescan to retry.".into());
                    }
                }
                break;
            }
        };
        let Ok((index, result)) = joined else {
            continue;
        };
        done[index] = true;
        let pkg = &mut packages[index];
        match result {
            Ok(update) => {
                pkg.has_update = update.is_some();
                pkg.update_version = update
                    .as_ref()
                    .map(|u| u.release.clone().unwrap_or_else(|| "latest".into()));
                if let Some(installation) = pkg.appimage_installation.as_mut() {
                    installation.update = update;
                }
            }
            Err(error) => pkg.update_note = Some(format!("Update check failed: {error}")),
        }
    }
}

/// Resolve the newest release and report it when its digest differs.
pub async fn check(installation: &super::AppImageInstallation) -> Result<Option<AppImageUpdate>> {
    let info = installation
        .update_info
        .as_deref()
        .context("The AppImage has no update information")?;
    let source = Source::parse(info)?;
    let path = installation.path.clone();
    let digests = tokio::task::spawn_blocking(move || super::hashes::digests(&path))
        .await
        .context("Hash worker stopped")??;
    let candidate = resolve(&source).await?;
    Ok((!candidate.installed_matches(&digests)).then_some(candidate.update))
}

/// The latest release plus what identifies the installed file as current.
struct Candidate {
    update: AppImageUpdate,
    /// Digests the publisher reported for the latest file.
    published: Vec<Digest>,
}

impl Candidate {
    fn installed_matches(&self, digests: &Digests) -> bool {
        self.published.iter().any(|digest| digest.matches(digests))
    }
}

async fn resolve(source: &Source) -> Result<Candidate> {
    match source {
        Source::GitHub {
            owner,
            repo,
            tag,
            pattern,
        } => github(owner, repo, tag, pattern, source.describe()).await,
        Source::Zsync { url } => {
            let header = ZsyncHeader::parse(&fetch_head(url).await?)?;
            let download_url = header.download_url(url)?;
            Ok(Candidate {
                published: vec![Digest::Sha1(header.sha1.clone())],
                update: AppImageUpdate {
                    download_url,
                    digest: Digest::Sha1(header.sha1),
                    release: None,
                    source: source.describe(),
                },
            })
        }
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

async fn github(
    owner: &str,
    repo: &str,
    tag: &str,
    pattern: &str,
    source: String,
) -> Result<Candidate> {
    let base = format!("https://api.github.com/repos/{owner}/{repo}/releases");
    let release = match tag {
        "latest" => serde_json::from_str::<Release>(&fetch(&format!("{base}/latest")).await?)?,
        "latest-pre" | "latest-all" => {
            let releases: Vec<Release> =
                serde_json::from_str(&fetch(&format!("{base}?per_page=20")).await?)?;
            releases
                .into_iter()
                .filter(|r| !r.draft)
                .find(|r| tag == "latest-all" || r.prerelease)
                .context("No matching GitHub release was found")?
        }
        literal => {
            let encoded: String =
                url::form_urlencoded::byte_serialize(literal.as_bytes()).collect();
            serde_json::from_str::<Release>(&fetch(&format!("{base}/tags/{encoded}")).await?)?
        }
    };
    let matched = pick_asset(&release.assets, pattern).with_context(|| {
        format!(
            "Release {} has no file matching {pattern}",
            release.tag_name
        )
    })?;
    let (target, zsync) = match matched.name.strip_suffix(".zsync") {
        Some(target_name) => (
            release
                .assets
                .iter()
                .find(|asset| asset.name == target_name)
                .with_context(|| format!("Release {} lacks {target_name}", release.tag_name))?,
            Some(matched),
        ),
        None => (matched, None),
    };
    if !target
        .browser_download_url
        .starts_with("https://github.com/")
    {
        bail!("Unexpected GitHub download URL");
    }
    let sha256 = target
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|hex| is_hex(hex, 64))
        .map(|hex| Digest::Sha256(hex.to_lowercase()));
    let sha1 = match zsync {
        Some(zsync) => Some(Digest::Sha1(
            ZsyncHeader::parse(&fetch_head(&zsync.browser_download_url).await?)?.sha1,
        )),
        None => None,
    };
    let published: Vec<Digest> = sha256.iter().chain(sha1.iter()).cloned().collect();
    let digest = published.first().cloned().with_context(|| {
        format!(
            "{} does not publish a checksum, so Scope cannot tell whether it is newer",
            target.name
        )
    })?;
    Ok(Candidate {
        published,
        update: AppImageUpdate {
            download_url: target.browser_download_url.clone(),
            digest,
            release: Some(release.tag_name.trim_start_matches('v').to_string())
                .filter(|t| !t.is_empty()),
            source,
        },
    })
}

/// Pick the asset matching the update pattern, preferring this machine's
/// architecture when the pattern matches several.
fn pick_asset<'a>(assets: &'a [Asset], pattern: &str) -> Option<&'a Asset> {
    let matches: Vec<&Asset> = assets
        .iter()
        .filter(|asset| glob_match(pattern, &asset.name))
        .collect();
    if matches.len() <= 1 {
        return matches.first().copied();
    }
    let tokens: &[&str] = match super::host_architecture() {
        "x86_64" => &["x86_64", "amd64", "x64"],
        "aarch64" => &["aarch64", "arm64"],
        _ => &[],
    };
    matches
        .iter()
        .find(|asset| {
            let name = asset.name.to_lowercase();
            tokens.iter().any(|token| name.contains(token))
        })
        .or(matches.first())
        .copied()
}

/// `*` and `?` wildcard match, as AppImageUpdate uses for asset names.
fn glob_match(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut pi, mut ni, mut star, mut mark) = (0, 0, None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, PartialEq, Eq)]
struct ZsyncHeader {
    sha1: String,
    url: Option<String>,
}

impl ZsyncHeader {
    fn parse(text: &str) -> Result<Self> {
        let header = text.split("\n\n").next().unwrap_or("");
        let mut sha1 = None;
        let mut url = None;
        for line in header.lines() {
            match line.split_once(':') {
                Some(("SHA-1", value)) => sha1 = Some(value.trim().to_lowercase()),
                Some(("URL", value)) => url = Some(value.trim().to_string()),
                _ => {}
            }
        }
        let sha1 = sha1
            .filter(|hex| is_hex(hex, 40))
            .context("The zsync manifest has no SHA-1")?;
        Ok(Self { sha1, url })
    }

    /// The file URL, resolved against the manifest URL like Gear Lever.
    fn download_url(&self, manifest: &str) -> Result<String> {
        let base = url::Url::parse(manifest)?;
        let resolved = match &self.url {
            Some(relative) => base.join(relative)?,
            None => url::Url::parse(manifest.trim_end_matches(".zsync"))?,
        };
        if resolved.scheme() != "https" {
            bail!("The AppImage download URL is not HTTPS, so Scope will not use it");
        }
        Ok(resolved.to_string())
    }
}

async fn fetch(url: &str) -> Result<String> {
    Ok(crate::domain::system::capture_stdout(
        "curl",
        &[
            "-fsSL",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "20",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: scope-appimage-updates",
            url,
        ],
        FETCH_TIMEOUT,
    )
    .await?)
}

/// Fetch only the start of a zsync manifest; its header precedes the block
/// checksums.
async fn fetch_head(url: &str) -> Result<String> {
    Ok(crate::domain::system::capture_stdout(
        "curl",
        &[
            "-fsSL",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "20",
            "-r",
            "0-16383",
            "-H",
            "User-Agent: scope-appimage-updates",
            url,
        ],
        FETCH_TIMEOUT,
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_update_information() {
        assert_eq!(
            Source::parse(
                "gh-releases-zsync|probono|AppImages|latest|Subsurface-*x86_64.AppImage.zsync"
            )
            .unwrap(),
            Source::GitHub {
                owner: "probono".into(),
                repo: "AppImages".into(),
                tag: "latest".into(),
                pattern: "Subsurface-*x86_64.AppImage.zsync".into(),
            }
        );
        assert!(matches!(
            Source::parse("zsync|https://example.com/App-latest.AppImage.zsync").unwrap(),
            Source::Zsync { .. }
        ));
    }

    #[test]
    fn rejects_unsupported_insecure_or_malformed_information() {
        assert!(Source::parse("pling-v1-zsync|1623134|*-x86_64.AppImage").is_err());
        assert!(Source::parse("zsync|http://example.com/a.zsync").is_err());
        assert!(Source::parse("gh-releases-zsync|own/er|repo|latest|*.zsync").is_err());
        assert!(Source::parse("gh-releases-zsync|owner|repo|latest").is_err());
    }

    #[test]
    fn glob_matches_wildcards() {
        assert!(glob_match(
            "App-*x86_64.AppImage.zsync",
            "App-1.2-x86_64.AppImage.zsync"
        ));
        assert!(glob_match("*.AppImage", "x.AppImage"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match(
            "App-*x86_64.AppImage",
            "App-1.2-aarch64.AppImage"
        ));
    }

    #[test]
    fn picks_the_host_architecture_among_several_matches() {
        let asset = |name: &str| Asset {
            name: name.into(),
            browser_download_url: String::new(),
            digest: None,
        };
        let assets = vec![asset("App-arm64.AppImage"), asset("App-x86_64.AppImage")];
        let picked = pick_asset(&assets, "App-*.AppImage").unwrap();
        if super::super::host_architecture() == "x86_64" {
            assert_eq!(picked.name, "App-x86_64.AppImage");
        }
    }

    #[test]
    fn zsync_header_yields_digest_and_resolved_url() {
        let text = "zsync: 0.6.2\nFilename: App.AppImage\nURL: App-2.0.AppImage\nSHA-1: 0123456789ABCDEF0123456789abcdef01234567\n\n\u{0}binary";
        let header = ZsyncHeader::parse(text).unwrap();
        assert_eq!(header.sha1, "0123456789abcdef0123456789abcdef01234567");
        assert_eq!(
            header
                .download_url("https://example.com/dl/App.AppImage.zsync")
                .unwrap(),
            "https://example.com/dl/App-2.0.AppImage"
        );
        let header = ZsyncHeader {
            sha1: header.sha1,
            url: Some("http://elsewhere/App.AppImage".into()),
        };
        assert!(header
            .download_url("https://example.com/App.AppImage.zsync")
            .is_err());
        assert!(ZsyncHeader::parse("zsync: 0.6.2\n\n").is_err());
    }

    #[test]
    fn a_matching_published_digest_means_up_to_date() {
        let digests = Digests {
            sha1: "aa".repeat(20),
            sha256: "bb".repeat(32),
        };
        let candidate = Candidate {
            published: vec![Digest::Sha256("BB".repeat(32))],
            update: AppImageUpdate {
                download_url: "https://github.com/x".into(),
                digest: Digest::Sha256("bb".repeat(32)),
                release: None,
                source: String::new(),
            },
        };
        assert!(candidate.installed_matches(&digests));
        let newer = Candidate {
            published: vec![Digest::Sha1("cc".repeat(20))],
            ..candidate
        };
        assert!(!newer.installed_matches(&digests));
    }
}
