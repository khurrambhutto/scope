use super::*;
use crate::domain::appimage::{elf, test_roots};

fn setup() -> (tempfile::TempDir, Roots, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let roots = test_roots(dir.path());
    let app = roots.appimages.join("demo_app.appimage");
    std::fs::write(&app, elf::test_appimage("", b"hsqs")).unwrap();
    (dir, roots, app)
}

fn launcher(roots: &Roots, name: &str, body: &str) -> PathBuf {
    let path = roots.applications.join(name);
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn finds_gear_lever_launchers_and_every_launcher_that_runs_the_file() {
    let (_dir, roots, app) = setup();
    let app_text = app.display().to_string();
    let primary = launcher(
        &roots,
        "demo_app.desktop",
        &format!("[Desktop Entry]\nType=Application\nName=Demo\nTryExec={app_text}\nExec=env DESKTOPINTEGRATION=1 {app_text} %U\nX-AppImage-Version=1.0\nX-AppImage-Name=Demo App\n"),
    );
    let extra = launcher(
        &roots,
        "demo-url.desktop",
        &format!("[Desktop Entry]\nType=Application\nName=Demo URL\nExec=\"{app_text}\" %U\n"),
    );
    launcher(
        &roots,
        "other.desktop",
        "[Desktop Entry]\nType=Application\nName=Other\nExec=/usr/bin/other\n",
    );
    std::fs::write(roots.icons().join("demo_app"), b"png").unwrap();
    std::fs::write(roots.icons().join("demo_app_2"), b"png").unwrap();

    let found = discover(&roots);
    let integration = &found[&app];
    assert_eq!(integration.launcher, primary);
    assert_eq!(integration.launchers, vec![extra, primary]);
    assert_eq!(integration.name.as_deref(), Some("Demo App"));
    assert_eq!(integration.version.as_deref(), Some("1.0"));

    let installation = installation(&roots, &app, integration).unwrap();
    assert_eq!(installation.icons, vec![roots.icons().join("demo_app")]);
    assert!(verify(&roots, &installation).is_ok());
}

#[test]
fn direct_exec_without_try_exec_is_an_integration() {
    let (_dir, roots, app) = setup();
    launcher(
        &roots,
        "demo.desktop",
        &format!(
            "[Desktop Entry]\nType=Application\nName=Demo\nExec={}\n",
            app.display()
        ),
    );
    let integration = discover(&roots).remove(&app).unwrap();
    let installation = installation(&roots, &app, &integration).unwrap();
    assert!(verify(&roots, &installation).is_ok());
}

#[test]
fn conflicting_try_exec_and_indirect_commands_are_not_integrations() {
    let (_dir, roots, app) = setup();
    for exec in [
        format!("TryExec={}\nExec=/usr/bin/other", app.display()),
        format!("TryExec=/usr/bin/other\nExec={}", app.display()),
        format!("Exec=sh -c {}", app.display()),
        "Exec=demo_app.appimage".into(),
    ] {
        launcher(
            &roots,
            "demo.desktop",
            &format!("[Desktop Entry]\nType=Application\nName=Demo\n{exec}\n"),
        );
        assert!(discover(&roots).is_empty(), "{exec}");
    }
}

#[test]
fn a_launcher_rewrite_can_drop_try_exec_but_cannot_retarget_exec() {
    let (_dir, roots, app) = setup();
    let primary = launcher(
        &roots,
        "demo.desktop",
        &format!(
            "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={0}\nExec={0}\n",
            app.display()
        ),
    );
    let integration = discover(&roots).remove(&app).unwrap();
    let installation = installation(&roots, &app, &integration).unwrap();
    std::fs::write(
        &primary,
        format!(
            "[Desktop Entry]\nType=Application\nName=Demo\nExec=\"{}\" --no-sandbox %U\n",
            app.display()
        ),
    )
    .unwrap();
    assert!(verify(&roots, &installation).is_ok());
    std::fs::write(
        &primary,
        format!(
            "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={}\nExec=/usr/bin/other\n",
            app.display()
        ),
    )
    .unwrap();
    assert!(verify(&roots, &installation).is_err());
}

#[test]
fn a_new_launcher_invalidates_the_preview_instead_of_leaving_it_broken() {
    let (_dir, roots, app) = setup();
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=Demo\nExec={}\n",
        app.display()
    );
    launcher(&roots, "demo.desktop", &body);
    let integration = discover(&roots).remove(&app).unwrap();
    let installation = installation(&roots, &app, &integration).unwrap();
    launcher(&roots, "demo-url.desktop", &body);
    assert!(verify(&roots, &installation).is_err());
}

#[test]
fn symlinked_launchers_and_icon_directories_are_not_removal_targets() {
    let (dir, roots, app) = setup();
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=Demo\nExec={}\n",
        app.display()
    );
    let outside = dir.path().join("other.desktop");
    std::fs::write(&outside, &body).unwrap();
    std::os::unix::fs::symlink(&outside, roots.applications.join("demo.desktop")).unwrap();
    assert!(discover(&roots).is_empty());
    std::fs::remove_file(roots.applications.join("demo.desktop")).unwrap();
    launcher(&roots, "demo.desktop", &body);
    std::fs::remove_dir(roots.icons()).unwrap();
    let outside_icons = dir.path().join("other-icons");
    std::fs::create_dir(&outside_icons).unwrap();
    std::fs::write(outside_icons.join("demo_app.png"), b"icon").unwrap();
    std::os::unix::fs::symlink(&outside_icons, roots.icons()).unwrap();
    let integration = discover(&roots).remove(&app).unwrap();
    let installation = installation(&roots, &app, &integration).unwrap();
    assert!(installation.icons.is_empty());
}

#[test]
fn appimages_outside_the_managed_folder_are_ignored() {
    let (dir, roots, _) = setup();
    let elsewhere = dir.path().join("Downloads/demo.appimage");
    launcher(
        &roots,
        "demo.desktop",
        &format!(
            "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={0}\nExec={0}\n",
            elsewhere.display()
        ),
    );
    assert!(discover(&roots).is_empty());
}

#[test]
fn verification_rejects_a_replaced_file_or_retargeted_launcher() {
    let (_dir, roots, app) = setup();
    let primary = launcher(
        &roots,
        "demo_app.desktop",
        &format!(
            "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={0}\nExec={0}\n",
            app.display()
        ),
    );
    let integration = discover(&roots).remove(&app).unwrap();
    let installation = installation(&roots, &app, &integration).unwrap();

    std::fs::write(
        &primary,
        "[Desktop Entry]\nType=Application\nName=Demo\nTryExec=/usr/bin/x\nExec=/usr/bin/x\n",
    )
    .unwrap();
    assert!(verify(&roots, &installation).is_err());

    std::fs::write(
        &primary,
        format!(
            "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={0}\nExec={0}\n",
            app.display()
        ),
    )
    .unwrap();
    assert!(verify(&roots, &installation).is_ok());
    std::fs::write(&app, elf::test_appimage("changed", b"hsqs-longer")).unwrap();
    assert!(verify(&roots, &installation).is_err());
}
