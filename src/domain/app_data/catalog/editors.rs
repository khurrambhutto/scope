//! Code editors and AI coding apps. Their data holds settings, extensions,
//! accounts, chats, and workspaces, so it always needs a separate selection.
//! Folders a vendor shares with its CLI (~/.cursor, ~/.gemini, ~/.claude,
//! ~/.codex, the OpenCode CLI folders) and project checkouts are never offered.
use super::{cache, config, data, home, home_cache, Rule, BASE, HOST_AND_CLASSIC_SNAP};

const CODE: Rule = Rule {
    aliases: &["code", "visual-studio-code", "com.visualstudio.code"],
    sources: HOST_AND_CLASSIC_SNAP,
    places: &[config("Code"), home(".vscode")],
    commands: &["code"],
    evidence: "VS Code settings live in ~/.config/Code and extensions in ~/.vscode: https://code.visualstudio.com/docs/configure/settings, https://code.visualstudio.com/docs/configure/extensions/extension-marketplace",
    ..BASE
};
const CODE_INSIDERS: Rule = Rule {
    aliases: &["code-insiders"],
    sources: HOST_AND_CLASSIC_SNAP,
    places: &[config("Code - Insiders"), home(".vscode-insiders")],
    commands: &["code-insiders"],
    evidence: "VS Code Insiders keeps its own Code - Insiders and ~/.vscode-insiders folders: https://code.visualstudio.com/docs/configure/settings",
    ..BASE
};
const VSCODIUM: Rule = Rule {
    aliases: &["codium", "vscodium", "com.vscodium.codium"],
    sources: HOST_AND_CLASSIC_SNAP,
    places: &[config("VSCodium"), home(".vscode-oss")],
    commands: &["codium"],
    evidence:
        "VSCodium data folders: https://github.com/VSCodium/vscodium/blob/master/docs/index.md",
    ..BASE
};
// ~/.cursor is shared with the separate cursor-agent CLI (cli-config.json,
// mcp.json, agents), as are ~/.config/cursor and ~/.local/state/cursor.
const CURSOR: Rule = Rule {
    aliases: &["cursor"],
    places: &[
        config("Cursor"),
        home(".cursor/extensions"),
        home(".cursor/argv.json"),
    ],
    commands: &["cursor"],
    evidence: "Observed with the cursor 3.24 .deb: the IDE uses ~/.config/Cursor and ~/.cursor/extensions; the cursor-agent CLI's files stay: https://cursor.com/docs/cli/reference/configuration",
    ..BASE
};
const ZED: Rule = Rule {
    aliases: &["zed", "zed-editor", "dev.zed.zed"],
    places: &[config("zed"), data("zed"), cache("zed")],
    commands: &["zed"],
    evidence: "Zed settings, data, and caches: https://zed.dev/docs/uninstall",
    ..BASE
};
const SUBLIME_TEXT: Rule = Rule {
    aliases: &["sublime-text"],
    sources: HOST_AND_CLASSIC_SNAP,
    places: &[config("sublime-text"), cache("sublime-text")],
    commands: &["subl"],
    evidence: "Sublime Text data and cache folders: https://www.sublimetext.com/docs/revert.html",
    ..BASE
};
const SUBLIME_MERGE: Rule = Rule {
    aliases: &["sublime-merge"],
    sources: HOST_AND_CLASSIC_SNAP,
    places: &[config("sublime-merge")],
    commands: &["smerge"],
    evidence: "Sublime Merge data folder: https://www.sublimemerge.com/docs/packages",
    ..BASE
};
// ~/.kiro is shared with the Kiro CLI.
const KIRO: Rule = Rule {
    aliases: &["kiro"],
    places: &[config("Kiro")],
    commands: &["kiro"],
    evidence: "Kiro IDE data folder: https://kiro.dev/docs/ide/troubleshooting/",
    ..BASE
};
// ~/.gemini is shared with the Gemini CLI; only Antigravity's own subfolder
// is offered.
const ANTIGRAVITY: Rule = Rule {
    aliases: &["antigravity", "google-antigravity"],
    places: &[
        config("Antigravity"),
        cache("antigravity"),
        home(".gemini/antigravity"),
    ],
    commands: &["antigravity"],
    evidence: "Observed with Google Antigravity on Ubuntu: Electron data in ~/.config/Antigravity and agent data in ~/.gemini/antigravity, beside the Gemini CLI's own files.",
    ..BASE
};
// ~/.t3/worktrees holds project checkouts and is never offered.
const T3_CODE: Rule = Rule {
    aliases: &[
        "t3code",
        "t3-code",
        "t3_code",
        "t3_code_alpha",
        "t3_code_beta",
        "t3_code_nightly",
        "com.t3tools.t3code",
        "t3",
    ],
    places: &[
        config("t3code-v2"),
        config("t3code"),
        config("T3 Code (Alpha)"),
        cache("t3code-updater"),
        home(".t3/userdata"),
        home(".t3/scratch"),
        home(".t3/bin"),
        home(".t3/tools"),
        home_cache(".t3/caches"),
    ],
    evidence: "T3 Code desktop data (t3code-v2, migrated from t3code and T3 Code (Alpha)) and its ~/.t3 state: https://github.com/pingdotgg/t3code/blob/main/apps/desktop/src/app/DesktopUserData.ts, https://github.com/pingdotgg/t3code/blob/main/apps/desktop/src/app/DesktopStatePaths.ts",
    ..BASE
};
// The opencode CLI's own folders (~/.config/opencode, ~/.local/share/opencode,
// ~/.local/state/opencode, ~/.cache/opencode) stay with the CLI.
const OPENCODE_DESKTOP: Rule = Rule {
    aliases: &["opencode", "opencode-desktop", "ai.opencode.desktop"],
    places: &[
        config("ai.opencode.desktop"),
        cache("@opencode-aidesktop-updater"),
    ],
    evidence: "OpenCode desktop stores its data under its app ID: https://github.com/anomalyco/opencode/blob/HEAD/packages/desktop/src/main/lifecycle/configure.ts",
    ..BASE
};
// Claude Code's ~/.claude and ~/.claude.json belong to the CLI and stay.
const CLAUDE_DESKTOP: Rule = Rule {
    aliases: &["claude-desktop", "com.anthropic.claude"],
    places: &[config("Claude"), config("Claude-3p")],
    commands: &["claude-desktop"],
    evidence: "Claude desktop data folders: https://claude.com/docs/third-party/claude-desktop/data-storage",
    ..BASE
};
const ZCODE: Rule = Rule {
    aliases: &["zcode"],
    places: &[config("ZCode"), home(".zcode")],
    evidence: "Observed with the ZCode AppImage on Ubuntu: Electron data in ~/.config/ZCode and workspaces, backups, and CLI settings in ~/.zcode.",
    ..BASE
};

pub(super) const RULES: &[&Rule] = &[
    &CODE,
    &CODE_INSIDERS,
    &VSCODIUM,
    &CURSOR,
    &ZED,
    &SUBLIME_TEXT,
    &SUBLIME_MERGE,
    &KIRO,
    &ANTIGRAVITY,
    &T3_CODE,
    &OPENCODE_DESKTOP,
    &CLAUDE_DESKTOP,
    &ZCODE,
];
