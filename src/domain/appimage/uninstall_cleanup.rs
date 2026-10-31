//! Execute a verified uninstall and retain every incomplete cleanup in its result.

use std::future::Future;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::app_data::{self, AppData};
use super::AppImageInstallation;
use crate::domain::operations::OperationResult;

// Snapshot secondary integration files before the first asynchronous move. If a
// path is replaced while Trash is running, leave its replacement alone.
#[derive(PartialEq, Eq)]
struct FileIdentity(u64, u64, u32, u64, i64, i64);

fn file_identity(path: &Path) -> Option<FileIdentity> {
    let m = path.symlink_metadata().ok()?;
    if !m.is_file() || path.parent()?.canonicalize().ok()? != path.parent()? {
        return None;
    }
    Some(FileIdentity(
        m.dev(),
        m.ino(),
        m.uid(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
    ))
}

pub(super) async fn remove<F, Fut>(
    installation: &AppImageInstallation,
    planned: &[AppData],
    fresh: &[AppData],
    trash: F,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult
where
    F: Fn(PathBuf) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let secondary: Vec<_> = installation
        .launchers
        .iter()
        .chain(&installation.icons)
        .map(|path| (path, file_identity(path)))
        .collect();
    let mut warnings: Vec<_> = planned
        .iter()
        .filter(|data| !app_data::still_found(std::slice::from_ref(data), fresh).contains(data))
        .map(|data| {
            format!(
                "Skipped {}: missing, changed, shared, or no longer verifiable.",
                data.path.display()
            )
        })
        .collect();
    if let Err(error) = trash(installation.path.clone()).await {
        return OperationResult {
            cleanup: Vec::new(),
            success: false,
            message: format!("Could not move the AppImage to Trash: {error}"),
            logs: String::new(),
            exit_code: None,
            warnings,
        };
    }
    if !missing(&installation.path) {
        return OperationResult {
            cleanup: Vec::new(),
            success: false,
            message:
                "The AppImage path remains or cannot be verified after Trash; cleanup stopped."
                    .into(),
            logs: String::new(),
            exit_code: None,
            warnings,
        };
    }
    on_line(&format!(
        "[scope] moved {} to Trash",
        installation.path.display()
    ));
    for (path, identity) in secondary {
        if identity.is_none() || file_identity(path) != identity {
            warnings.push(format!(
                "Skipped {}: the launcher or icon changed during uninstall.",
                path.display()
            ));
            continue;
        }
        move_one(path, &trash, on_line, &mut warnings).await;
    }
    for data in app_data::still_found(planned, fresh) {
        if !app_data::unchanged(&data) {
            warnings.push(format!(
                "Skipped {}: app data changed during uninstall.",
                data.path.display()
            ));
            continue;
        }
        move_one(&data.path, &trash, on_line, &mut warnings).await;
    }
    for warning in &warnings {
        on_line(&format!("[scope] {warning}"));
    }
    OperationResult {
        cleanup: Vec::new(),
        success: true,
        message: if warnings.is_empty() {
            "The AppImage and selected files were moved to Trash.".into()
        } else {
            "The AppImage was moved to Trash, but cleanup is incomplete.".into()
        },
        logs: String::new(),
        exit_code: Some(0),
        warnings,
    }
}

async fn move_one<F, Fut>(
    path: &Path,
    trash: &F,
    on_line: &(dyn Fn(&str) + Send + Sync),
    warnings: &mut Vec<String>,
) where
    F: Fn(PathBuf) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    match trash(path.to_path_buf()).await {
        Ok(()) if missing(path) => on_line(&format!("[scope] moved {} to Trash", path.display())),
        Ok(()) => warnings.push(format!(
            "{} remains or cannot be verified after Trash",
            path.display()
        )),
        Err(error) => warnings.push(format!("{}: {error}", path.display())),
    }
}

fn missing(path: &Path) -> bool {
    matches!(path.symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
}

#[cfg(test)]
mod tests {
    use super::super::{
        app_data::{Bases, OtherLaunchers},
        managed, test_roots, Roots,
    };
    use super::*;

    struct Fixture {
        _temp: tempfile::TempDir,
        roots: Roots,
        bases: Bases,
        installation: AppImageInstallation,
        data: PathBuf,
        trash: PathBuf,
    }

    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let roots = test_roots(temp.path());
        let app = roots.appimages.join("research.appimage");
        std::fs::write(&app, super::super::elf::test_appimage("", b"hsqs")).unwrap();
        let launcher = roots.applications.join("research.desktop");
        std::fs::write(
            &launcher,
            format!(
                "[Desktop Entry]\nType=Application\nName=Research\nExec={}\n",
                app.display()
            ),
        )
        .unwrap();
        let icon = roots.icons().join("research.png");
        std::fs::write(icon, b"icon").unwrap();
        let integration = managed::discover(&roots).remove(&app).unwrap();
        let installation = managed::installation(&roots, &app, &integration).unwrap();
        let bases = Bases {
            config: roots.home.join(".config"),
            data: roots.home.join(".local/share"),
            cache: roots.home.join(".cache"),
            state: roots.home.join(".local/state"),
        };
        let data = bases.config.join("research");
        std::fs::create_dir_all(data.join("nested")).unwrap();
        std::fs::write(data.join("nested/profile"), b"settings").unwrap();
        let trash = roots.home.join("test-trash");
        std::fs::create_dir(&trash).unwrap();
        Fixture {
            _temp: temp,
            roots,
            bases,
            installation,
            data,
            trash,
        }
    }

    fn find(f: &Fixture, others: &OtherLaunchers) -> Vec<AppData> {
        app_data::find(&f.roots, &f.bases, &f.installation, others)
    }

    async fn move_to(path: PathBuf, trash: &Path) -> Result<()> {
        std::fs::rename(&path, trash.join(path.file_name().unwrap()))?;
        Ok(())
    }

    #[tokio::test]
    async fn opted_in_directory_and_integration_are_moved_with_profile_intact() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        assert!(managed::verify(&f.roots, &f.installation).is_ok());
        let result = remove(
            &f.installation,
            &planned,
            &planned,
            |path| move_to(path, &f.trash),
            &|_| {},
        )
        .await;
        assert!(result.success && result.warnings.is_empty(), "{result:?}");
        assert!(
            !f.installation.path.exists() && !f.installation.launcher.exists() && !f.data.exists()
        );
        assert_eq!(
            std::fs::read(f.trash.join("research/nested/profile")).unwrap(),
            b"settings"
        );
        assert!(f.trash.join("research.png").exists());
    }

    #[tokio::test]
    async fn unchecked_data_option_leaves_profile_in_place() {
        let f = fixture();
        let fresh = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &[],
            &fresh,
            |path| move_to(path, &f.trash),
            &|_| {},
        )
        .await;
        assert!(result.success && result.warnings.is_empty(), "{result:?}");
        assert!(f.data.join("nested/profile").exists());
    }

    #[tokio::test]
    async fn failed_data_move_reports_path_after_the_app_is_gone() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &planned,
            |path| {
                let f = &f;
                async move {
                    if path == f.data {
                        anyhow::bail!("injected Trash failure");
                    }
                    move_to(path, &f.trash).await
                }
            },
            &|_| {},
        )
        .await;
        assert!(result.success && !f.installation.path.exists());
        assert!(f.data.exists());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains(&f.data.display().to_string()));
        assert!(result.warnings[0].contains("injected Trash failure"));
    }

    #[tokio::test]
    async fn newly_shared_data_is_retained_and_reported() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let mut others = OtherLaunchers::default();
        others.add(
            "research",
            "Research",
            Some(Path::new("/usr/bin/research")),
            None,
        );
        let fresh = find(&f, &others);
        let result = remove(
            &f.installation,
            &planned,
            &fresh,
            |path| move_to(path, &f.trash),
            &|_| {},
        )
        .await;
        assert!(result.success && f.data.exists() && !f.installation.path.exists());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains(&f.data.display().to_string())));
    }

    #[tokio::test]
    async fn replacement_at_the_same_preview_path_is_retained() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        std::fs::rename(&f.data, f.roots.home.join("old-profile")).unwrap();
        std::fs::create_dir(&f.data).unwrap();
        std::fs::write(f.data.join("new-profile"), b"keep").unwrap();
        let fresh = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &fresh,
            |path| move_to(path, &f.trash),
            &|_| {},
        )
        .await;
        assert!(result.success && f.data.join("new-profile").exists());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains(&f.data.display().to_string())));
    }

    #[tokio::test]
    async fn replaced_data_during_executable_move_is_retained() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &planned,
            |path| {
                let f = &f;
                async move {
                    if path == f.installation.path {
                        std::fs::rename(&f.data, f.roots.home.join("old-profile"))?;
                        std::os::unix::fs::symlink(f.roots.home.join("old-profile"), &f.data)?;
                    }
                    move_to(path, &f.trash).await
                }
            },
            &|_| {},
        )
        .await;
        assert!(result.success && f.data.is_symlink());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains(&f.data.display().to_string())));
    }

    #[tokio::test]
    async fn executable_move_failure_leaves_all_other_files_untouched() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &planned,
            |_| async { anyhow::bail!("injected failure") },
            &|_| {},
        )
        .await;
        assert!(!result.success);
        assert!(
            f.installation.path.exists() && f.installation.launcher.exists() && f.data.exists()
        );
    }

    #[tokio::test]
    async fn successful_trash_without_executable_absence_stops_data_cleanup() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &planned,
            |_| async { Ok(()) },
            &|_| {},
        )
        .await;
        assert!(!result.success);
        assert!(
            f.installation.path.exists() && f.installation.launcher.exists() && f.data.exists()
        );
    }

    #[tokio::test]
    async fn replacement_launcher_is_retained_during_uninstall() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &planned,
            |path| {
                let f = &f;
                async move {
                    if path == f.installation.path {
                        std::fs::rename(
                            &f.installation.launcher,
                            f.roots.home.join("old-launcher"),
                        )?;
                        std::fs::write(&f.installation.launcher, b"replacement launcher")?;
                    }
                    move_to(path, &f.trash).await
                }
            },
            &|_| {},
        )
        .await;
        assert!(result.success && f.installation.launcher.exists());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains(&f.installation.launcher.display().to_string())));
    }

    #[tokio::test]
    async fn failed_icon_move_is_reported_without_stopping_data_cleanup() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &planned,
            |path| {
                let f = &f;
                async move {
                    if f.installation.icons.contains(&path) {
                        anyhow::bail!("injected icon failure");
                    }
                    move_to(path, &f.trash).await
                }
            },
            &|_| {},
        )
        .await;
        assert!(result.success && !f.data.exists());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("research.png") && w.contains("injected icon failure")));
    }

    #[tokio::test]
    async fn fresh_data_not_shown_in_preview_is_never_removed() {
        let f = fixture();
        let planned = find(&f, &OtherLaunchers::default());
        let cache = f.bases.cache.join("research");
        std::fs::create_dir_all(&cache).unwrap();
        let fresh = find(&f, &OtherLaunchers::default());
        let result = remove(
            &f.installation,
            &planned,
            &fresh,
            |path| move_to(path, &f.trash),
            &|_| {},
        )
        .await;
        assert!(result.success && cache.exists());
    }

    #[tokio::test]
    async fn legacy_preview_without_identity_retains_data() {
        let f = fixture();
        let fresh = find(&f, &OtherLaunchers::default());
        let mut planned = fresh.clone();
        for data in &mut planned {
            data.identity = None;
        }
        let result = remove(
            &f.installation,
            &planned,
            &fresh,
            |path| move_to(path, &f.trash),
            &|_| {},
        )
        .await;
        assert!(result.success && f.data.exists());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains(&f.data.display().to_string())));
    }
}
