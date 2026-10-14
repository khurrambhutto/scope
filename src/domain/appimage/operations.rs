//! Uninstall and update for integrated AppImages.
//!
//! Removal moves the AppImage, every launcher that runs it, and its
//! `~/AppImages/.icons` entries to Trash, as Gear Lever does. Updating
//! downloads the published file into a hidden directory in `~/AppImages`,
//! checks it against the digest the publisher reported, and replaces the
//! installed copy in place, keeping its launcher's environment and arguments.
//! No step needs privileges.

use std::time::Duration;

use anyhow::{bail, Context, Result};

use super::app_data::{self, AppData};
use super::install::{integrate, Target};
use super::{managed, AppImageInstallation, Roots};
use crate::domain::operations::{Operation, OperationResult, PlanStep};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Preview steps for an integrated AppImage.
pub fn steps(installation: &AppImageInstallation, operation: Operation) -> Result<Vec<PlanStep>> {
    match operation {
        Operation::Uninstall => {
            let mut steps = vec![PlanStep {
                description: "Move the AppImage to Trash.".into(),
                command_summary: format!("gio trash {}", installation.path.display()),
            }];
            for launcher in &installation.launchers {
                steps.push(PlanStep {
                    description: "Move its launcher to Trash.".into(),
                    command_summary: format!("gio trash {}", launcher.display()),
                });
            }
            for icon in &installation.icons {
                steps.push(PlanStep {
                    description: "Move its icon to Trash.".into(),
                    command_summary: format!("gio trash {}", icon.display()),
                });
            }
            Ok(steps)
        }
        Operation::Update => {
            let update = installation
                .update
                .as_ref()
                .context("No verified AppImage update is available. Rescan first.")?;
            let (algorithm, hex) = match &update.digest {
                super::updates::Digest::Sha1(hex) => ("SHA-1", hex),
                super::updates::Digest::Sha256(hex) => ("SHA-256", hex),
            };
            Ok(vec![
                PlanStep {
                    description: format!("Download the new AppImage from {}.", update.source),
                    command_summary: format!("curl -fSL {}", update.download_url),
                },
                PlanStep {
                    description: format!("Require the published {algorithm} checksum."),
                    command_summary: hex.clone(),
                },
                PlanStep {
                    description:
                        "Replace the installed AppImage and refresh its launcher and icon.".into(),
                    command_summary: format!("{}", installation.path.display()),
                },
            ])
        }
        Operation::Install => bail!("Install plans are built from an opened file"),
    }
}

/// Re-verify the integration right before applying.
pub fn verify(installation: &AppImageInstallation) -> Result<()> {
    managed::verify(&Roots::current()?, installation)
}

/// App data attributable to `installation` by name, for the opt-in removal.
pub fn find_app_data(installation: &AppImageInstallation) -> Vec<AppData> {
    let Ok(roots) = Roots::current() else {
        return Vec::new();
    };
    app_data::find(
        &roots,
        &app_data::Bases::current(&roots),
        installation,
        &app_data::OtherLaunchers::current(installation),
    )
}

/// Trash the AppImage, its launchers and icons, and the opted-in `app_data`.
pub async fn uninstall(
    installation: &AppImageInstallation,
    app_data: &[AppData],
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let roots = match Roots::current() {
        Ok(roots) => roots,
        Err(error) => return failed(error.to_string()),
    };
    if let Err(error) = managed::verify(&roots, installation) {
        return failed(error.to_string());
    }
    // Search again while the launchers still exist; drop anything that changed.
    let app_data = if app_data.is_empty() {
        Vec::new()
    } else {
        let fresh = installation.clone();
        let fresh = tokio::task::spawn_blocking(move || find_app_data(&fresh))
            .await
            .unwrap_or_default();
        app_data::still_found(app_data, &fresh)
    };
    if let Err(error) = crate::domain::system::trash::trash(&installation.path).await {
        return failed(format!("Could not move the AppImage to Trash: {error}"));
    }
    on_line(&format!(
        "[scope] moved {} to Trash",
        installation.path.display()
    ));
    let mut problems = Vec::new();
    for path in installation
        .launchers
        .iter()
        .chain(&installation.icons)
        .chain(app_data.iter().map(|data| &data.path))
    {
        match crate::domain::system::trash::trash(path).await {
            Ok(()) => on_line(&format!("[scope] moved {} to Trash", path.display())),
            Err(error) => problems.push(format!("{}: {error}", path.display())),
        }
    }
    super::refresh_launchers(&roots, on_line).await;
    if problems.is_empty() {
        OperationResult {
            success: true,
            message: if app_data.is_empty() {
                "The AppImage, its launchers, and its icon were moved to Trash.".into()
            } else {
                "The AppImage, its launchers, its icon, and its app data were moved to Trash."
                    .into()
            },
            logs: String::new(),
            exit_code: Some(0),
        }
    } else {
        OperationResult {
            success: true,
            message: format!(
                "The AppImage was moved to Trash, but some files remain: {}",
                problems.join("; ")
            ),
            logs: String::new(),
            exit_code: Some(0),
        }
    }
}

pub async fn update(
    installation: &AppImageInstallation,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    match update_inner(installation, on_line).await {
        Ok(message) => OperationResult {
            success: true,
            message,
            logs: String::new(),
            exit_code: Some(0),
        },
        Err(error) => failed(error.to_string()),
    }
}

async fn update_inner(
    installation: &AppImageInstallation,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> Result<String> {
    let roots = Roots::current()?;
    managed::verify(&roots, installation)?;
    let update = installation
        .update
        .as_ref()
        .context("No verified AppImage update is available. Rescan first.")?;
    // Same filesystem as the destination, so the final rename is atomic; the
    // dot prefix keeps scanners from listing a partial download.
    let temp = tempfile::Builder::new()
        .prefix(".scope-update-")
        .tempdir_in(&roots.appimages)?;
    let download = temp.path().join("download.appimage");
    on_line(&format!("[scope] downloading {}", update.download_url));
    let out = crate::domain::system::capture_output(
        "curl",
        &[
            "-fSL",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "1800",
            "-o",
            &download.to_string_lossy(),
            &update.download_url,
        ],
        DOWNLOAD_TIMEOUT + Duration::from_secs(10),
    )
    .await?;
    if !out.success {
        bail!("Download failed: {}", out.stderr.trim());
    }
    let hashed = download.clone();
    let digests = tokio::task::spawn_blocking(move || super::hashes::compute(&hashed))
        .await
        .context("Hash worker stopped")??;
    if !update.digest.matches(&digests) {
        bail!("The downloaded file does not match the published checksum. Nothing was changed.");
    }
    on_line("[scope] published checksum verified");
    let meta = super::extract::read(&download).await?;
    if meta.runtime.architecture != super::host_architecture()
        && meta.runtime.architecture != "unknown"
    {
        bail!(
            "The update is built for {}, not this computer.",
            meta.runtime.architecture
        );
    }
    // The file can change between the scan and this point; verify again.
    managed::verify(&roots, installation)?;
    integrate(
        &roots,
        &download,
        &digests.sha256,
        &meta,
        &Target {
            destination: installation.path.clone(),
            launcher: installation.launcher.clone(),
            replaces: Some(installation),
        },
        on_line,
    )
    .await?;
    super::refresh_launchers(&roots, on_line).await;
    Ok(match &meta.version {
        Some(version) => format!("{} was updated to {version}", meta.name),
        None => format!("{} was updated", meta.name),
    })
}

fn failed(message: impl Into<String>) -> OperationResult {
    OperationResult {
        success: false,
        message: message.into(),
        logs: String::new(),
        exit_code: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::appimage::updates::{AppImageUpdate, Digest};
    use std::path::PathBuf;

    fn installation() -> AppImageInstallation {
        AppImageInstallation {
            path: PathBuf::from("/home/u/AppImages/demo.appimage"),
            launcher: PathBuf::from("/home/u/.local/share/applications/demo.desktop"),
            launchers: vec![PathBuf::from(
                "/home/u/.local/share/applications/demo.desktop",
            )],
            icons: vec![PathBuf::from("/home/u/AppImages/.icons/demo.png")],
            size: 1,
            modified_ns: 1,
            update_info: None,
            update: None,
        }
    }

    #[test]
    fn uninstall_steps_list_every_removed_file() {
        let steps = steps(&installation(), Operation::Uninstall).unwrap();
        let commands: Vec<_> = steps.iter().map(|s| s.command_summary.as_str()).collect();
        assert_eq!(
            commands,
            vec![
                "gio trash /home/u/AppImages/demo.appimage",
                "gio trash /home/u/.local/share/applications/demo.desktop",
                "gio trash /home/u/AppImages/.icons/demo.png",
            ]
        );
    }

    #[test]
    fn update_steps_require_a_resolved_update() {
        assert!(steps(&installation(), Operation::Update).is_err());
        let mut with_update = installation();
        with_update.update = Some(AppImageUpdate {
            download_url: "https://github.com/o/r/releases/download/v2/Demo.AppImage".into(),
            digest: Digest::Sha256("ab".repeat(32)),
            release: Some("2".into()),
            source: "GitHub releases of o/r".into(),
        });
        let steps = steps(&with_update, Operation::Update).unwrap();
        assert!(steps[0].command_summary.contains("Demo.AppImage"));
        assert_eq!(steps[1].command_summary, "ab".repeat(32));
    }
}
