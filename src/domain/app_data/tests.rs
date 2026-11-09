use std::collections::HashSet;
use std::path::Path;

use super::*;
use crate::domain::package::{InstallScope, PackageSource};

struct Fixture {
    _dir: tempfile::TempDir,
    roots: Roots,
    pkg: InstalledPackage,
}
impl Fixture {
    fn new(source: PackageSource, id: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let roots = Roots {
            config: home.join(".config"),
            data: home.join(".local/share"),
            cache: home.join(".cache"),
            state: home.join(".local/state"),
            processes: home.join("private-proc"),
            home,
        };
        std::fs::create_dir_all(roots.processes.join("self")).unwrap();
        std::fs::write(roots.processes.join("self/mountinfo"), "").unwrap();
        for root in [&roots.config, &roots.data, &roots.cache, &roots.state] {
            std::fs::create_dir_all(root).unwrap();
        }
        let mut pkg = InstalledPackage::new(source, id);
        pkg.name = id.into();
        Self {
            _dir: dir,
            roots,
            pkg,
        }
    }
    fn profile(&self) -> PathBuf {
        let path = self.roots.config.join("com.saqla.marklite");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("history"), b"old recents").unwrap();
        path
    }
    fn plan(&self, claims: &claims::Claims) -> CleanupPlan {
        discovery::build(&self.pkg, &self.roots, claims)
    }
}
fn complete() -> claims::Claims {
    claims::Claims {
        complete: true,
        ..Default::default()
    }
}

#[test]
fn legacy_identity_is_shared_across_supported_package_sources() {
    for source in [
        PackageSource::Apt,
        PackageSource::Uv,
        PackageSource::Pipx,
        PackageSource::Cargo,
        PackageSource::Npm,
        PackageSource::Bun,
        PackageSource::Snap,
    ] {
        let f = Fixture::new(source, "paperling");
        let path = f.profile();
        let plan = f.plan(&complete());
        assert_eq!(plan.items.len(), 1, "{source:?}");
        assert_eq!(plan.items[0].path, path);
        assert_eq!(plan.items[0].effect, CleanupEffect::Trash);
        if source == PackageSource::Apt {
            assert_eq!(plan.default_selection(), vec![plan.items[0].id.clone()]);
        } else {
            assert!(
                plan.default_selection().is_empty(),
                "Host profiles of tool/Snap editions need separate review"
            );
        }
    }
}

#[test]
fn display_name_alone_never_authorizes_a_profile() {
    let mut f = Fixture::new(PackageSource::Apt, "unrelated-package");
    f.pkg.display_name = Some("Paperling".into());
    f.profile();
    assert!(f.plan(&complete()).items.is_empty());
}

#[test]
fn generic_apt_cleanup_captures_owned_executables_for_running_writer_checks() {
    use std::os::unix::fs::PermissionsExt as _;
    let f = Fixture::new(PackageSource::Apt, "example-editor");
    let executable = f.roots.home.join("bin/example-editor");
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    std::fs::write(&executable, b"fixture").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut claims = complete();
    claims.owned_paths.insert(executable.clone());
    let plan = f.plan(&claims);
    assert_eq!(plan.executables, vec![executable.canonicalize().unwrap()]);
}

#[test]
fn another_source_sharing_legacy_identity_retains_the_profile() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    f.profile();
    let mut claims = complete();
    claims
        .packages
        .push(InstalledPackage::new(PackageSource::Npm, "marklite"));
    let plan = f.plan(&claims);
    assert_eq!(plan.items[0].effect, CleanupEffect::Kept);
    assert!(plan.default_selection().is_empty());
    assert!(plan.items[0].reason.contains("npm"));
}

#[test]
fn incomplete_native_ownership_is_visible_and_not_selectable() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    f.profile();
    let claims = claims::Claims {
        warnings: vec!["Flatpak system inventory failed".into()],
        ..Default::default()
    };
    let plan = f.plan(&claims);
    assert!(!plan.warnings.is_empty());
    assert_eq!(plan.items[0].effect, CleanupEffect::Unknown);
    assert!(validate_selection(&plan, &[plan.items[0].id.clone()]).is_err());
}

#[test]
fn forged_duplicate_and_overlapping_selections_are_rejected() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    f.profile();
    let mut plan = f.plan(&complete());
    assert!(validate_selection(&plan, &["not-reviewed".into()]).is_err());
    let id = plan.items[0].id.clone();
    assert!(validate_selection(&plan, &[id.clone(), id.clone()]).is_err());
    let mut nested = plan.items[0].clone();
    nested.id = "child".into();
    nested.path = nested.path.join("history");
    plan.items.push(nested);
    assert!(validate_selection(&plan, &[id, "child".into()]).is_err());
}

#[test]
fn code_and_ruff_rules_do_not_expand_to_unverified_namespaces() {
    for (id, allowed) in [("code", "config"), ("ruff", "cache")] {
        let f = Fixture::new(PackageSource::Apt, id);
        let name = if id == "code" { "Code" } else { "ruff" };
        for root in [
            &f.roots.config,
            &f.roots.data,
            &f.roots.cache,
            &f.roots.state,
        ] {
            std::fs::create_dir(root.join(name)).unwrap();
        }
        let plan = f.plan(&complete());
        assert_eq!(plan.items.len(), 1);
        assert_eq!(
            plan.items[0].path,
            if allowed == "config" {
                f.roots.config.join(name)
            } else {
                f.roots.cache.join(name)
            }
        );
    }
}

#[test]
fn flatpak_dual_scope_keeps_shared_current_user_profile() {
    let mut f = Fixture::new(PackageSource::Flatpak, "org.example.Editor");
    f.pkg.install_scope = Some(InstallScope::User);
    f.pkg.key = "flatpak:user:org.example.Editor".into();
    std::fs::create_dir_all(f.roots.home.join(".var/app/org.example.Editor")).unwrap();
    let mut claims = complete();
    let mut other = f.pkg.clone();
    other.install_scope = Some(InstallScope::System);
    other.key = "flatpak:system:org.example.Editor".into();
    claims.packages.push(other);
    let plan = f.plan(&claims);
    assert_eq!(plan.items[0].effect, CleanupEffect::Kept);
    assert!(plan.items.iter().any(|item| matches!(
        item.action,
        CleanupAction::FlatpakPermissions { .. }
    ) && item.effect == CleanupEffect::Kept));
}

#[test]
fn missing_flatpak_scope_never_constructs_data_authority() {
    let f = Fixture::new(PackageSource::Flatpak, "org.example.Editor");
    std::fs::create_dir_all(f.roots.home.join(".var/app/org.example.Editor")).unwrap();
    let plan = f.plan(&complete());
    assert!(plan.items.is_empty());
    assert!(plan.warnings.iter().any(|w| w.contains("scope")));
}

#[test]
fn linked_ancestor_or_replaced_leaf_is_not_the_reviewed_identity() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let path = f.profile();
    let identity = identity::verify(&path, &f.roots).unwrap();
    std::fs::rename(&path, f.roots.home.join("old")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_ne!(identity::verify(&path, &f.roots).unwrap(), identity);
    std::fs::remove_dir(&path).unwrap();
    std::os::unix::fs::symlink(f.roots.home.join("old"), &path).unwrap();
    assert!(identity::verify(&path, &f.roots).is_err());
}

#[tokio::test]
async fn full_reviewed_cleanup_leaves_a_clean_profile_after_reinstall_and_preserves_documents() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let path = f.profile();
    let document = f.roots.home.join("document.md");
    std::fs::write(&document, b"keep my work").unwrap();
    let plan = f.plan(&complete());
    let selected = plan.default_selection();
    let trash = f.roots.home.join("private-trash");
    std::fs::create_dir(&trash).unwrap();
    let results = apply_with(
        &plan,
        &selected,
        &complete(),
        |path| {
            let expected = plan
                .items
                .iter()
                .find(|item| item.path == path)
                .unwrap()
                .identity
                .clone()
                .unwrap();
            let roots = f.roots.clone();
            let trash = trash.clone();
            async move {
                crate::domain::system::trash::trash_checked_into(&path, &trash, move |staged| {
                    anyhow::ensure!(
                        identity::verify(staged, &roots)? == expected,
                        "reviewed identity changed"
                    );
                    Ok(())
                })
                .await
            }
        },
        &|_| {},
    )
    .await;
    assert_eq!(results[0].status, CleanupStatus::Trashed);
    std::fs::create_dir(&path).unwrap();
    assert!(!path.join("history").exists());
    assert_eq!(std::fs::read(document).unwrap(), b"keep my work");
    assert_eq!(
        std::fs::read(trash.join("files/com.saqla.marklite/history")).unwrap(),
        b"old recents"
    );
}

#[tokio::test]
async fn opt_out_keeps_the_profile_and_produces_a_per_item_outcome() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let path = f.profile();
    let plan = f.plan(&complete());
    let results = apply_with(
        &plan,
        &[],
        &complete(),
        |_| async { panic!("unselected data must not reach Trash") },
        &|_| {},
    )
    .await;
    assert!(path.join("history").exists());
    assert_eq!(results[0].status, CleanupStatus::Kept);
}

#[tokio::test]
async fn failed_trash_retains_the_path_and_reports_the_failure() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let path = f.profile();
    let plan = f.plan(&complete());
    let results = apply_with(
        &plan,
        &plan.default_selection(),
        &complete(),
        |_| async { anyhow::bail!("fixture move denied") },
        &|_| {},
    )
    .await;
    assert!(path.exists());
    assert_eq!(results[0].status, CleanupStatus::Failed);
    assert!(results[0].message.contains("fixture move denied"));
}

#[tokio::test]
async fn replacement_or_new_claim_during_apply_retains_reviewed_data() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let path = f.profile();
    let plan = f.plan(&complete());
    let mut claims = complete();
    claims
        .packages
        .push(InstalledPackage::new(PackageSource::Cargo, "paperling"));
    let results = apply_with(
        &plan,
        &plan.default_selection(),
        &claims,
        |_| async { panic!("shared data must not reach Trash") },
        &|_| {},
    )
    .await;
    assert_eq!(results[0].status, CleanupStatus::Shared);
    assert!(path.exists());
    std::fs::rename(&path, f.roots.home.join("old")).unwrap();
    std::fs::create_dir(&path).unwrap();
    let results = apply_with(
        &plan,
        &plan.default_selection(),
        &complete(),
        |_| async { panic!("replaced data must not reach Trash") },
        &|_| {},
    )
    .await;
    assert_eq!(results[0].status, CleanupStatus::Changed);
}

#[tokio::test]
async fn running_or_uninspectable_writer_blocks_data_cleanup() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    f.profile();
    let plan = f.plan(&complete());
    let mut claims = complete();
    claims.writer = Some("Close the running app".into());
    let results = apply_with(
        &plan,
        &plan.default_selection(),
        &claims,
        |_| async { panic!("running writer") },
        &|_| {},
    )
    .await;
    assert_eq!(results[0].status, CleanupStatus::Unknown);
}

#[test]
fn native_inventory_retains_both_flatpak_scopes_and_detects_malformed_data() {
    let user = claims::parse_native(PackageSource::Flatpak, "org.example.App\n", "--user").unwrap();
    let system =
        claims::parse_native(PackageSource::Flatpak, "org.example.App\n", "--system").unwrap();
    assert_ne!(user[0].key, system[0].key);
    assert!(claims::parse_native(PackageSource::Apt, "not tab separated", "").is_err());
    let apt = claims::parse_native(
        PackageSource::Apt,
        "paperling\tinstalled\nold\tconfig-files\n",
        "",
    )
    .unwrap();
    assert_eq!(apt.len(), 2);
}

#[test]
fn hidden_launcher_is_still_a_claimant_independent_of_ui_visibility() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    f.profile();
    let launcher = f.roots.home.join("hidden.desktop");
    std::fs::write(
        &launcher,
        "[Desktop Entry]\nType=Application\nName=Paperling\nHidden=true\nExec=/other/paperling\n",
    )
    .unwrap();
    let mut claims = complete();
    claims
        .launchers
        .push(crate::domain::desktop_entries::parse_launcher(&launcher).unwrap());
    claims.owned_paths = HashSet::new();
    let plan = f.plan(&claims);
    assert_eq!(plan.items[0].effect, CleanupEffect::Kept);
    assert!(plan.items[0].reason.contains("launcher"));
}

#[test]
fn only_existing_reviewed_namespaces_are_offered_not_documents_or_similar_names() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let path = f.profile();
    std::fs::create_dir(f.roots.config.join("com.saqla.marklite-backup")).unwrap();
    std::fs::create_dir(f.roots.home.join("Paperling Documents")).unwrap();
    let plan = f.plan(&complete());
    assert_eq!(
        plan.items
            .iter()
            .map(|i| i.path.as_path())
            .collect::<Vec<&Path>>(),
        vec![path.as_path()]
    );
}

#[test]
fn unrelated_native_package_names_and_display_names_cannot_claim_a_legacy_profile() {
    for source in [PackageSource::Apt, PackageSource::Npm, PackageSource::Cargo] {
        for id in [
            "paperling-plugin2",
            "paperling.desktop",
            "paperling_2_helper",
        ] {
            let mut f = Fixture::new(source, id);
            f.profile();
            f.pkg.name = "paperling".into();
            f.pkg.display_name = Some("Paperling".into());
            let plan = f.plan(&complete());
            assert!(plan.default_selection().is_empty(), "{source:?} {id}");
            assert!(plan.items.is_empty(), "{source:?} {id}");
        }
    }
}

#[test]
fn same_device_bind_mount_inside_profile_prevents_cleanup_authority() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let profile = f.profile();
    let mount = profile.join("documents");
    std::fs::create_dir(&mount).unwrap();
    let encoded = mount.to_string_lossy().replace(' ', "\\040");
    std::fs::write(
        f.roots.processes.join("self/mountinfo"),
        format!("36 25 0:24 / {encoded} rw - ext4 /dev/fake rw\n"),
    )
    .unwrap();
    let plan = f.plan(&complete());
    assert_eq!(plan.items[0].effect, CleanupEffect::Unknown);
    assert!(plan.default_selection().is_empty());
    assert!(plan.items[0].reason.contains("mount"));
}

#[test]
fn unavailable_mount_inventory_cannot_authorize_profile_removal() {
    let f = Fixture::new(PackageSource::Apt, "paperling");
    f.profile();
    std::fs::remove_file(f.roots.processes.join("self/mountinfo")).unwrap();
    let plan = f.plan(&complete());
    assert_eq!(plan.items[0].effect, CleanupEffect::Unknown);
    assert!(plan.default_selection().is_empty());
}

#[test]
fn arbitrary_managed_sources_share_exact_name_review_without_automatic_deletion() {
    for source in [
        PackageSource::Apt,
        PackageSource::Snap,
        PackageSource::Uv,
        PackageSource::Pipx,
        PackageSource::Cargo,
        PackageSource::Npm,
        PackageSource::Bun,
    ] {
        let f = Fixture::new(source, "example-editor");
        let path = f.roots.config.join("example-editor");
        std::fs::create_dir(&path).unwrap();
        let plan = f.plan(&complete());
        assert_eq!(plan.items.len(), 1, "{source:?}");
        assert_eq!(plan.items[0].effect, CleanupEffect::Trash);
        assert!(plan.default_selection().is_empty());
        assert!(validate_selection(&plan, &[plan.items[0].id.clone()]).is_ok());
    }
}

#[test]
fn generic_name_association_retains_known_shared_manager_roots() {
    for id in ["cargo", "npm", "icons", "keyrings", "systemd"] {
        let f = Fixture::new(PackageSource::Apt, id);
        std::fs::create_dir(f.roots.config.join(id)).unwrap();
        assert!(f.plan(&complete()).items.is_empty(), "{id}");
    }
}

#[test]
fn app_named_namespace_cannot_remove_a_relocated_shared_xdg_root() {
    let mut f = Fixture::new(PackageSource::Apt, "paperling");
    let profile = f.profile();
    f.roots.data = profile.clone();
    let plan = f.plan(&complete());
    let item = plan.items.iter().find(|item| item.path == profile).unwrap();
    assert_eq!(item.effect, CleanupEffect::Unknown);
    assert!(item.reason.contains("shared XDG"));
}

#[tokio::test]
#[ignore = "read-only live metadata; needs SCOPE_VERIFY_APPIMAGE pointing at a managed Paperling AppImage"]
async fn readonly_live_preview_finds_paperling_legacy_profile() {
    let path = PathBuf::from(
        std::env::var_os("SCOPE_VERIFY_APPIMAGE").expect("explicit managed AppImage path required"),
    );
    let roots = crate::domain::appimage::Roots::current().unwrap();
    let integrations = crate::domain::appimage::managed::discover(&roots);
    let integration = integrations
        .get(&path)
        .expect("file has no verified integration");
    let installation =
        crate::domain::appimage::managed::installation(&roots, &path, integration).unwrap();
    let mut pkg = InstalledPackage::new(PackageSource::AppImage, path.to_string_lossy());
    pkg.name = integration
        .name
        .clone()
        .unwrap_or_else(|| "paperling".into());
    pkg.appimage_installation = Some(Box::new(installation));
    let plan = preview(&pkg).await;
    assert!(
        plan.items.iter().any(|item| item
            .path
            .file_name()
            .is_some_and(|name| name == "com.saqla.marklite")),
        "legacy profile was not discovered"
    );
    println!("Live read-only Paperling preview: {} reviewed items, {} default selections, {} coverage warnings", plan.items.len(), plan.default_selection().len(), plan.warnings.len());
    for item in &plan.items {
        println!(
            "Reviewed {:?}: {} — {}",
            item.effect, item.label, item.reason
        );
    }
    for warning in &plan.warnings {
        println!("Coverage: {warning}");
    }
    let explicit_choices: Vec<_> = plan
        .items
        .iter()
        .filter(|item| item.selectable())
        .map(|item| item.id.clone())
        .collect();
    match preflight(&plan, &explicit_choices) {
        Ok(()) => println!("Live read-only data preflight: ready"),
        Err(error) => println!("Live read-only data preflight: {error}"),
    }
}

#[test]
fn preview_futures_do_not_inline_all_manager_inventory_states() {
    fn size<F: std::future::Future>(_: impl FnOnce() -> F) -> usize {
        std::mem::size_of::<F>()
    }
    let f = Fixture::new(PackageSource::Apt, "paperling");
    let collected = size(|| claims::collect(&f.pkg, &f.roots));
    let cleanup = size(|| preview(&f.pkg));
    let uninstall = size(|| crate::domain::operations::uninstall::preview(&f.pkg));
    let plan = crate::domain::operations::OperationPlan {
        plan_id: "memory-fixture".into(),
        operation: crate::domain::operations::Operation::Uninstall,
        target: crate::domain::operations::OperationTarget::Snap {
            package_id: "fixture".into(),
        },
        display_name: "Fixture".into(),
        current_version: "1".into(),
        target_version: String::new(),
        requires_auth: false,
        auth_method: crate::domain::operations::AuthMethod::None,
        protected: false,
        protection_reason: None,
        steps: Vec::new(),
        created_at_ms: 0,
        apt_transaction_fingerprint: None,
        snap_details: None,
        cleanup: Default::default(),
    };
    let cleanup_apply = size(|| apply(&plan.cleanup, &[], &|_| {}));
    let uninstall_apply = size(|| crate::domain::operations::uninstall::apply(&plan, &|_| {}));
    println!("Apply future storage: cleanup={cleanup_apply}, uninstall={uninstall_apply}");
    println!(
        "Future storage: claims={collected}, data preview={cleanup}, uninstall preview={uninstall}"
    );
    assert!(
        uninstall < 64 * 1024,
        "Uninstall preview must not inline manager inventories"
    );
    assert!(
        uninstall_apply < 48 * 1024,
        "Uninstall apply must not inline manager inventories"
    );
    assert!(
        collected < 16 * 1024,
        "Ownership collection inlines too much state onto ordinary worker stacks"
    );
}
