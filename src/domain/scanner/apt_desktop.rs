//! Installed APT owners of visible desktop launchers, including automatic installs.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::domain::desktop_entries::DesktopApp;
use crate::domain::package::{AppKind, InstalledPackage, PackageSource};
use crate::domain::system::{capture_output_env, capture_stdout, SCAN_TIMEOUT};

use super::ScanReport;

const SEP: char = '\x1f';

pub(super) async fn extend(report: &mut ScanReport, apps: &[&DesktopApp]) -> Result<()> {
    let mut owners = HashMap::new();
    for batch in apps.chunks(128) {
        let mut args = vec!["-S", "--"];
        let paths: Vec<_> = batch
            .iter()
            .map(|app| app.file_path.to_string_lossy())
            .collect();
        args.extend(paths.iter().map(|path| path.as_ref()));
        let output = capture_output_env("dpkg-query", &args, &[("LC_ALL", "C")], SCAN_TIMEOUT)
            .await
            .context("read desktop package ownership")?;
        // dpkg-query returns 1 when any launcher has no package owner.
        if !output.success && output.exit_code != Some(1) {
            bail!("Desktop package ownership failed: {}", output.stderr.trim());
        }
        let expected = batch.iter().map(|app| app.file_path.clone()).collect();
        for (package, paths) in parse_owners(&output.stdout, &expected) {
            owners.entry(package).or_insert_with(Vec::new).extend(paths);
        }
    }
    let existing: HashSet<_> = report
        .packages
        .iter()
        .map(|pkg| package_base(&pkg.package_id))
        .collect();
    let mut candidates: Vec<_> = owners
        .keys()
        .filter(|id| !existing.contains(package_base(id)))
        .cloned()
        .collect();
    candidates.sort();
    if candidates.is_empty() {
        return Ok(());
    }
    let automatic = capture_stdout("apt-mark", &["showauto"], SCAN_TIMEOUT)
        .await
        .context("read automatically installed packages")?;
    let automatic: HashSet<_> = automatic.lines().map(str::trim).collect();
    for batch in candidates.chunks(128) {
        let format = format!("${{binary:Package}}{SEP}${{Version}}{SEP}${{Installed-Size}}{SEP}${{binary:Summary}}{SEP}${{db:Status-Status}}\\n");
        let option = format!("-f={format}");
        let mut args = vec!["-W", &option, "--"];
        args.extend(batch.iter().map(String::as_str));
        let output = capture_output_env("dpkg-query", &args, &[("LC_ALL", "C")], SCAN_TIMEOUT)
            .await
            .context("read installed desktop packages")?;
        if !output.success {
            bail!("Desktop package metadata failed: {}", output.stderr.trim());
        }
        report
            .packages
            .extend(parse_packages(&output.stdout, &owners, apps, &automatic));
    }
    Ok(())
}

fn parse_owners(output: &str, expected: &HashSet<PathBuf>) -> HashMap<String, Vec<PathBuf>> {
    let mut owners: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for line in output.lines() {
        let Some((packages, path)) = line.split_once(": ") else {
            continue;
        };
        let path = PathBuf::from(path);
        if !expected.contains(&path) {
            continue;
        }
        for package in packages
            .split(", ")
            .filter(|package| valid_package(package))
        {
            owners
                .entry(package.to_owned())
                .or_default()
                .push(path.clone());
        }
    }
    owners
}

fn package_base(id: &str) -> &str {
    id.split_once(':').map_or(id, |(name, _)| name)
}

fn valid_package(name: &str) -> bool {
    name.as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"+.-:".contains(&b))
}

fn parse_packages(
    output: &str,
    owners: &HashMap<String, Vec<PathBuf>>,
    apps: &[&DesktopApp],
    automatic: &HashSet<&str>,
) -> Vec<InstalledPackage> {
    let mut packages = Vec::new();
    let mut seen = HashSet::new();
    for line in output.lines() {
        let fields: Vec<_> = line.split(SEP).collect();
        let [id, version, size, summary, "installed"] = fields.as_slice() else {
            continue;
        };
        let Some(paths) = owners.get(*id) else {
            continue;
        };
        let Some(app) = apps.iter().find(|app| paths.contains(&app.file_path)) else {
            continue;
        };
        if !seen.insert(*id) {
            continue;
        }
        let mut pkg = InstalledPackage::new(PackageSource::Apt, *id);
        pkg.name = (*id).into();
        pkg.display_name = Some(app.name.clone());
        pkg.description = app.comment.clone().or_else(|| Some((*summary).into()));
        pkg.version = (*version).into();
        pkg.size_bytes = size.parse::<u64>().unwrap_or(0).saturating_mul(1024);
        pkg.apt_automatic =
            automatic.contains(id) || automatic.contains(id.split(':').next().unwrap_or(id));
        pkg.app_kind = if app.terminal {
            AppKind::Cli
        } else {
            AppKind::Gui
        };
        pkg.terminal = app.terminal;
        packages.push(pkg);
    }
    packages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_manual_packages_and_qualified_owners_share_an_identity() {
        assert_eq!(package_base("demo:amd64"), package_base("demo"));
    }

    #[test]
    fn ownership_accepts_only_exact_requested_paths_and_package_names() {
        let path = PathBuf::from("/usr/share/applications/test.desktop");
        let owners = parse_owners("demo:amd64: /usr/share/applications/test.desktop\nwrong: /other.desktop\ndiversion by other: /usr/share/applications/test.desktop\n", &HashSet::from([path.clone()]));
        assert_eq!(owners, HashMap::from([("demo:amd64".into(), vec![path])]));
    }

    #[test]
    fn desktop_inventory_requires_installed_state_and_retains_automatic_provenance() {
        let app = DesktopApp {
            id: "demo".into(),
            file_path: "/app.desktop".into(),
            name: "Demo".into(),
            comment: None,
            exec: "/bin/true".into(),
            executable: Some("/bin/true".into()),
            icon: None,
            categories: vec![],
            terminal: false,
            menu_visible: true,
        };
        let owners = HashMap::from([
            ("demo".into(), vec![app.file_path.clone()]),
            ("removed".into(), vec![app.file_path.clone()]),
        ]);
        let packages = parse_packages(
            "demo\x1f1\x1f2\x1fSummary\x1finstalled\nremoved\x1f1\x1f2\x1fOld\x1fconfig-files\n",
            &owners,
            &[&app],
            &HashSet::from(["demo"]),
        );
        assert_eq!(packages.len(), 1);
        assert!(packages[0].apt_automatic);
        assert_eq!(packages[0].display_name.as_deref(), Some("Demo"));
        assert_eq!(packages[0].size_bytes, 2048);
        assert!(crate::domain::listing::is_listable(&packages[0]));
        assert!(crate::domain::listing::can_uninstall(&packages[0]));
    }
}
