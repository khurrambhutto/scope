//! Reviewed associations. These paths are exact app namespaces, not fuzzy matches.
use crate::domain::package::InstalledPackage;

pub(super) struct Rule {
    pub aliases: &'static [&'static str],
    pub namespaces: &'static [&'static str],
    pub commands: &'static [&'static str],
    pub evidence: &'static str,
    pub sensitive: bool,
}

// Paperling retains its original Tauri bundle identifier after the rename.
// Verified against its first-party src-tauri/tauri.conf.json, 2026-10-31.
const PAPERLING: Rule = Rule { aliases: &["paperling", "marklite", "com.saqla.marklite"], namespaces: &["com.saqla.marklite"], commands: &["paperling", "marklite"], evidence: "Paperling retains com.saqla.marklite: https://github.com/khurrambhutto/Paperling/blob/main/src-tauri/tauri.conf.json", sensitive: false };
// Upstream namespaces: https://github.com/cjpais/Handy (Tauri identifier).
const HANDY: Rule = Rule { aliases: &["handy", "com.pais.handy"], namespaces: &["com.pais.handy"], commands: &["handy"], evidence: "Handy Tauri identifier com.pais.handy: https://github.com/cjpais/Handy/blob/main/src-tauri/tauri.conf.json. May include models/transcripts.", sensitive: true };
// These applications explicitly use the named XDG namespaces. Profile roots
// retain a separate review choice because they may contain valuable content.
const CODE: Rule = Rule { aliases: &["code", "visual-studio-code", "com.visualstudio.code"], namespaces: &["Code"], commands: &["code"], evidence: "VS Code Linux user data: https://code.visualstudio.com/docs/configure/settings (config/Code only).", sensitive: true };
const RUFF: Rule = Rule { aliases: &["ruff"], namespaces: &["ruff"], commands: &["ruff"], evidence: "Ruff global cache namespace: https://docs.astral.sh/ruff/configuration/#cache-dir. Project .ruff_cache is excluded.", sensitive: false };
const UV: Rule = Rule {
    aliases: &["uv"],
    namespaces: &[],
    commands: &["uv"],
    evidence: "uv cache and Python installations are shared manager resources and are retained.",
    sensitive: true,
};

pub(super) fn rule(pkg: &InstalledPackage) -> Option<&'static Rule> {
    let ids = identities(pkg);
    [&PAPERLING, &HANDY, &CODE, &RUFF, &UV]
        .into_iter()
        .find(|rule| ids.iter().any(|id| rule.aliases.contains(&id.as_str())))
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
