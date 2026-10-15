//! Opt-in checks never operate on an existing working tool installation.
use super::*;
use crate::domain::operations::{self as plans, Operation};

#[tokio::test]
#[ignore = "reads installed tool inventories and previews; no package changes"]
async fn live_inventory_and_previews() {
    for source in [PackageSource::Uv, PackageSource::Pipx, PackageSource::Cargo] {
        let mut packages = inventory(source).await.unwrap();
        let _ = check_updates(&mut packages).await;
        assert!(!packages.is_empty(), "{source:?}");
        for pkg in packages {
            validate(pkg.tool_installation.as_ref().unwrap()).unwrap();
            let plan = plans::uninstall::preview(&pkg).await.unwrap();
            assert!(!plan.requires_auth);
            if pkg.has_update {
                let update = plans::update::preview(&pkg).await.unwrap();
                assert_eq!(update.operation, Operation::Update);
            }
        }
    }
}

async fn execute(context: &ToolInstallation, program: &std::path::Path, args: &[&str]) {
    let env = context
        .env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect::<Vec<_>>();
    let output = system::capture_output_context(
        program.to_str().unwrap(),
        args,
        &env,
        Some(&home().unwrap()),
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    assert!(
        output.success,
        "command failed: {} {args:?}: {output:?}",
        program.display()
    );
}

fn seed(source: PackageSource, directory: &std::path::Path) -> ToolInstallation {
    let manager = system::resolve(manager_name(source)).unwrap();
    let root = directory.join(source.id());
    std::fs::create_dir_all(root.join("bin")).unwrap();
    let bin = directory.join(format!("{}-bin", source.id()));
    std::fs::create_dir_all(&bin).unwrap();
    let env = match source {
        PackageSource::Uv => BTreeMap::from([
            ("UV_TOOL_DIR".into(), root.display().to_string()),
            ("UV_TOOL_BIN_DIR".into(), bin.display().to_string()),
        ]),
        PackageSource::Pipx => BTreeMap::from([
            ("PIPX_HOME".into(), root.display().to_string()),
            ("PIPX_BIN_DIR".into(), bin.display().to_string()),
            (
                "PIPX_MAN_DIR".into(),
                directory.join("man").display().to_string(),
            ),
        ]),
        PackageSource::Cargo => BTreeMap::from([
            ("CARGO_INSTALL_ROOT".into(), root.display().to_string()),
            (
                "CARGO_HOME".into(),
                home().unwrap().join(".cargo").display().to_string(),
            ),
        ]),
        _ => unreachable!(),
    };
    ToolInstallation {
        source,
        manager,
        root,
        environment: None,
        executables: vec![],
        env,
        receipt_hash: String::new(),
        protection_reason: None,
        update: UpdateRecipe::Unsupported("seed".into()),
        notes: vec![],
    }
}

async fn apply_preview(pkg: &InstalledPackage, operation: Operation) {
    let plan = match operation {
        Operation::Update => plans::update::preview(pkg).await.unwrap(),
        Operation::Uninstall => plans::uninstall::preview(pkg).await.unwrap(),
        Operation::Install => unreachable!("tools have no install plans"),
    };
    let store = plans::PlanStore::default();
    store.issue(plan.clone());
    let plan = store.take(&plan.plan_id).unwrap();
    let probed = plans::probe::probe_target(&plan).await.unwrap();
    let result = match operation {
        Operation::Update => {
            plans::update::revalidate(&plan, &probed).unwrap();
            plans::update::apply(&plan, &|_| {}).await
        }
        Operation::Uninstall => {
            plans::uninstall::revalidate(&plan, &probed).unwrap();
            plans::uninstall::apply(&plan, &|_| {}).await
        }
        Operation::Install => unreachable!("tools have no install plans"),
    };
    assert!(result.success, "{}\n{}", result.message, result.logs);
    assert!(store.take(&plan.plan_id).is_none());
}

async fn wheel(context: &ToolInstallation, directory: &std::path::Path, version: &str) {
    // A dependency-free fixture wheel, created without building or importing app code.
    const SCRIPT: &str = r#"
import sys, pathlib, zipfile
root, version = pathlib.Path(sys.argv[1]), sys.argv[2]
name = 'scope_fixture_tool'
dist = f'{name}-{version}.dist-info'
files = {
 f'{name}.py': 'def main():\n    print("fixture")\n',
 f'{dist}/METADATA': f'Metadata-Version: 2.1\nName: scope-fixture-tool\nVersion: {version}\n',
 f'{dist}/WHEEL': 'Wheel-Version: 1.0\nGenerator: Scope test\nRoot-Is-Purelib: true\nTag: py3-none-any\n',
 f'{dist}/entry_points.txt': '[console_scripts]\nscope-fixture-command = scope_fixture_tool:main\nscope-fixture-helper = scope_fixture_tool:main\n',
}
files[f'{dist}/RECORD'] = ''.join(f'{p},,\n' for p in files) + f'{dist}/RECORD,,\n'
with zipfile.ZipFile(root / f'{name}-{version}-py3-none-any.whl', 'w') as archive:
 for path, content in files.items(): archive.writestr(path, content)
"#;
    execute(
        context,
        &system::resolve("python3").unwrap(),
        &["-c", SCRIPT, directory.to_str().unwrap(), version],
    )
    .await;
}

#[tokio::test]
#[ignore = "installs, upgrades, and uninstalls fixture wheels in private temporary uv/pipx homes; pipx may download its packaging libraries"]
async fn disposable_python_tools_update_and_uninstall() {
    let directory = tempfile::tempdir_in(home().unwrap()).unwrap();
    let wheels = directory.path().join("wheels");
    std::fs::create_dir(&wheels).unwrap();
    let uv = seed(PackageSource::Uv, directory.path());
    let pipx = seed(PackageSource::Pipx, directory.path());
    wheel(&uv, &wheels, "1.0.0").await;
    execute(
        &uv,
        &uv.manager,
        &[
            "tool",
            "install",
            "--no-index",
            "--find-links",
            wheels.to_str().unwrap(),
            "scope-fixture-tool<3",
        ],
    )
    .await;
    let pip_args = format!("--find-links={}", wheels.display());
    execute(
        &pipx,
        &pipx.manager,
        &[
            "install",
            &format!("--pip-args={pip_args}"),
            "scope-fixture-tool==1.0.0",
        ],
    )
    .await;
    wheel(&uv, &wheels, "2.0.0").await;
    for context in [uv, pipx] {
        let mut packages = inventory_recorded(&context).await.unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].version, "1.0.0");
        assert_eq!(
            packages[0]
                .tool_installation
                .as_ref()
                .unwrap()
                .executables
                .len(),
            2
        );
        let warning = updates::check(&mut packages).await;
        assert!(
            packages[0].has_update,
            "{:?} {:?} {warning:?}",
            context.source, packages[0].update_note
        );
        apply_preview(&packages[0], Operation::Update).await;
        let packages = inventory_recorded(&context).await.unwrap();
        assert_eq!(packages[0].version, "2.0.0");
        apply_preview(&packages[0], Operation::Uninstall).await;
        assert!(inventory_recorded(&context).await.unwrap().is_empty());
    }
}

#[tokio::test]
#[ignore = "builds a dependency-free fixture from a local Git repository in a private Cargo root"]
async fn disposable_cargo_git_tool_update_and_uninstall() {
    let directory = tempfile::tempdir_in(home().unwrap()).unwrap();
    let context = seed(PackageSource::Cargo, directory.path());
    let repo = directory.path().join("repository");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let git = system::resolve("git").unwrap();
    let repo_str = repo.to_str().unwrap();
    execute(&context, &git, &["init", "--initial-branch=main", repo_str]).await;
    std::fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scope-fixture-tool\"\nversion = \"1.0.0\"\nedition = \"2021\"\n[features]\nfixture = []\n").unwrap();
    std::fs::write(
        repo.join("src/main.rs"),
        "fn main() { println!(\"fixture\"); }\n",
    )
    .unwrap();
    execute(&context, &git, &["-C", repo_str, "add", "."]).await;
    execute(
        &context,
        &git,
        &[
            "-C",
            repo_str,
            "-c",
            "user.name=Scope test",
            "-c",
            "user.email=scope-test@localhost",
            "commit",
            "-m",
            "fixture v1",
        ],
    )
    .await;
    let url = url::Url::from_directory_path(&repo).unwrap().to_string();
    let root = context.root.to_str().unwrap();
    execute(
        &context,
        &context.manager,
        &[
            "install",
            "--git",
            &url,
            "--branch",
            "main",
            "--root",
            root,
            "--features",
            "fixture",
            "--bin",
            "scope-fixture-tool",
        ],
    )
    .await;
    let original = inventory_recorded(&context).await.unwrap();
    assert_eq!(original[0].version_label(), "1.0.0");
    let manifest = std::fs::read_to_string(repo.join("Cargo.toml"))
        .unwrap()
        .replace("1.0.0", "2.0.0");
    std::fs::write(repo.join("Cargo.toml"), manifest).unwrap();
    execute(&context, &git, &["-C", repo_str, "add", "."]).await;
    execute(
        &context,
        &git,
        &[
            "-C",
            repo_str,
            "-c",
            "user.name=Scope test",
            "-c",
            "user.email=scope-test@localhost",
            "commit",
            "-m",
            "fixture v2",
        ],
    )
    .await;
    let mut packages = inventory_recorded(&context).await.unwrap();
    assert!(updates::check(&mut packages).await.is_none());
    assert!(packages[0].has_update);
    apply_preview(&packages[0], Operation::Update).await;
    let packages = inventory_recorded(&context).await.unwrap();
    assert_eq!(packages[0].version_label(), "2.0.0");
    let UpdateRecipe::Cargo { options, .. } =
        &packages[0].tool_installation.as_ref().unwrap().update
    else {
        panic!("Cargo recipe expected");
    };
    assert!(options
        .windows(2)
        .any(|pair| pair == ["--features", "fixture"]));
    apply_preview(&packages[0], Operation::Uninstall).await;
    assert!(inventory_recorded(&context).await.unwrap().is_empty());
}
