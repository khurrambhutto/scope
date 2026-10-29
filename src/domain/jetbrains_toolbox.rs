//! JetBrains Toolbox manager launcher.
//!
//! Toolbox-managed IDEs cannot be updated or uninstalled through Scope. This
//! module only opens the manager's own desktop entry after validating it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

const DESKTOP_ID: &str = "jetbrains-toolbox";
const DESKTOP_FILE: &str = "jetbrains-toolbox.desktop";
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Open the installed JetBrains Toolbox manager through its desktop entry.
///
/// The desktop entry is resolved using XDG application-directory precedence so
/// a user override, including a hidden or invalid one, cannot be bypassed by a
/// lower-precedence system launcher.
pub async fn open() -> Result<(), String> {
    let launcher = manager_launcher_path()?;
    let gio = crate::domain::system::abs("gio");
    let display_path = launcher.display();
    let mut command = Command::new(&gio);
    command
        .arg("launch")
        .arg(&launcher)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    match tokio::time::timeout(LAUNCH_TIMEOUT, command.status()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(status)) => Err(format!(
            "Could not open JetBrains Toolbox: {gio} launch {display_path} exited with {status}."
        )),
        Ok(Err(error)) => Err(format!(
            "Could not start JetBrains Toolbox with {gio} launch {display_path}: {error}"
        )),
        Err(_) => Err(format!(
            "Opening JetBrains Toolbox timed out after {} seconds.",
            LAUNCH_TIMEOUT.as_secs()
        )),
    }
}

fn manager_launcher_path() -> Result<PathBuf, String> {
    manager_launcher_path_in(&crate::domain::desktop_entries::application_dirs())
}

fn manager_launcher_path_in(application_dirs: &[PathBuf]) -> Result<PathBuf, String> {
    for directory in application_dirs {
        let candidate = directory.join(DESKTOP_FILE);
        match fs::symlink_metadata(&candidate) {
            Ok(_) => return validate_launcher(&candidate),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "Could not inspect the JetBrains Toolbox launcher at {}: {error}",
                    candidate.display()
                ));
            }
        }
    }

    Err(
        "JetBrains Toolbox is not installed: no jetbrains-toolbox.desktop launcher was found."
            .to_string(),
    )
}

fn validate_launcher(candidate: &Path) -> Result<PathBuf, String> {
    let canonical = candidate.canonicalize().map_err(|error| {
        format!(
            "Could not resolve the JetBrains Toolbox launcher at {}: {error}",
            candidate.display()
        )
    })?;
    if !canonical.is_file() {
        return Err(format!(
            "JetBrains Toolbox launcher at {} is not a regular file.",
            candidate.display()
        ));
    }

    let launcher = crate::domain::desktop_entries::parse_launcher(candidate).ok_or_else(|| {
        format!(
            "JetBrains Toolbox launcher at {} is not a valid Application entry with an Exec command.",
            candidate.display()
        )
    })?;
    if launcher.id != DESKTOP_ID {
        return Err(format!(
            "JetBrains Toolbox launcher at {} has unexpected desktop ID {}.",
            candidate.display(),
            launcher.id
        ));
    }
    if !launcher.menu_visible {
        return Err(format!(
            "JetBrains Toolbox launcher at {} is hidden or unavailable.",
            candidate.display()
        ));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_launcher(directory: &Path, body: &str) -> PathBuf {
        fs::create_dir_all(directory).unwrap();
        let path = directory.join(DESKTOP_FILE);
        fs::write(&path, body).unwrap();
        path
    }

    fn manager_entry(executable: &Path) -> String {
        format!(
            "[Desktop Entry]\nType=Application\nName=JetBrains Toolbox\nExec={} %U\n",
            executable.display()
        )
    }

    fn executable(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, "").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn user_launcher_precedes_system_launcher() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user/applications");
        let system = root.path().join("system/applications");
        let user_executable = executable(root.path(), "user-toolbox");
        let system_executable = executable(root.path(), "system-toolbox");
        let user_launcher = write_launcher(&user, &manager_entry(&user_executable));
        write_launcher(&system, &manager_entry(&system_executable));

        assert_eq!(
            manager_launcher_path_in(&[user, system]).unwrap(),
            user_launcher.canonicalize().unwrap()
        );
    }

    #[test]
    fn hidden_user_launcher_masks_system_launcher() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user/applications");
        let system = root.path().join("system/applications");
        let executable = executable(root.path(), "toolbox");
        write_launcher(
            &user,
            &format!("{}Hidden=true\n", manager_entry(&executable)),
        );
        write_launcher(&system, &manager_entry(&executable));

        assert!(manager_launcher_path_in(&[user, system]).is_err());
    }

    #[test]
    fn invalid_user_launcher_masks_system_launcher() {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("user/applications");
        let system = root.path().join("system/applications");
        let executable = executable(root.path(), "toolbox");
        write_launcher(
            &user,
            "[Desktop Entry]\nType=Link\nName=JetBrains Toolbox\nURL=https://example.invalid\n",
        );
        write_launcher(&system, &manager_entry(&executable));

        assert!(manager_launcher_path_in(&[user, system]).is_err());
    }

    #[test]
    fn missing_launcher_returns_error() {
        let root = tempfile::tempdir().unwrap();

        assert!(manager_launcher_path_in(&[root.path().join("applications")]).is_err());
    }

    #[test]
    fn ide_desktop_file_is_not_a_toolbox_manager_launcher() {
        let root = tempfile::tempdir().unwrap();
        let applications = root.path().join("applications");
        let executable = executable(root.path(), "pycharm");
        fs::create_dir_all(&applications).unwrap();
        fs::write(
            applications.join("pycharm.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=PyCharm\nExec={} %U\n",
                executable.display()
            ),
        )
        .unwrap();

        assert!(manager_launcher_path_in(&[applications]).is_err());
    }

    #[test]
    fn nonregular_manager_launcher_is_rejected_before_parsing() {
        let root = tempfile::tempdir().unwrap();
        let applications = root.path().join("applications");
        fs::create_dir_all(applications.join(DESKTOP_FILE)).unwrap();

        assert!(manager_launcher_path_in(&[applications]).is_err());
    }
}
