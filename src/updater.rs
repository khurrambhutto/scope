//! Self-update functionality for Scope
//!
//! Checks GitHub releases and updates the binary if a newer version is available.

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;

const GITHUB_REPO: &str = "khurrambhutto/scope";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Check for updates and optionally install them
pub fn check_and_update(auto_install: bool) -> Result<()> {
    println!("🔭 Scope Self-Updater");
    println!("Current version: v{}", CURRENT_VERSION);
    println!();

    // Get latest release info
    print!("Checking for updates... ");
    io::stdout().flush()?;
    
    let release = get_latest_release()?;
    let latest_version = parse_version(&release.tag_name)?;
    let current_version = Version::parse(CURRENT_VERSION)?;

    println!("done!");
    println!("Latest version: {}", release.tag_name);
    println!();

    if latest_version <= current_version {
        println!("✅ You're already running the latest version!");
        return Ok(());
    }

    println!("🆕 New version available: {} → {}", CURRENT_VERSION, release.tag_name);
    println!("   Release: {}", release.html_url);
    println!();

    // Find the appropriate asset
    let asset = find_linux_binary(&release.assets)?;
    println!("📦 Asset: {} ({:.2} MB)", asset.name, asset.size as f64 / 1_000_000.0);

    if !auto_install {
        print!("\nDo you want to update? [y/N]: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Update cancelled.");
            return Ok(());
        }
    }

    // Download and install
    println!("\n⬇️  Downloading...");
    download_and_install(&asset.browser_download_url)?;

    println!("\n✅ Successfully updated to {}!", release.tag_name);
    println!("   Please restart scope to use the new version.");

    Ok(())
}

/// Just check if an update is available (non-interactive)
pub fn check_update_available() -> Result<Option<String>> {
    let release = get_latest_release()?;
    let latest_version = parse_version(&release.tag_name)?;
    let current_version = Version::parse(CURRENT_VERSION)?;

    if latest_version > current_version {
        Ok(Some(release.tag_name))
    } else {
        Ok(None)
    }
}

fn get_latest_release() -> Result<GitHubRelease> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    
    let client = reqwest::blocking::Client::builder()
        .user_agent("scope-updater")
        .build()?;
    
    let response = client
        .get(&url)
        .send()
        .context("Failed to fetch release info from GitHub")?;

    if !response.status().is_success() {
        if response.status().as_u16() == 404 {
            anyhow::bail!("No releases found. Please create a release on GitHub first.");
        }
        anyhow::bail!("GitHub API error: {}", response.status());
    }

    let release: GitHubRelease = response
        .json()
        .context("Failed to parse GitHub release info")?;

    Ok(release)
}

fn parse_version(tag: &str) -> Result<Version> {
    // Remove 'v' prefix if present
    let version_str = tag.trim_start_matches('v');
    Version::parse(version_str).context("Failed to parse version")
}

fn find_linux_binary(assets: &[GitHubAsset]) -> Result<&GitHubAsset> {
    // Look for Linux binary in order of preference
    let patterns = [
        "scope-linux-x86_64",
        "scope-linux-amd64", 
        "scope-x86_64-linux",
        "scope_amd64",
        "scope-linux",
        "scope",
    ];

    for pattern in patterns {
        for asset in assets {
            let name_lower = asset.name.to_lowercase();
            if name_lower.contains(pattern) && !name_lower.ends_with(".deb") && !name_lower.ends_with(".tar.gz") {
                return Ok(asset);
            }
        }
    }

    // Also check for just "scope" without any suffix
    for asset in assets {
        if asset.name == "scope" {
            return Ok(asset);
        }
    }

    anyhow::bail!(
        "No compatible Linux binary found in release assets.\n\
         Available assets: {:?}\n\
         Please upload a binary named 'scope' or 'scope-linux-x86_64' to the release.",
        assets.iter().map(|a| &a.name).collect::<Vec<_>>()
    )
}

fn download_and_install(url: &str) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("scope-updater")
        .build()?;

    // Download to temp file
    let response = client
        .get(url)
        .send()
        .context("Failed to download update")?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed: {}", response.status());
    }

    let bytes = response.bytes()?;
    
    // Get current executable path
    let current_exe = std::env::current_exe()
        .context("Failed to get current executable path")?;
    
    // Create temp file in the same directory
    let temp_path = current_exe.with_extension("new");
    let backup_path = current_exe.with_extension("backup");

    // Write new binary
    {
        let mut file = File::create(&temp_path)
            .context("Failed to create temp file")?;
        file.write_all(&bytes)?;
    }

    // Make executable
    fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
        .context("Failed to set executable permissions")?;

    // Backup current binary
    if current_exe.exists() {
        fs::rename(&current_exe, &backup_path)
            .context("Failed to backup current binary. Try running with sudo.")?;
    }

    // Move new binary to current location
    match fs::rename(&temp_path, &current_exe) {
        Ok(_) => {
            // Success - remove backup
            let _ = fs::remove_file(&backup_path);
            Ok(())
        }
        Err(e) => {
            // Failed - restore backup
            if backup_path.exists() {
                let _ = fs::rename(&backup_path, &current_exe);
            }
            Err(e).context("Failed to install update. Try running with sudo.")
        }
    }
}

/// Get the current version string
pub fn current_version() -> &'static str {
    CURRENT_VERSION
}
