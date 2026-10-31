//! Bounded same-user writer checks. No process is launched or terminated.
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::identity::Roots;
use crate::domain::package::{InstalledPackage, PackageSource};
use anyhow::{bail, Result};

#[derive(Default)]
pub(super) struct Inspection {
    pub running: bool,
    pub hidden: Vec<String>,
    pub error: Option<String>,
}

pub(super) fn assess(pkg: &InstalledPackage, captured: &[PathBuf], roots: &Roots) -> Inspection {
    assess_at(pkg, captured, &roots.home, &roots.processes)
}

fn assess_at(
    pkg: &InstalledPackage,
    captured: &[PathBuf],
    home: &Path,
    processes: &Path,
) -> Inspection {
    let mut expected = captured.to_vec();
    if let Some(app) = &pkg.appimage_installation {
        expected.push(app.path.clone());
    }
    if let Some(app) = &pkg.extracted_installation {
        expected.push(app.executable.clone());
    }
    if let Some(tool) = &pkg.tool_installation {
        expected.extend(tool.executables.iter().cloned());
    }
    if expected.is_empty() && !matches!(pkg.source, PackageSource::Flatpak | PackageSource::Snap) {
        return Inspection {
            error: Some("The app's process identity is unavailable; data was retained.".into()),
            ..Default::default()
        };
    }
    match inspect(pkg, &expected, home, processes) {
        Ok(inspection) => inspection,
        Err(error) => Inspection {
            error: Some(format!(
                "Running-app inspection is incomplete; data was retained: {error}"
            )),
            ..Default::default()
        },
    }
}

pub(crate) fn check_extracted(
    installation: &crate::domain::extracted_app::ExtractedInstallation,
    home: &Path,
    processes: &Path,
    acknowledged: &[String],
) -> Result<()> {
    let mut package = InstalledPackage::new(
        PackageSource::Desktop,
        installation.launcher.to_string_lossy(),
    );
    package.extracted_installation = Some(Box::new(installation.clone()));
    if let Some(reason) = check(&assess_at(&package, &[], home, processes), acknowledged) {
        bail!("{reason}");
    }
    Ok(())
}

pub(super) fn check_current(
    pkg: &InstalledPackage,
    captured: &[PathBuf],
    roots: &Roots,
    acknowledged: &[String],
) -> Option<String> {
    let state = assess(pkg, captured, roots);
    check(&state, acknowledged)
}

fn check(state: &Inspection, acknowledged: &[String]) -> Option<String> {
    if state.running {
        return Some("The app is still running. Close it before removing its data.".into());
    }
    if let Some(error) = &state.error {
        return Some(error.clone());
    }
    if state
        .hidden
        .iter()
        .any(|process| !acknowledged.contains(process))
    {
        return Some("Running-app inspection is incomplete or changed. Preview again and review the background-process warning before selecting data.".into());
    }
    None
}

#[cfg(test)]
fn running_writer(pkg: &InstalledPackage, captured: &[PathBuf], roots: &Roots) -> Option<String> {
    check_current(pkg, captured, roots, &[])
}

fn inspect(
    pkg: &InstalledPackage,
    expected: &[PathBuf],
    home: &Path,
    processes: &Path,
) -> Result<Inspection> {
    let owner = std::fs::metadata(home)?.uid();
    let started = Instant::now();
    let mut state = Inspection::default();
    for (count, entry) in std::fs::read_dir(processes)?.enumerate() {
        if count > 20_000 || started.elapsed() > Duration::from_secs(2) {
            bail!("Process inspection exceeded its bound");
        }
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let process = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.uid() != owner {
            continue;
        }
        match inspect_process(&process, pkg, expected) {
            Ok(true) => {
                state.running = true;
                return Ok(state);
            }
            Ok(false) => {}
            Err(_) if !process.exists() => continue,
            Err(_) => {
                // Bind a displayed inspection gap to PID + kernel start time.
                // PID reuse or a newly hidden process must require a new preview.
                let stat = String::from_utf8(read(&process.join("stat"), 64 * 1024)?)?;
                let rest = stat
                    .rsplit_once(')')
                    .ok_or_else(|| anyhow::anyhow!("Malformed process identity"))?
                    .1;
                let started: u64 = rest
                    .split_whitespace()
                    .nth(19)
                    .ok_or_else(|| anyhow::anyhow!("Missing process start time"))?
                    .parse()?;
                state
                    .hidden
                    .push(format!("{}:{started}", entry.file_name().to_string_lossy()));
            }
        }
    }
    state.hidden.sort();
    state.hidden.dedup();
    Ok(state)
}

fn inspect_process(process: &Path, pkg: &InstalledPackage, expected: &[PathBuf]) -> Result<bool> {
    let exe = match std::fs::read_link(process.join("exe")) {
        Ok(exe) => exe,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let status = read(&process.join("status"), 64 * 1024)?;
            if status
                .split(|byte| *byte == b'\n')
                .any(|line| line.starts_with(b"State:\tZ"))
            {
                return Ok(false);
            }
            return Err(error.into());
        }
        Err(error) => return Err(error.into()),
    };
    let normalized = exe
        .to_string_lossy()
        .trim_end_matches(" (deleted)")
        .to_string();
    if matches_path(Path::new(&normalized), pkg, expected) {
        return Ok(true);
    }
    let cmdline = read(&process.join("cmdline"), 64 * 1024)?;
    use std::os::unix::ffi::OsStrExt as _;
    if cmdline.split(|byte| *byte == 0).any(|argument| {
        matches_path(
            Path::new(std::ffi::OsStr::from_bytes(argument)),
            pkg,
            expected,
        )
    }) {
        return Ok(true);
    }
    let env = read(&process.join("environ"), 1024 * 1024)?;
    // Compare only exact identity keys. Never retain or log environments.
    for value in env.split(|byte| *byte == 0) {
        if let Some(value) = value.strip_prefix(b"APPIMAGE=") {
            if expected
                .iter()
                .any(|path| path.as_os_str().as_encoded_bytes() == value)
            {
                return Ok(true);
            }
        }
        if let Some(value) = value.strip_prefix(b"FLATPAK_ID=") {
            if pkg.source == PackageSource::Flatpak && pkg.package_id.as_bytes() == value {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn matches_path(path: &Path, pkg: &InstalledPackage, expected: &[PathBuf]) -> bool {
    expected.iter().any(|expected| path == expected)
        || (pkg.source == PackageSource::Snap
            && path.starts_with(Path::new("/snap").join(&pkg.package_id)))
        || pkg
            .extracted_installation
            .as_ref()
            .is_some_and(|app| path.starts_with(&app.root))
        || pkg
            .tool_installation
            .as_ref()
            .and_then(|tool| tool.environment.as_ref())
            .is_some_and(|root| path.starts_with(root))
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("Process metadata exceeds its bound");
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Roots, InstalledPackage, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let roots = Roots {
            home: home.clone(),
            config: home.join("config"),
            data: home.join("data"),
            cache: home.join("cache"),
            state: home.join("state"),
            processes: home.join("proc"),
        };
        let process = roots.processes.join("123");
        std::fs::create_dir_all(&process).unwrap();
        std::os::unix::fs::symlink("/usr/bin/unrelated", process.join("exe")).unwrap();
        std::fs::write(process.join("cmdline"), b"/usr/bin/unrelated\0").unwrap();
        std::fs::write(process.join("environ"), b"UNRELATED=1\0").unwrap();
        std::fs::write(
            process.join("stat"),
            format!("123 (fixture) {} 42", vec!["0"; 19].join(" ")),
        )
        .unwrap();
        let pkg = InstalledPackage::new(PackageSource::Apt, "paperling");
        (temp, roots, pkg, process)
    }

    #[test]
    fn exact_cmdline_and_appimage_environment_detect_mounted_executables() {
        let (_temp, roots, pkg, process) = fixture();
        let captured = vec![roots.home.join("paperling.appimage")];
        assert!(running_writer(&pkg, &captured, &roots).is_none());
        std::fs::write(
            process.join("environ"),
            format!("APPIMAGE={}\0", captured[0].display()),
        )
        .unwrap();
        assert!(running_writer(&pkg, &captured, &roots)
            .unwrap()
            .contains("still running"));
    }

    #[test]
    fn unreadable_or_oversized_process_metadata_keeps_data() {
        let (_temp, roots, pkg, process) = fixture();
        let captured = vec![roots.home.join("paperling.appimage")];
        std::fs::write(process.join("environ"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
        assert!(running_writer(&pkg, &captured, &roots)
            .unwrap()
            .contains("incomplete"));
        std::fs::remove_file(process.join("environ")).unwrap();
        assert!(running_writer(&pkg, &captured, &roots)
            .unwrap()
            .contains("incomplete"));
    }

    #[test]
    fn flatpak_environment_matches_the_exact_id_only() {
        let (_temp, roots, mut pkg, process) = fixture();
        pkg.source = PackageSource::Flatpak;
        pkg.package_id = "org.example.App".into();
        std::fs::write(
            process.join("environ"),
            b"FLATPAK_ID=org.example.AppSibling\0",
        )
        .unwrap();
        assert!(running_writer(&pkg, &[], &roots).is_none());
        std::fs::write(process.join("environ"), b"FLATPAK_ID=org.example.App\0").unwrap();
        assert!(running_writer(&pkg, &[], &roots)
            .unwrap()
            .contains("still running"));
    }

    #[test]
    fn snap_revision_processes_match_the_package_root_without_launcher_metadata() {
        let (_temp, roots, mut pkg, process) = fixture();
        pkg.source = PackageSource::Snap;
        pkg.package_id = "paperling".into();
        std::fs::remove_file(process.join("exe")).unwrap();
        std::os::unix::fs::symlink("/snap/paperling-helper/42/usr/bin/app", process.join("exe"))
            .unwrap();
        assert!(running_writer(&pkg, &[], &roots).is_none());
        std::fs::remove_file(process.join("exe")).unwrap();
        std::os::unix::fs::symlink("/snap/paperling/42/usr/bin/app", process.join("exe")).unwrap();
        assert!(running_writer(&pkg, &[], &roots)
            .unwrap()
            .contains("still running"));
    }
    #[test]
    fn exact_script_argument_detects_interpreter_launched_tools() {
        let (_temp, roots, pkg, process) = fixture();
        let tool = roots.home.join("tool/bin/paperling");
        let mut args = b"/usr/bin/python3\0".to_vec();
        args.extend_from_slice(tool.as_os_str().as_encoded_bytes());
        args.push(0);
        std::fs::write(process.join("cmdline"), args).unwrap();
        assert!(running_writer(&pkg, &[tool], &roots)
            .unwrap()
            .contains("still running"));
    }
    #[test]
    fn displayed_hidden_process_gap_does_not_acknowledge_a_reused_pid_or_known_writer() {
        let state = Inspection {
            hidden: vec!["123:42".into()],
            ..Default::default()
        };
        assert!(check(&state, &[]).is_some());
        assert!(check(&state, &["123:42".into()]).is_none());
        assert!(check(&state, &["123:41".into()]).is_some());
        assert!(check(
            &Inspection {
                running: true,
                ..state
            },
            &["123:42".into()]
        )
        .unwrap()
        .contains("still running"));
    }
}
