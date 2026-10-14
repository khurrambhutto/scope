//! Opt-in native-manager coverage runs in a subprocess with a private HOME and local registry.
use super::*;
use crate::domain::operations::{self as plans, Operation};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

#[tokio::test]
#[ignore = "reads existing global inventories and builds previews; no package changes"]
async fn live_global_inventory_and_previews() {
    for source in [PackageSource::Npm, PackageSource::Bun] {
        let packages = super::super::inventory(source).await.unwrap();
        for package in packages {
            let installation = package.tool_installation.as_ref().unwrap();
            if installation.protection_reason.is_some()
                || crate::domain::safety::check_package(source, &package.package_id).protected
            {
                continue;
            }
            let live = super::super::inventory_recorded(installation)
                .await
                .unwrap();
            let found = live.iter().find(|live| live.key == package.key).unwrap();
            assert_eq!(
                found.tool_installation.as_ref().unwrap().receipt_hash,
                installation.receipt_hash,
                "{}",
                package.package_id
            );
            let plan = plans::uninstall::preview(&package).await.unwrap();
            assert!(!plan.requires_auth);
            assert!(!plan.protected);
        }
    }
}

struct Registry(std::process::Child);
impl Drop for Registry {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
#[ignore = "installs, updates, and removes fixtures only in private roots using a local registry"]
async fn disposable_global_javascript_update_and_uninstall() {
    if let Ok(source) = std::env::var("SCOPE_JS_FIXTURE_CHILD") {
        child_lifecycle(if source == "npm" {
            PackageSource::Npm
        } else {
            PackageSource::Bun
        })
        .await;
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("version");
    std::fs::write(&state, "1.0.0").unwrap();
    let python = crate::domain::system::resolve("python3").unwrap();
    let mut registry = Registry(
        Command::new(python)
            .args(["-u", "-c", REGISTRY])
            .arg(&state)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    BufReader::new(registry.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let address = address.trim();
    assert!(address.starts_with("http://127.0.0.1:"));
    for source in ["npm", "bun"] {
        std::fs::write(&state, "1.0.0").unwrap();
        let user_home = directory.path().join(source);
        let root = user_home.join("global");
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::write(root.join("package.json"), "{}").unwrap();
        std::fs::write(user_home.join("package.json"), "{}").unwrap();
        std::fs::create_dir_all(user_home.join("config")).unwrap();
        let npmrc = format!(
            "registry={address}\ncache={}\nprefer-online=true\naudit=false\nfund=false\n",
            user_home.join("cache").display()
        );
        std::fs::write(user_home.join(".npmrc"), npmrc).unwrap();
        std::fs::write(user_home.join(".bunfig.toml"), format!("[install]\nregistry = '{address}'\n[install.cache]\ndir = '{}'\ndisableManifest = true\n", user_home.join("cache").display())).unwrap();
        std::fs::copy(
            user_home.join(".bunfig.toml"),
            user_home.join("config/.bunfig.toml"),
        )
        .unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "domain::tool_managers::javascript::live_tests::disposable_global_javascript_update_and_uninstall", "--ignored", "--nocapture"])
            .env("HOME", &user_home).env("XDG_CONFIG_HOME", user_home.join("config"))
            .env("SCOPE_JS_FIXTURE_CHILD", source).env("SCOPE_JS_FIXTURE_STATE", &state)
            .env("NPM_CONFIG_PREFIX", &root).env("NPM_CONFIG_USERCONFIG", user_home.join(".npmrc"))
            .env("NPM_CONFIG_GLOBALCONFIG", user_home.join("unused-npmrc"))
            .env("BUN_INSTALL_GLOBAL_DIR", &root).env("BUN_INSTALL_BIN", root.join("bin"))
            .current_dir(&user_home).status().unwrap();
        assert!(status.success(), "{source} fixture failed");
    }
}

async fn child_lifecycle(source: PackageSource) {
    let manager = crate::domain::system::resolve(super::super::manager_name(source)).unwrap();
    let context = super::super::javascript_config::discover(source, &manager)
        .await
        .unwrap();
    let env = context
        .env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect::<Vec<_>>();
    let args: &[&str] = if source == PackageSource::Npm {
        &["install", "--global", "@scope/scope-global-fixture@1.0.0"]
    } else {
        &["add", "--global", "@scope/scope-global-fixture@^1.0.0"]
    };
    let result = crate::domain::system::capture_output_context(
        manager.to_str().unwrap(),
        args,
        &env,
        Some(&context.root),
        Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert!(result.success, "{}", result.stderr);
    let mut packages = super::super::inventory(source).await.unwrap();
    assert_eq!(packages.len(), 1);
    let mut package = packages.remove(0);
    assert_eq!(package.version, "1.0.0");
    super::super::validate(package.tool_installation.as_ref().unwrap()).unwrap();
    assert_eq!(
        super::super::javascript_updates::candidate(
            &package,
            package.tool_installation.as_ref().unwrap()
        )
        .await
        .unwrap(),
        None
    );
    let stale = plans::uninstall::preview(&package).await.unwrap();
    use std::io::Write;
    writeln!(
        std::fs::OpenOptions::new()
            .append(true)
            .open(home().unwrap().join(".npmrc"))
            .unwrap(),
        "# settings changed after preview"
    )
    .unwrap();
    assert!(plans::probe::probe_target(&stale).await.is_err());
    package = super::super::inventory(source).await.unwrap().remove(0);
    std::fs::write(std::env::var_os("SCOPE_JS_FIXTURE_STATE").unwrap(), "1.1.0").unwrap();
    let candidate = super::super::javascript_updates::candidate(
        &package,
        package.tool_installation.as_ref().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(candidate, Some("1.1.0".into()));
    package.has_update = true;
    package.update_version = candidate;
    apply_preview(&package, Operation::Update).await;
    package = super::super::inventory(source).await.unwrap().remove(0);
    assert_eq!(package.version, "1.1.0");
    apply_preview(&package, Operation::Uninstall).await;
    assert!(super::super::inventory(source).await.unwrap().is_empty());
}

async fn apply_preview(package: &InstalledPackage, operation: Operation) {
    let plan = match operation {
        Operation::Update => plans::update::preview(package).await.unwrap(),
        Operation::Uninstall => plans::uninstall::preview(package).await.unwrap(),
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

const REGISTRY: &str = r#"
import base64, hashlib, io, json, pathlib, sys, tarfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import unquote
state = pathlib.Path(sys.argv[1])
tarballs = {}
for version in ['1.0.0', '1.1.0']:
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode='w:gz') as archive:
        files = {'package.json': json.dumps({'name': '@scope/scope-global-fixture', 'version': version, 'bin': {'scope-global-fixture': 'bin/main.js'}}).encode(), 'bin/main.js': b'#!/usr/bin/env node\nconsole.log("fixture");\n'}
        for name, data in files.items():
            entry = tarfile.TarInfo('package/' + name); entry.size = len(data); entry.mode = 0o755
            archive.addfile(entry, io.BytesIO(data))
    tarballs[version] = output.getvalue()
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def do_GET(self):
        path = unquote(self.path).split('?')[0]
        if path.startswith('/tarballs/'):
            body = tarballs.get(path.rsplit('/', 1)[1].removesuffix('.tgz'))
            content_type = 'application/octet-stream'
        elif path.rstrip('/') == '/@scope/scope-global-fixture':
            latest = state.read_text().strip()
            versions = {}
            for version in ['1.0.0', '1.1.0']:
                if version > latest: continue
                tarball = tarballs[version]
                versions[version] = {'name': '@scope/scope-global-fixture', 'version': version, 'bin': {'scope-global-fixture': 'bin/main.js'}, 'dist': {'tarball': f'http://127.0.0.1:{self.server.server_port}/tarballs/{version}.tgz', 'shasum': hashlib.sha1(tarball).hexdigest(), 'integrity': 'sha512-' + base64.b64encode(hashlib.sha512(tarball).digest()).decode()}}
            body = json.dumps({'name': '@scope/scope-global-fixture', 'dist-tags': {'latest': latest}, 'versions': versions}).encode()
            content_type = 'application/json'
        else: body = None
        if body is None: self.send_error(404); return
        self.send_response(200); self.send_header('Content-Type', content_type); self.send_header('Content-Length', str(len(body))); self.send_header('Cache-Control', 'no-store'); self.end_headers(); self.wfile.write(body)
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
print(f'http://127.0.0.1:{server.server_port}', flush=True)
server.serve_forever()
"#;
