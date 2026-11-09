use super::*;
use std::os::unix::fs::PermissionsExt as _;

struct Fixture {
    _temp: tempfile::TempDir,
    roots: Roots,
    launcher: PathBuf,
    root: PathBuf,
}
fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let root = home.join(".local/opt/Handy");
    let applications = home.join(".local/share/applications");
    let processes = temp.path().join("proc");
    for path in [
        root.join("usr/bin"),
        applications.clone(),
        processes.clone(),
    ] {
        std::fs::create_dir_all(path).unwrap();
    }
    std::fs::write(root.join("AppRun"), b"#!/bin/sh\nexec usr/bin/handy\n").unwrap();
    std::fs::set_permissions(root.join("AppRun"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(root.join("usr/bin/handy"), b"app").unwrap();
    std::fs::write(
        root.join("Handy.desktop"),
        "[Desktop Entry]\nType=Application\nName=Handy\nExec=handy\n",
    )
    .unwrap();
    let launcher = applications.join("handy.desktop");
    std::fs::write(
        &launcher,
        format!(
            "[Desktop Entry]\nType=Application\nName=Handy\nExec={}\nIcon={}\n",
            root.join("AppRun").display(),
            root.join("handy.png").display()
        ),
    )
    .unwrap();
    std::fs::create_dir(processes.join("self")).unwrap();
    std::fs::write(processes.join("self/mountinfo"), "").unwrap();
    Fixture {
        _temp: temp,
        roots: Roots {
            home,
            applications,
            other_applications: Vec::new(),
            processes,
        },
        launcher,
        root,
    }
}

#[test]
fn dedicated_appdir_is_discovered_without_including_downloads() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    assert_eq!(install.root, f.root);
    assert_eq!(install.launchers, vec![f.launcher]);
}

#[test]
fn arbitrary_local_executable_is_not_an_installation() {
    let f = fixture();
    std::fs::remove_file(f.root.join("Handy.desktop")).unwrap();
    assert!(discover_with_roots(&f.roots, &f.launcher).is_err());
}

#[test]
fn replaced_root_and_new_launcher_invalidate_preview() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    std::fs::copy(&f.launcher, f.roots.applications.join("second.desktop")).unwrap();
    assert!(verify_with_roots(&f.roots, &install).is_err());
}

#[test]
fn symlinked_root_is_rejected() {
    let f = fixture();
    let real = f.roots.home.join("Handy-real");
    std::fs::rename(&f.root, &real).unwrap();
    std::os::unix::fs::symlink(real, &f.root).unwrap();
    assert!(discover_with_roots(&f.roots, &f.launcher).is_err());
}

#[test]
fn foreign_launcher_claim_blocks_root_removal() {
    let mut f = fixture();
    let other = f._temp.path().join("system-applications");
    std::fs::create_dir(&other).unwrap();
    std::fs::copy(&f.launcher, other.join("handy.desktop")).unwrap();
    f.roots.other_applications.push(other);
    assert!(discover_with_roots(&f.roots, &f.launcher).is_err());
}

#[tokio::test]
async fn running_executable_blocks_uninstall() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    let process = f.roots.processes.join("123");
    std::fs::create_dir(&process).unwrap();
    std::os::unix::fs::symlink(f.root.join("usr/bin/handy"), process.join("exe")).unwrap();
    let result = remove(
        &f.roots,
        &install,
        &[],
        |_| async { panic!("running app reached Trash") },
        &|_| {},
    )
    .await;
    assert!(!result.success && result.message.contains("still running"));
}

#[tokio::test]
async fn uninstall_rejects_a_missing_review_before_touching_app_files() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    let result = uninstall(&install, &Default::default(), &|_| {}).await;
    assert!(!result.success && result.message.contains("reviewed uninstall plan"));
    assert!(f.root.exists() && f.launcher.exists());
}

#[tokio::test]
async fn only_previewed_protected_processes_can_be_acknowledged_for_app_files() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    let process = f.roots.processes.join("123");
    std::fs::create_dir(&process).unwrap();
    std::os::unix::fs::symlink("/usr/bin/unrelated", process.join("exe")).unwrap();
    std::fs::write(process.join("cmdline"), b"/usr/bin/unrelated\0").unwrap();
    std::fs::write(
        process.join("stat"),
        format!("123 (fixture) {} 42", vec!["0"; 19].join(" ")),
    )
    .unwrap();
    for gaps in [vec![], vec!["123:41".into()]] {
        let result = remove(
            &f.roots,
            &install,
            &gaps,
            |_| async { panic!("unreviewed process gap reached Trash") },
            &|_| {},
        )
        .await;
        assert!(!result.success && f.root.exists() && f.launcher.exists());
    }
    let trash = f._temp.path().join("Trash");
    std::fs::create_dir(&trash).unwrap();
    let result = remove(
        &f.roots,
        &install,
        &["123:42".into()],
        |path| {
            let trash = &trash;
            async move {
                std::fs::rename(&path, trash.join(path.file_name().unwrap()))?;
                Ok(())
            }
        },
        &|_| {},
    )
    .await;
    assert!(
        result.success && !f.root.exists() && !f.launcher.exists(),
        "{result:?}"
    );
}

#[tokio::test]
async fn uninstall_moves_tree_and_launcher_but_preserves_documents() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    let documents = f.roots.home.join("Documents");
    std::fs::create_dir(&documents).unwrap();
    std::fs::write(documents.join("notes.md"), b"keep").unwrap();
    let trash = f._temp.path().join("Trash");
    std::fs::create_dir(&trash).unwrap();
    let result = remove(
        &f.roots,
        &install,
        &[],
        |path| {
            let trash = &trash;
            async move {
                std::fs::rename(&path, trash.join(path.file_name().unwrap()))?;
                Ok(())
            }
        },
        &|_| {},
    )
    .await;
    assert!(result.success && result.warnings.is_empty(), "{result:?}");
    assert!(!f.root.exists());
    assert!(!f.launcher.exists());
    assert_eq!(std::fs::read(documents.join("notes.md")).unwrap(), b"keep");
    assert!(trash.join("Handy/usr/bin/handy").exists());
}

#[tokio::test]
async fn failed_launcher_move_is_reported_after_tree_removal() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    let trash = f._temp.path().join("Trash");
    std::fs::create_dir(&trash).unwrap();
    let result = remove(
        &f.roots,
        &install,
        &[],
        |path| {
            let trash = &trash;
            let launcher = &f.launcher;
            async move {
                if &path == launcher {
                    bail!("fixture failure");
                }
                std::fs::rename(&path, trash.join(path.file_name().unwrap()))?;
                Ok(())
            }
        },
        &|_| {},
    )
    .await;
    assert!(result.success && !f.root.exists() && f.launcher.exists());
    assert!(result.warnings[0].contains("fixture failure"));
}
#[test]
fn same_device_mount_under_installation_is_rejected() {
    let f = fixture();
    std::fs::write(
        f.roots.processes.join("self/mountinfo"),
        format!(
            "12 11 0:1 / {} rw - tmpfs tmpfs rw\n",
            f.root.join("usr").display()
        ),
    )
    .unwrap();
    assert!(discover_with_roots(&f.roots, &f.launcher).is_err());
}

#[test]
fn launcher_action_claiming_tree_blocks_removal() {
    let f = fixture();
    std::fs::write(f.roots.applications.join("editor.desktop"), format!("[Desktop Entry]\nType=Application\nName=Other\nExec=/usr/bin/editor\n[Desktop Action open]\nExec={}\n", f.root.join("AppRun").display())).unwrap();
    assert!(discover_with_roots(&f.roots, &f.launcher).is_err());
}

#[tokio::test]
async fn successful_trash_status_without_absence_is_not_success() {
    let f = fixture();
    let install = discover_with_roots(&f.roots, &f.launcher).unwrap();
    let result = remove(&f.roots, &install, &[], |_| async { Ok(()) }, &|_| {}).await;
    assert!(!result.success && f.root.exists() && f.launcher.exists());
}

#[tokio::test]
#[ignore = "read-only live metadata; needs SCOPE_VERIFY_EXTRACTED_DESKTOP"]
async fn readonly_live_extracted_installation_discovery() {
    let path = PathBuf::from(
        std::env::var_os("SCOPE_VERIFY_EXTRACTED_DESKTOP")
            .expect("explicit launcher path required"),
    );
    let installation = discover(&path).unwrap();
    println!(
        "Live read-only extracted candidate: {} at {}, {} verified launchers",
        installation.name,
        installation.root.display(),
        installation.launchers.len()
    );
    verify(&installation).unwrap();
    let mut package = crate::domain::package::InstalledPackage::new(
        crate::domain::package::PackageSource::Desktop,
        path.to_string_lossy(),
    );
    package.extracted_installation = Some(Box::new(installation));
    let plan = crate::domain::app_data::preview(&package).await;
    for warning in &plan.warnings {
        println!("Coverage: {warning}");
    }
    match crate::domain::app_data::preflight(&plan, &[]) {
        Ok(()) => println!("Live read-only extracted preflight: ready"),
        Err(error) => println!("Live read-only extracted preflight: {error}"),
    }
}
