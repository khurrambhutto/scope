//! Reviewed associations. Every place is an exact path an app documents or was
//! observed to create, never a fuzzy match. A rule applies only to the install
//! types that write to the normal home directories; strict Snaps keep data in
//! `~/snap/<name>` and Flatpaks in `~/.var/app/<id>`.
use std::path::{Path, PathBuf};

use super::identity::Roots;
use crate::domain::package::{InstalledPackage, PackageSource};

mod apps;
mod browsers;
mod editors;
mod xdg_ninja;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Base {
    Home,
    Config,
    Data,
    Cache,
    State,
}

/// What a place holds. Caches regenerate; data can be personal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Data,
    Cache,
    State,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Data => "User data",
            Kind::Cache => "Cache",
            Kind::State => "App state",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Place {
    pub base: Base,
    pub path: &'static str,
    pub kind: Kind,
}

impl Place {
    pub fn resolve(&self, roots: &Roots) -> PathBuf {
        match self.base {
            Base::Home => &roots.home,
            Base::Config => &roots.config,
            Base::Data => &roots.data,
            Base::Cache => &roots.cache,
            Base::State => &roots.state,
        }
        .join(self.path)
    }
}

pub(super) const fn config(path: &'static str) -> Place {
    Place {
        base: Base::Config,
        path,
        kind: Kind::Data,
    }
}
pub(super) const fn data(path: &'static str) -> Place {
    Place {
        base: Base::Data,
        path,
        kind: Kind::Data,
    }
}
pub(super) const fn cache(path: &'static str) -> Place {
    Place {
        base: Base::Cache,
        path,
        kind: Kind::Cache,
    }
}
pub(super) const fn state(path: &'static str) -> Place {
    Place {
        base: Base::State,
        path,
        kind: Kind::State,
    }
}
pub(super) const fn home(path: &'static str) -> Place {
    Place {
        base: Base::Home,
        path,
        kind: Kind::Data,
    }
}
pub(super) const fn home_cache(path: &'static str) -> Place {
    Place {
        base: Base::Home,
        path,
        kind: Kind::Cache,
    }
}

pub(super) struct Rule {
    /// Lowercase package IDs, AppImage stems, command names, or app IDs.
    pub aliases: &'static [&'static str],
    /// Install types whose files land in the places below.
    pub sources: &'static [PackageSource],
    pub places: &'static [Place],
    /// Commands whose running processes write the places.
    pub commands: &'static [&'static str],
    pub evidence: &'static str,
    /// Data may hold accounts, history, chats, or hand-written settings, so it
    /// always needs a separate selection. Caches may still default selected.
    pub sensitive: bool,
    /// Also offer exact-name matches, for apps whose rule only adds the
    /// oddly named places a name search cannot find.
    pub name_match: bool,
}

impl Rule {
    /// Whether one of this rule's places is, contains, or sits inside `path`.
    pub fn names_path(&self, roots: &Roots, path: &Path) -> bool {
        self.places.iter().any(|place| {
            let named = place.resolve(roots);
            named.starts_with(path) || path.starts_with(&named)
        })
    }
}

/// Unsandboxed installs: APT packages, AppImages, and verified extracted apps.
pub(super) const HOST: &[PackageSource] = &[
    PackageSource::Apt,
    PackageSource::AppImage,
    PackageSource::Desktop,
];
/// As [`HOST`], plus classic Snaps, which also write normal home paths.
pub(super) const HOST_AND_CLASSIC_SNAP: &[PackageSource] = &[
    PackageSource::Apt,
    PackageSource::Snap,
    PackageSource::AppImage,
    PackageSource::Desktop,
];

pub(super) const BASE: Rule = Rule {
    aliases: &[],
    sources: HOST,
    places: &[],
    commands: &[],
    evidence: "",
    sensitive: true,
    name_match: false,
};

fn rules() -> impl Iterator<Item = &'static Rule> {
    browsers::RULES
        .iter()
        .chain(editors::RULES)
        .chain(apps::RULES)
        .copied()
        .chain(xdg_ninja::RULES.iter())
}

/// The rule for this installation, when its install type writes the places.
pub(super) fn rule(pkg: &InstalledPackage) -> Option<&'static Rule> {
    known(pkg).filter(|rule| rule.sources.contains(&pkg.source))
}

/// The rule naming this installation, whatever its install type. Ownership
/// checks use it so a sandboxed copy still claims the app's shared names.
pub(super) fn known(pkg: &InstalledPackage) -> Option<&'static Rule> {
    let ids = identities(pkg);
    rules().find(|rule| ids.iter().any(|id| rule.aliases.contains(&id.as_str())))
}

pub(super) fn identities(pkg: &InstalledPackage) -> Vec<String> {
    let mut names = vec![pkg.package_id.to_lowercase()];
    if let Some(app) = &pkg.appimage_installation {
        if let Some(stem) = app.path.file_stem().and_then(|s| s.to_str()) {
            names.push(normalize(stem));
        }
    }
    if let Some(app) = &pkg.extracted_installation {
        if let Some(name) = app.root.file_name().and_then(|s| s.to_str()) {
            names.push(name.to_lowercase());
        }
    }
    if let Some(tool) = &pkg.tool_installation {
        names.extend(
            tool.executables
                .iter()
                .filter_map(|p| p.file_name()?.to_str())
                .map(str::to_lowercase),
        );
    }
    names.into_iter().map(|s| s.to_lowercase()).collect()
}

pub(super) fn normalize(s: &str) -> String {
    s.trim_end_matches(".desktop")
        .trim_end_matches(".appimage")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Component;

    fn valid_place(path: &str) -> bool {
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
    }

    #[test]
    fn every_rule_is_exact_and_documented() {
        for rule in rules() {
            assert!(!rule.aliases.is_empty());
            assert!(!rule.evidence.is_empty(), "{:?}", rule.aliases);
            for alias in rule.aliases {
                assert_eq!(*alias, alias.to_lowercase(), "aliases are lowercase");
            }
            for place in rule.places {
                assert!(valid_place(place.path), "{}", place.path);
                if place.base == Base::Home {
                    assert!(place.path.starts_with('.'), "{}", place.path);
                }
            }
            assert!(
                !rule.sources.contains(&PackageSource::Flatpak),
                "Flatpak data stays in ~/.var/app"
            );
        }
    }

    #[test]
    fn aliases_name_one_rule() {
        let mut seen = HashSet::new();
        for rule in rules() {
            for alias in rule.aliases {
                assert!(seen.insert(*alias), "duplicate alias {alias}");
            }
        }
    }

    #[test]
    fn no_rule_offers_a_shared_root() {
        let shared = [
            ".mozilla",
            ".cursor",
            ".gemini",
            ".t3",
            ".claude",
            ".codex",
            ".config",
            ".local",
            ".cache",
            "BraveSoftware",
            "mozilla",
            ".t3/worktrees",
        ];
        for rule in rules() {
            for place in rule.places {
                assert!(!shared.contains(&place.path), "{} is shared", place.path);
            }
        }
    }

    #[test]
    fn browser_and_chat_profiles_are_never_default_data() {
        for rule in browsers::RULES.iter().chain(editors::RULES) {
            assert!(rule.sensitive, "{:?}", rule.aliases);
        }
    }
}
