//! Preview-bound management of dedicated, user-owned extracted AppDir installs.

use std::future::Future;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::appimage::desktop::{exec_program, unescape, DesktopFile};
use crate::domain::operations::OperationResult;

mod ownership;
use ownership::{check_mounts, collect_launchers, validate_tree};

const ENTRY_LIMIT: usize = 100_000;
const WALK_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct Roots {
    pub home: PathBuf,
    pub applications: PathBuf,
    pub other_applications: Vec<PathBuf>,
    pub processes: PathBuf,
}

impl Roots {
    pub fn current() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .context("HOME is missing or relative")?;
        let applications = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"))
            .join("applications");
        let others = std::env::var_os("XDG_DATA_DIRS")
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        let mut other_applications = Vec::new();
        for base in std::env::split_paths(&others) {
            if !base.is_absolute() {
                bail!("XDG_DATA_DIRS contains a relative location");
            }
            let path = base.join("applications");
            if path != applications && !other_applications.contains(&path) {
                other_applications.push(path);
            }
        }
        Ok(Self {
            home,
            applications,
            other_applications,
            processes: "/proc".into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Identity {
    device: u64,
    inode: u64,
    owner: u32,
    length: u64,
    modified: i64,
    modified_ns: i64,
}

impl Identity {
    fn read(path: &Path) -> Result<Self> {
        let m = path
            .symlink_metadata()
            .with_context(|| format!("Cannot inspect {}", path.display()))?;
        if !(m.is_dir() || m.is_file()) || path.canonicalize()? != path {
            bail!(
                "{} is linked or not an ordinary file/directory",
                path.display()
            );
        }
        Ok(Self {
            device: m.dev(),
            inode: m.ino(),
            owner: m.uid(),
            length: m.len(),
            modified: m.mtime(),
            modified_ns: m.mtime_nsec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedInstallation {
    pub root: PathBuf,
    pub executable: PathBuf,
    pub launcher: PathBuf,
    pub launchers: Vec<PathBuf>,
    pub name: String,
    pub version: String,
    home: PathBuf,
    applications: PathBuf,
    identities: Vec<(PathBuf, Identity)>,
}

pub fn discover(desktop_file: &Path) -> Result<ExtractedInstallation> {
    discover_with_roots(&Roots::current()?, desktop_file)
}

pub fn discover_with_roots(roots: &Roots, desktop_file: &Path) -> Result<ExtractedInstallation> {
    let home = Identity::read(&roots.home)?;
    if desktop_file.parent() != Some(roots.applications.as_path()) {
        bail!(
            "Only a launcher directly in your applications directory can manage an extracted app"
        );
    }
    let launcher_id = Identity::read(desktop_file)?;
    if launcher_id.owner != home.owner || !desktop_file.is_file() {
        bail!("The extracted app launcher is not owned by you");
    }
    let desktop = DesktopFile::read(desktop_file)?;
    let executable = target(&desktop).context("Launcher does not directly name an executable")?;
    let root = executable
        .parent()
        .context("Executable has no installation root")?
        .to_path_buf();
    if executable.file_name().is_none_or(|name| name != "AppRun")
        || root.parent() != Some(roots.home.join(".local/opt").as_path())
        || root
            .file_name()
            .is_none_or(|name| name.to_string_lossy().starts_with('.'))
    {
        bail!("Only dedicated AppDir installations directly in ~/.local/opt are supported");
    }
    let root_id = Identity::read(&root)?;
    let executable_id = Identity::read(&executable)?;
    if !root.is_dir()
        || !executable.is_file()
        || executable_id.owner != home.owner
        || root_id.owner != home.owner
        || root_id.device != home.device
        || executable_id.device != home.device
        || executable.symlink_metadata()?.mode() & 0o111 == 0
    {
        bail!("The extracted installation is not an owned executable tree on your home filesystem");
    }
    if !root.join("usr").is_dir() || root.join("usr").canonicalize()? != root.join("usr") {
        bail!("The extracted installation has no ordinary usr directory");
    }
    let name = desktop
        .entry("Name")
        .filter(|name| !name.trim().is_empty())
        .context("Launcher has no application name")?
        .to_string();
    let mut manifests = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "desktop") {
            let id = Identity::read(&path)?;
            let manifest = DesktopFile::read(&path)?;
            if manifest.entry("Type") == Some("Application")
                && manifest
                    .entry("Name")
                    .is_some_and(|value| value.eq_ignore_ascii_case(&name))
            {
                manifests.push((path, id));
            }
        }
    }
    if manifests.is_empty() {
        bail!("The installation has no matching bundled application metadata");
    }
    check_mounts(roots, &root)?;
    validate_tree(&root, home.owner, home.device)?;
    let mut launchers = collect_launchers(roots, &root, &executable, home.owner)?;
    launchers.sort();
    if !launchers.contains(&desktop_file.to_path_buf()) {
        bail!("Launcher ownership changed");
    }
    let mut identities = vec![(root.clone(), root_id), (executable.clone(), executable_id)];
    identities.extend(manifests);
    for path in &launchers {
        identities.push((path.clone(), Identity::read(path)?));
    }
    identities.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(ExtractedInstallation {
        root,
        executable,
        launcher: desktop_file.into(),
        launchers,
        name,
        version: desktop
            .entry("X-AppImage-Version")
            .unwrap_or_default()
            .into(),
        home: roots.home.clone(),
        applications: roots.applications.clone(),
        identities,
    })
}

fn target(file: &DesktopFile) -> Option<PathBuf> {
    if file.entry("Type") != Some("Application") {
        return None;
    }
    let path = PathBuf::from(exec_program(&unescape(file.entry("Exec")?))?);
    if !path.is_absolute() {
        return None;
    }
    if file
        .entry("TryExec")
        .filter(|value| !value.is_empty())
        .is_some_and(|value| Path::new(&unescape(value)) != path)
    {
        return None;
    }
    Some(path)
}

pub fn verify(installation: &ExtractedInstallation) -> Result<()> {
    verify_with_roots(&Roots::current()?, installation)
}

pub fn verify_with_roots(roots: &Roots, installation: &ExtractedInstallation) -> Result<()> {
    if installation.home != roots.home || installation.applications != roots.applications {
        bail!("Extracted installation context changed");
    }
    let fresh = discover_with_roots(roots, &installation.launcher)?;
    if &fresh != installation {
        bail!("Extracted installation changed since preview; rescan first");
    }
    Ok(())
}

pub async fn uninstall(
    installation: &ExtractedInstallation,
    review: &crate::domain::app_data::CleanupPlan,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let acknowledged = match crate::domain::app_data::extracted_process_review(review, installation)
    {
        Ok(acknowledged) => acknowledged,
        Err(error) => return failed(error.to_string()),
    };
    let roots = match Roots::current() {
        Ok(roots) => roots,
        Err(error) => return failed(error.to_string()),
    };
    remove(
        &roots,
        installation,
        acknowledged,
        |path| {
            let expected = installation
                .identities
                .iter()
                .find(|(original, _)| original == &path)
                .map(|(_, identity)| identity.clone());
            async move {
                let expected = expected.context("Reviewed extracted-file identity is missing")?;
                crate::domain::system::trash::trash_checked(&path, move |candidate| {
                    if Identity::read(candidate)? != expected {
                        bail!("The extracted object changed before Trash");
                    }
                    Ok(())
                })
                .await
            }
        },
        on_line,
    )
    .await
}

async fn remove<F, Fut>(
    roots: &Roots,
    installation: &ExtractedInstallation,
    acknowledged: &[String],
    trash: F,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult
where
    F: Fn(PathBuf) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    if let Err(error) = verify_with_roots(roots, installation) {
        return failed(error.to_string());
    }
    if let Err(error) = crate::domain::app_data::processes::check_extracted(
        installation,
        &roots.home,
        &roots.processes,
        acknowledged,
    ) {
        return failed(error.to_string());
    }
    if let Err(error) = trash(installation.root.clone()).await {
        return failed(format!(
            "Could not move the extracted app to Trash: {error}"
        ));
    }
    if !missing(&installation.root) {
        return failed(
            "The installation path remains or cannot be verified after Trash; cleanup stopped."
                .into(),
        );
    }
    let mut warnings = Vec::new();
    on_line(&format!(
        "[scope] moved {} to Trash",
        installation.root.display()
    ));
    for launcher in &installation.launchers {
        let expected = installation
            .identities
            .iter()
            .find(|(path, _)| path == launcher)
            .map(|(_, id)| id);
        if Identity::read(launcher).ok().as_ref() != expected {
            warnings.push(format!(
                "Skipped {}: launcher changed during uninstall",
                launcher.display()
            ));
            continue;
        }
        match trash(launcher.clone()).await {
            Err(error) => warnings.push(format!("{}: {error}", launcher.display())),
            Ok(()) if !missing(launcher) => warnings.push(format!(
                "{} remains or cannot be verified after Trash",
                launcher.display()
            )),
            Ok(()) => {}
        }
    }
    for warning in &warnings {
        on_line(warning);
    }
    OperationResult {
        success: true,
        message: "The extracted app was moved to Trash.".into(),
        logs: String::new(),
        exit_code: Some(0),
        warnings,
        cleanup: Vec::new(),
    }
}

fn missing(path: &Path) -> bool {
    matches!(path.symlink_metadata(), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
}

fn failed(message: String) -> OperationResult {
    OperationResult {
        success: false,
        message,
        logs: String::new(),
        exit_code: None,
        warnings: Vec::new(),
        cleanup: Vec::new(),
    }
}

#[cfg(test)]
mod tests;
