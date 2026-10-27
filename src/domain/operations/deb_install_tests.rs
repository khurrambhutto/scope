use super::*;

#[test]
fn rejects_protected_archive_metadata() {
    assert!(metadata("systemd\n999\namd64\n").is_err());
}

#[test]
fn rejects_invalid_package_names() {
    for name in [
        "-option",
        "../escape",
        "app:amd64",
        "App",
        "a",
        "app*",
        "app name",
    ] {
        assert!(metadata(&format!("{name}\n1.0\nall\n")).is_err(), "{name}");
    }
}

#[test]
fn accepts_debian_names_and_epochs() {
    assert_eq!(
        metadata("demo-app+tools\n2:1.0-3\nall\n").unwrap().1,
        "2:1.0-3"
    );
}

#[test]
fn snapshot_keeps_original_and_detects_changed_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("a package.deb");
    std::fs::write(&source, "first").unwrap();
    let first = Archive::copy(&source).unwrap();
    std::fs::write(&source, "second").unwrap();
    let second = Archive::copy(&source).unwrap();
    assert_ne!(first.sha256, second.sha256);
    assert_eq!(std::fs::read_to_string(first.path).unwrap(), "first");
    assert!(source.exists());
}

#[test]
fn rejects_symlinked_archives() {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("target.deb");
    std::fs::write(&target, "archive").unwrap();
    let link = directory.path().join("link.deb");
    std::os::unix::fs::symlink(target, &link).unwrap();
    assert!(Archive::copy(&link).is_err());
}

#[tokio::test]
async fn changed_archive_is_rejected_before_querying_installed_state() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("changed.deb");
    std::fs::write(&source, "preview bytes").unwrap();
    let snapshot = Archive::copy(&source).unwrap();
    let plan = DebInstallPlan {
        source: source.clone(),
        sha256: snapshot.sha256,
        package_id: "scope-deb-fixture".into(),
        version: "1.0".into(),
        architecture: "all".into(),
        installed_version: None,
        metadata: Default::default(),
        size_bytes: 0,
        changes: vec![],
    };
    std::fs::write(source, "different bytes").unwrap();
    assert!(revalidate(&plan)
        .await
        .unwrap_err()
        .to_string()
        .contains("changed since preview"));
}

#[tokio::test]
async fn protection_is_rechecked_before_reading_archive() {
    let plan = DebInstallPlan {
        source: PathBuf::from("/does-not-exist.deb"),
        sha256: String::new(),
        package_id: "apt".into(),
        version: "1.0".into(),
        architecture: "all".into(),
        installed_version: None,
        metadata: Default::default(),
        size_bytes: 0,
        changes: vec![],
    };
    assert!(revalidate(&plan)
        .await
        .unwrap_err()
        .to_string()
        .contains("protected package"));
}

fn fixture(directory: &Path, version: &str) -> PathBuf {
    let root = directory.join(format!("root-{version}"));
    std::fs::create_dir_all(root.join("DEBIAN")).unwrap();
    std::fs::write(root.join("DEBIAN/control"), format!(
        "Package: scope-deb-fixture\nVersion: {version}\nArchitecture: all\nMaintainer: Scope Tests <test@example.invalid>\nHomepage: https://example.invalid/app\nDescription: Disposable archive fixture\n .\n A longer description.\nSection: utils\nPriority: optional\n"
    )).unwrap();
    // Inspection and simulation must never run this script.
    std::fs::write(root.join("DEBIAN/preinst"), "#!/bin/sh\nexit 99\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        root.join("DEBIAN/preinst"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let archive = directory.join(format!("a file with spaces-{version}.deb"));
    let output = std::process::Command::new("dpkg-deb")
        .args(["--build", "--root-owner-group"])
        .arg(root)
        .arg(&archive)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    archive
}

#[tokio::test]
async fn real_archive_metadata_and_isolated_apt_simulation_never_execute_scripts() {
    let directory = tempfile::tempdir().unwrap();
    let source = fixture(directory.path(), "1.0");
    let archive = Archive::copy(&source).unwrap();
    let output = query(
        "dpkg-deb",
        &[
            "--show",
            "--showformat=${Package}\n${Version}\n${Architecture}\n${Homepage}\n${Description}\n",
            archive.argument().unwrap(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        metadata(&output).unwrap(),
        ("scope-deb-fixture".into(), "1.0".into(), "all".into())
    );
    let app = app_metadata(&output);
    assert_eq!(app.homepage.as_deref(), Some("https://example.invalid/app"));
    assert_eq!(
        app.description.as_deref(),
        Some("Disposable archive fixture\n\nA longer description.")
    );
    let status = directory.path().join("status");
    std::fs::write(&status, "").unwrap();
    let lists_option = format!(
        "Dir::State::lists={}",
        directory.path().join("lists").display()
    );
    let status_option = format!("Dir::State::status={}", status.display());
    let output = query(
        "apt-get",
        &[
            "-s",
            "-o",
            &status_option,
            "-o",
            "Dir::Etc::sourcelist=/dev/null",
            "-o",
            "Dir::Etc::sourceparts=-",
            "-o",
            &lists_option,
            "-o",
            "Debug::NoLocking=1",
            "install",
            "-y",
            "--",
            archive.argument().unwrap(),
        ],
    )
    .await
    .unwrap();
    let transaction =
        apt_transaction::parse(&output, Operation::Install, "scope-deb-fixture").unwrap();
    assert_eq!(transaction.installs, vec!["scope-deb-fixture"]);
    assert!(transaction.upgrades.is_empty());
    assert_eq!(std::fs::read_to_string(status).unwrap(), "");
}

#[tokio::test]
async fn corrupted_archive_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("bad.deb");
    std::fs::write(&source, "not a Debian archive").unwrap();
    assert!(preview(&source).await.is_err());
}

#[test]
fn install_rejects_indirect_protected_upgrades_but_accepts_new_dependencies() {
    let upgraded = apt_transaction::parse(
        "Inst demo (1.0 local-deb [all])\nInst libssl3 [3.0] (3.1 Ubuntu [amd64])\n",
        Operation::Install,
        "demo",
    )
    .unwrap();
    assert!(validate_transaction(&upgraded).is_err());
    let dependency = apt_transaction::parse(
        "Inst demo (1.0 local-deb [all])\nInst libssl3 (3.1 Ubuntu [amd64])\n",
        Operation::Install,
        "demo",
    )
    .unwrap();
    assert!(validate_transaction(&dependency).is_ok());
}

#[test]
fn rejects_installed_state_changes_for_both_new_installs_and_upgrades() {
    assert!(validate_installed_state(None, Some("1.0")).is_err());
    assert!(validate_installed_state(Some("1.0"), None).is_err());
    assert!(validate_installed_state(Some("1.0"), Some("1.1")).is_err());
    assert!(validate_installed_state(None, None).is_ok());
    assert!(validate_installed_state(Some("1.0"), Some("1.0")).is_ok());
}

#[test]
fn compact_install_preview_retains_additional_apt_changes() {
    let transaction = apt_transaction::parse(
        "Inst demo (2.0 local-deb [all])\nInst helper (1.0 Ubuntu [all])\nInst editor [1.0] (2.0 Ubuntu [all])\nRemv old-editor [1.0]\n",
        Operation::Install,
        "demo",
    ).unwrap();
    assert_eq!(
        transaction_changes(&transaction, "demo"),
        vec![
            "Also installs: helper.",
            "Also upgrades: editor.",
            "Removes: old-editor.",
        ]
    );
}

#[test]
fn compact_install_preview_omits_the_target_from_additional_changes() {
    let transaction = apt_transaction::parse(
        "Inst demo [1.0] (2.0 local-deb [all])\n",
        Operation::Install,
        "demo",
    )
    .unwrap();
    assert!(transaction_changes(&transaction, "demo").is_empty());
}

#[tokio::test]
async fn install_target_rejects_update_and_uninstall_routes() {
    let install = DebInstallPlan {
        source: PathBuf::from("/not-an-installed-package.deb"),
        sha256: "digest".into(),
        package_id: "demo".into(),
        version: "2.0".into(),
        architecture: "all".into(),
        installed_version: Some("1.0".into()),
        metadata: Default::default(),
        size_bytes: 0,
        changes: vec![],
    };
    let plan = OperationPlan {
        plan_id: "fixture".into(),
        operation: Operation::Install,
        target: OperationTarget::DebInstall {
            plan: Box::new(install),
        },
        display_name: "demo".into(),
        current_version: "1.0".into(),
        target_version: "2.0".into(),
        requires_auth: true,
        auth_method: AuthMethod::Pkexec,
        protected: false,
        protection_reason: None,
        steps: vec![],
        created_at_ms: 0,
        apt_transaction_fingerprint: None,
        snap_details: None,
        app_data: vec![],
    };
    let state = probe::ProbedPackage {
        present: true,
        version: Some("1.0".into()),
        has_update: Some(true),
    };
    assert!(super::super::update::revalidate(&plan, &state).is_err());
    assert!(super::super::uninstall::revalidate(&plan, &state).is_err());
    assert!(!super::super::update::apply(&plan, &|_| {}).await.success);
    assert!(!super::super::uninstall::apply(&plan, &|_| {}).await.success);
    // The install route itself also rejects a missing transaction fingerprint.
    assert!(apply(&plan, &|_| {}).await.is_err());
}
