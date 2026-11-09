use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::{
    catalog,
    claims::Claims,
    identity::{self, Roots},
    CleanupAction, CleanupEffect, CleanupItem, CleanupPlan,
};
use crate::domain::{
    appimage,
    package::{InstalledPackage, PackageSource},
};

struct Candidate {
    path: PathBuf,
    label: String,
    reason: String,
    default_selected: bool,
}

pub(super) fn build(pkg: &InstalledPackage, roots: &Roots, claims: &Claims) -> CleanupPlan {
    let started = Instant::now();
    let mut plan = CleanupPlan {
        package: Some(Box::new(pkg.clone())),
        roots: Some(roots.clone()),
        warnings: claims.warnings.clone(),
        ..Default::default()
    };
    if (matches!(
        pkg.source,
        PackageSource::Steam | PackageSource::JetBrainsToolbox
    ) || (pkg.source == PackageSource::Desktop && pkg.extracted_installation.is_none()))
        || (pkg.source == PackageSource::AppImage && pkg.appimage_installation.is_none())
    {
        plan.warnings.push(
            "This installation has no supported removal contract; personal data is retained."
                .into(),
        );
        return plan;
    }
    if let Some(rule) = catalog::rule(pkg) {
        plan.executables.extend(
            rule.commands
                .iter()
                .filter_map(|command| crate::domain::system::resolve(command))
                .map(|p| p.canonicalize().unwrap_or(p)),
        );
    }
    use std::os::unix::fs::PermissionsExt as _;
    for path in &claims.owned_paths {
        if std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        {
            plan.executables
                .push(path.canonicalize().unwrap_or_else(|_| path.clone()));
        }
    }
    for launcher in claims
        .launchers
        .iter()
        .filter(|launcher| claims.owned_paths.contains(&launcher.file_path))
    {
        if let Some(executable) = &launcher.executable {
            plan.executables.push(
                executable
                    .canonicalize()
                    .unwrap_or_else(|_| executable.clone()),
            );
        }
    }
    plan.executables.sort();
    plan.executables.dedup();
    let mut candidates = Vec::new();
    if pkg.source == PackageSource::Flatpak {
        if pkg.install_scope.is_none() || !valid_app_id(&pkg.package_id) {
            plan.warnings
                .push("Flatpak scope or application identity is missing; data is retained.".into());
            return plan;
        }
        candidates.push(Candidate { path: roots.home.join(".var/app").join(&pkg.package_id), label: "User data".into(), reason: "Flatpak's exact application-ID data directory. It may contain saved app content; review before selecting.".into(), default_selected: false });
        plan.warnings.push(
            "Files outside the Flatpak app data directory remain outside this cleanup coverage."
                .into(),
        );
    }
    if let Some(rule) = catalog::rule(pkg) {
        for namespace in rule.namespaces {
            let roots_to_search = match *namespace {
                "Code" => vec![(&roots.config, "User data")],
                "ruff" => vec![(&roots.cache, "Cache")],
                _ => vec![
                    (&roots.config, "User data"),
                    (&roots.data, "User data"),
                    (&roots.cache, "Cache"),
                    (&roots.state, "App state"),
                ],
            };
            for (base, label) in roots_to_search {
                candidates.push(Candidate {
                    path: base.join(namespace),
                    label: label.into(),
                    reason: rule.evidence.into(),
                    default_selected: !rule.sensitive
                        && matches!(
                            pkg.source,
                            PackageSource::Apt | PackageSource::AppImage | PackageSource::Desktop
                        ),
                });
            }
        }
    } else if pkg.source != PackageSource::Flatpak {
        plan.warnings
            .push("Exact-name matches only; other user-data locations are unknown.".into());
    }
    if let Some(app) = &pkg.appimage_installation {
        let app_roots = appimage::Roots::new(roots.home.clone(), roots.data.join("applications"));
        let bases = appimage::app_data::Bases {
            config: roots.config.clone(),
            data: roots.data.clone(),
            cache: roots.cache.clone(),
            state: roots.state.clone(),
        };
        let mut others = appimage::app_data::OtherLaunchers::default();
        for launcher in &claims.launchers {
            if !claims.owned_paths.contains(&launcher.file_path) {
                others.add(
                    &launcher.id,
                    &launcher.name,
                    launcher.executable.as_deref(),
                    launcher.icon.as_deref(),
                );
            }
        }
        for data in appimage::app_data::find_paths(&app_roots, &bases, app, &others) {
            candidates.push(Candidate { path: data.path, label: data.kind.label().into(), reason: "Exact launcher/name association; contents and exclusive app ownership require review.".into(), default_selected: false });
        }
        for (suffix, label) in [(".home", "Portable home"), (".config", "Portable data")] {
            let mut path = app.path.as_os_str().to_os_string();
            path.push(suffix);
            candidates.push(Candidate {
                path: PathBuf::from(path),
                label: label.into(),
                reason: "Exact AppImage portable sidecar; may contain personal files.".into(),
                default_selected: false,
            });
        }
    }
    if pkg.source != PackageSource::Flatpak
        && pkg.appimage_installation.is_none()
        && catalog::rule(pkg).is_none()
    {
        candidates.extend(associated_candidates(
            pkg,
            roots,
            claims,
            &mut plan.warnings,
        ));
    }
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    candidates.dedup_by(|a, b| a.path == b.path);
    let mut budget_warning = false;
    for (index, candidate) in candidates.into_iter().enumerate() {
        if index >= 128 || started.elapsed() > Duration::from_secs(15) {
            if !budget_warning {
                plan.warnings.push("App-data inspection exceeded its request budget; remaining candidates are unverified.".into());
                budget_warning = true;
            }
            plan.items
                .push(item(candidate, CleanupEffect::Unknown, None, None));
            continue;
        }
        match std::fs::symlink_metadata(&candidate.path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                plan.warnings.push(format!(
                    "Could not inspect {}: {error}",
                    candidate.path.display()
                ));
                plan.items
                    .push(item(candidate, CleanupEffect::Unknown, None, None));
            }
            Ok(_) => {
                let proof = identity::verify(&candidate.path, roots);
                let reason = if !claims.complete {
                    Some("Installation ownership enumeration is incomplete; retained.".into())
                } else {
                    claims.shared_reason(pkg, &candidate.path)
                };
                let (effect, identity) = match (proof, reason) {
                    (Ok(proof), None) => (CleanupEffect::Trash, Some(proof)),
                    (Ok(_), Some(reason)) => {
                        let mut candidate = candidate;
                        candidate.reason = reason;
                        plan.items.push(item(
                            candidate,
                            if claims.complete {
                                CleanupEffect::Kept
                            } else {
                                CleanupEffect::Unknown
                            },
                            None,
                            None,
                        ));
                        continue;
                    }
                    (Err(error), _) => {
                        let mut candidate = candidate;
                        candidate.reason = error.to_string();
                        plan.items
                            .push(item(candidate, CleanupEffect::Unknown, None, None));
                        continue;
                    }
                };
                let measured = crate::domain::package_details::measurement::measure(
                    &candidate.path,
                    &AtomicBool::new(false),
                );
                let bytes = measured.bytes.filter(|_| measured.complete);
                plan.items.push(item(candidate, effect, identity, bytes));
            }
        }
    }
    // Expose every kept/unknown candidate while removing overlapping authority.
    // A selected parent would implicitly authorize an unreviewed child selection.
    for i in 0..plan.items.len() {
        if plan.items[..i]
            .iter()
            .any(|p| plan.items[i].path.starts_with(&p.path))
        {
            plan.items[i].effect = CleanupEffect::Included;
            plan.items[i].default_selected = false;
            plan.items[i].reason =
                "Included in a parent data directory; review the parent instead.".into();
        }
    }
    if pkg.source == PackageSource::Flatpak {
        let shared = claims.shared_reason(pkg, std::path::Path::new(""));
        let effect = if !claims.complete {
            CleanupEffect::Unknown
        } else if shared.is_none() {
            CleanupEffect::Native
        } else {
            CleanupEffect::Kept
        };
        plan.items.push(CleanupItem {
            id: format!("flatpak-permissions:{}", pkg.package_id),
            label: "Saved Flatpak permissions".into(), path: PathBuf::new(), bytes: None,
            effect, default_selected: false,
            action: CleanupAction::FlatpakPermissions { app_id: pkg.package_id.clone() },
            reason: shared.unwrap_or_else(|| if claims.complete { "Reset this app's current-user portal permission records through Flatpak. Requires a separate selection.".into() } else { "Ownership enumeration is incomplete; saved permissions are retained.".into() }),
            identity: None,
        });
    }
    plan
}

fn associated_candidates(
    pkg: &InstalledPackage,
    roots: &Roots,
    claims: &Claims,
    warnings: &mut Vec<String>,
) -> Vec<Candidate> {
    let mut names = catalog::identities(pkg);
    for launcher in claims
        .launchers
        .iter()
        .filter(|app| claims.owned_paths.contains(&app.file_path))
    {
        names.push(launcher.id.clone());
        names.push(launcher.name.clone());
        if let Some(executable) = &launcher.executable {
            if let Some(name) = executable.file_name().and_then(|name| name.to_str()) {
                names.push(name.into());
            }
        }
        if let Ok(raw) = appimage::desktop::DesktopFile::read(&launcher.file_path) {
            if let Some(class) = raw.entry("StartupWMClass") {
                names.push(class.into());
            }
        }
    }
    let shared = [
        "applications",
        "autostart",
        "icons",
        "fonts",
        "flatpak",
        "snap",
        "appimages",
        "jetbrains",
        "steam",
        "node",
        "npm",
        "bun",
        "uv",
        "pipx",
        "cargo",
        "rustup",
        "python",
        "pip",
        "setuptools",
        "wheel",
        "nodejs",
        "pypoetry",
        "dconf",
        "gtk-3.0",
        "gtk-4.0",
        "keyrings",
        "trash",
        "gnome-shell",
        "systemd",
        "thumbnails",
        "sessions",
        "electron",
        "chromium",
        "mozilla",
        "google",
        "microsoft",
        "kde",
    ];
    names.retain(|name| {
        name.len() >= 3
            && !name.starts_with('.')
            && !name.contains(['/', '\\'])
            && !shared.contains(&name.to_lowercase().as_str())
    });
    let mut found = Vec::new();
    for (base, label) in [
        (&roots.config, "User data"),
        (&roots.data, "User data"),
        (&roots.cache, "Cache"),
        (&roots.state, "App state"),
    ] {
        let entries = match std::fs::read_dir(base) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                warnings.push(format!("Could not inspect {}: {error}", base.display()));
                continue;
            }
        };
        for (index, entry) in entries.enumerate() {
            if index >= 20_000 {
                warnings.push(format!(
                    "Discovery at {} exceeded its bound; coverage is incomplete.",
                    base.display()
                ));
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warnings.push(format!(
                        "Discovery at {} was incomplete: {error}",
                        base.display()
                    ));
                    continue;
                }
            };
            if names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&entry.file_name().to_string_lossy()))
            {
                found.push(Candidate { path: entry.path(), label: label.into(), reason: "Exact package, owned command or launcher-name association. Check the contents before selecting; the app has no reviewed path rule.".into(), default_selected: false });
            }
        }
    }
    found
}

fn item(
    candidate: Candidate,
    effect: CleanupEffect,
    identity: Option<identity::FileIdentity>,
    bytes: Option<u64>,
) -> CleanupItem {
    use sha2::{Digest as _, Sha256};
    let id = format!(
        "{:x}",
        Sha256::digest(candidate.path.as_os_str().as_encoded_bytes())
    );
    CleanupItem {
        action: CleanupAction::Trash,
        id,
        label: candidate.label,
        path: candidate.path,
        bytes,
        effect,
        default_selected: candidate.default_selected && effect == CleanupEffect::Trash,
        reason: candidate.reason,
        identity,
    }
}

fn valid_app_id(id: &str) -> bool {
    id.len() >= 3
        && id.contains('.')
        && !id.starts_with('.')
        && !id.ends_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}
