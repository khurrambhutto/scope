//! Disposable end-to-end AppImage install and removal.
//!
//! Ignored by default. Run with a real AppImage and a throwaway HOME so the
//! launcher, icon, and Trash all live in a temporary directory:
//!
//! ```bash
//! export HOME=$(mktemp -d) XDG_DATA_HOME=$HOME/.local/share
//! SCOPE_TEST_APPIMAGE=/path/App.AppImage cargo test appimage::live_tests -- --ignored
//! ```

use std::path::PathBuf;

use super::{install, managed, operations, Roots};

fn disposable_roots() -> Roots {
    let roots = Roots::current().unwrap();
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert!(
        roots.home.canonicalize().unwrap().starts_with(&temp),
        "refusing to run outside a temporary HOME"
    );
    roots
}

#[tokio::test]
#[ignore = "needs a temporary HOME and SCOPE_TEST_APPIMAGE"]
async fn install_then_uninstall_round_trip() {
    let roots = disposable_roots();
    let fixture = PathBuf::from(std::env::var_os("SCOPE_TEST_APPIMAGE").unwrap());
    let downloads = roots.home.join("Downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let opened = downloads.join(fixture.file_name().unwrap());
    std::fs::copy(&fixture, &opened).unwrap();

    let plan = install::preview(&opened).await.unwrap();
    assert!(plan.move_source);
    install::revalidate(&plan).unwrap();
    install::apply(&plan, &|line| println!("{line}"))
        .await
        .unwrap();
    assert!(!opened.exists(), "Gear Lever moves the opened file");
    println!("{}", std::fs::read_to_string(&plan.launcher).unwrap());
    if let Some(validate) = crate::domain::system::resolve("desktop-file-validate") {
        let out = std::process::Command::new(validate)
            .arg(&plan.launcher)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    let integration = managed::discover(&roots).remove(&plan.destination).unwrap();
    assert_eq!(integration.launcher, plan.launcher);
    let installation = managed::installation(&roots, &plan.destination, &integration).unwrap();
    managed::verify(&roots, &installation).unwrap();

    // Opening the same file again is recognised as already installed.
    std::fs::copy(&fixture, &opened).unwrap();
    assert!(install::preview(&opened).await.is_err());

    let result = operations::uninstall(&installation, &[], &|line| println!("{line}")).await;
    assert!(result.success, "{}", result.message);
    assert!(!plan.destination.exists());
    assert!(!plan.launcher.exists());
    assert!(plan.icon.as_ref().is_none_or(|icon| !icon.exists()));
    assert!(roots
        .home
        .join(".local/share/Trash/files")
        .read_dir()
        .unwrap()
        .next()
        .is_some());
}

/// Needs network and an AppImage whose `.upd_info` names a GitHub release,
/// for example Neovim's `nvim-linux-x86_64.appimage`.
#[tokio::test]
#[ignore = "needs a temporary HOME, network, and SCOPE_TEST_UPDATABLE_APPIMAGE"]
async fn detects_and_applies_a_published_update() {
    use std::io::Write as _;
    let roots = disposable_roots();
    let fixture = PathBuf::from(std::env::var_os("SCOPE_TEST_UPDATABLE_APPIMAGE").unwrap());
    let opened = roots.home.join(fixture.file_name().unwrap());
    std::fs::copy(&fixture, &opened).unwrap();
    let plan = install::preview(&opened).await.unwrap();
    assert!(
        plan.update_source.is_some(),
        "fixture has no update information"
    );
    install::apply(&plan, &|_| {}).await.unwrap();

    let installed = |roots: &Roots| {
        let integration = managed::discover(roots).remove(&plan.destination).unwrap();
        managed::installation(roots, &plan.destination, &integration).unwrap()
    };
    let current = installed(&roots);
    assert_eq!(super::updates::check(&current).await.unwrap(), None);

    // Make the installed copy differ from the published release.
    std::fs::OpenOptions::new()
        .append(true)
        .open(&plan.destination)
        .unwrap()
        .write_all(b"stale")
        .unwrap();
    let mut stale = installed(&roots);
    stale.update = super::updates::check(&stale).await.unwrap();
    let update = stale.update.clone().expect("an update should be detected");
    println!("update: {update:?}");

    let result = operations::update(&stale, &|line| println!("{line}")).await;
    assert!(result.success, "{}", result.message);
    let digests = super::hashes::compute(&plan.destination).unwrap();
    assert!(update.digest.matches(&digests));
    assert_eq!(
        super::updates::check(&installed(&roots)).await.unwrap(),
        None
    );
}
