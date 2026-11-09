//! Other apps and tools with reviewed folders.
use super::{cache, config, data, home, state, Rule, BASE};
use crate::domain::package::PackageSource;

/// Tool installs Scope manages, plus APT and unsandboxed app installs.
const TOOL_SOURCES: &[PackageSource] = &[
    PackageSource::Apt,
    PackageSource::Uv,
    PackageSource::Pipx,
    PackageSource::Cargo,
    PackageSource::AppImage,
    PackageSource::Desktop,
];

/// Every host-visible install type. These apps write the same folders from a
/// Snap or tool install as from a package.
const ANY_HOST: &[PackageSource] = &[
    PackageSource::Apt,
    PackageSource::Uv,
    PackageSource::Pipx,
    PackageSource::Cargo,
    PackageSource::Npm,
    PackageSource::Bun,
    PackageSource::Snap,
    PackageSource::AppImage,
    PackageSource::Desktop,
];

// Paperling retains its original Tauri bundle identifier after the rename.
// Verified against its first-party src-tauri/tauri.conf.json, 2026-10-31.
const PAPERLING: Rule = Rule {
    aliases: &["paperling", "marklite", "com.saqla.marklite"],
    sources: ANY_HOST,
    places: &[
        config("com.saqla.marklite"),
        data("com.saqla.marklite"),
        cache("com.saqla.marklite"),
        state("com.saqla.marklite"),
    ],
    commands: &["paperling", "marklite"],
    evidence: "Paperling retains com.saqla.marklite: https://github.com/khurrambhutto/Paperling/blob/main/src-tauri/tauri.conf.json",
    sensitive: false,
    ..BASE
};
const HANDY: Rule = Rule {
    aliases: &["handy", "com.pais.handy"],
    sources: ANY_HOST,
    places: &[
        config("com.pais.handy"),
        data("com.pais.handy"),
        cache("com.pais.handy"),
        state("com.pais.handy"),
    ],
    commands: &["handy"],
    evidence: "Handy Tauri identifier com.pais.handy: https://github.com/cjpais/Handy/blob/main/src-tauri/tauri.conf.json. May include models/transcripts.",
    ..BASE
};
const RUFF: Rule = Rule {
    aliases: &["ruff"],
    sources: TOOL_SOURCES,
    places: &[cache("ruff")],
    commands: &["ruff"],
    evidence: "Ruff global cache namespace: https://docs.astral.sh/ruff/configuration/#cache-dir. Project .ruff_cache is excluded.",
    sensitive: false,
    ..BASE
};
// uv's cache and managed Pythons are shared manager resources, so the rule
// names no places and keeps a name search from offering them.
const UV: Rule = Rule {
    aliases: &["uv"],
    sources: TOOL_SOURCES,
    commands: &["uv"],
    evidence: "uv cache and Python installations are shared manager resources and are retained.",
    ..BASE
};
// Vaults are the user's own folders and are never touched.
const OBSIDIAN: Rule = Rule {
    aliases: &["obsidian", "md.obsidian.obsidian"],
    places: &[config("obsidian")],
    commands: &["obsidian"],
    evidence: "Obsidian app data folder (vaults are stored elsewhere): https://obsidian.md/help/data-storage",
    ..BASE
};
const ZOOM: Rule = Rule {
    aliases: &["zoom"],
    places: &[
        home(".zoom"),
        config("zoomus.conf"),
        config("zoom.conf"),
        config("zoom"),
        cache("zoom"),
    ],
    commands: &["zoom"],
    evidence: "Observed with the zoom 7.1 .deb on Ubuntu: meetings data and logs in ~/.zoom, settings in ~/.config/zoomus.conf and zoom.conf.",
    ..BASE
};

pub(super) const RULES: &[&Rule] = &[&PAPERLING, &HANDY, &RUFF, &UV, &OBSIDIAN, &ZOOM];
