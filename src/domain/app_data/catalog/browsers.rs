//! Browser profiles hold passwords, cookies, and history, so their data always
//! needs a separate selection. Each channel has its own folders; the vendor's
//! parent folder and the folders other apps write (native-messaging hosts)
//! are never offered.
use super::{cache, config, home, Rule, BASE};

const CHROMIUM: &str = "Chromium-based user data and cache folders: https://chromium.googlesource.com/chromium/src/+/HEAD/docs/user_data_dir.md";
const BRAVE: &str = "Brave keeps each channel's profile under BraveSoftware/<channel>: https://community.brave.com/t/where-are-my-profiles-stored-on-linux-if-not-in-config/520668";
const FIREFOX: &str = "Firefox profiles live in ~/.mozilla/firefox, and since Firefox 147 new profiles use ~/.config/mozilla/firefox: https://bugzilla.mozilla.org/show_bug.cgi?id=259356. ~/.mozilla itself is shared with other Mozilla apps and native-messaging hosts.";

const CHROME: Rule = Rule {
    aliases: &["google-chrome-stable", "google-chrome"],
    places: &[config("google-chrome"), cache("google-chrome")],
    commands: &["google-chrome-stable", "google-chrome"],
    evidence: CHROMIUM,
    ..BASE
};
const CHROME_BETA: Rule = Rule {
    aliases: &["google-chrome-beta"],
    places: &[config("google-chrome-beta"), cache("google-chrome-beta")],
    commands: &["google-chrome-beta"],
    evidence: CHROMIUM,
    ..BASE
};
const CHROME_DEV: Rule = Rule {
    aliases: &["google-chrome-unstable"],
    places: &[
        config("google-chrome-unstable"),
        cache("google-chrome-unstable"),
    ],
    commands: &["google-chrome-unstable"],
    evidence: CHROMIUM,
    ..BASE
};
// Ubuntu's chromium-browser package and the strict chromium Snap keep data in
// ~/snap/chromium, so only unsandboxed builds named chromium qualify.
const CHROMIUM_BROWSER: Rule = Rule {
    aliases: &["chromium"],
    places: &[config("chromium"), cache("chromium")],
    commands: &["chromium"],
    evidence: CHROMIUM,
    ..BASE
};
const BRAVE_STABLE: Rule = Rule {
    aliases: &["brave-browser"],
    places: &[
        config("BraveSoftware/Brave-Browser"),
        cache("BraveSoftware/Brave-Browser"),
    ],
    commands: &["brave-browser", "brave-browser-stable"],
    evidence: BRAVE,
    ..BASE
};
const BRAVE_BETA: Rule = Rule {
    aliases: &["brave-browser-beta"],
    places: &[
        config("BraveSoftware/Brave-Browser-Beta"),
        cache("BraveSoftware/Brave-Browser-Beta"),
    ],
    commands: &["brave-browser-beta"],
    evidence: BRAVE,
    ..BASE
};
const BRAVE_NIGHTLY: Rule = Rule {
    aliases: &["brave-browser-nightly"],
    places: &[
        config("BraveSoftware/Brave-Browser-Nightly"),
        cache("BraveSoftware/Brave-Browser-Nightly"),
    ],
    commands: &["brave-browser-nightly"],
    evidence: BRAVE,
    ..BASE
};
const EDGE: Rule = Rule {
    aliases: &["microsoft-edge-stable", "microsoft-edge"],
    places: &[config("microsoft-edge"), cache("microsoft-edge")],
    commands: &["microsoft-edge-stable", "microsoft-edge"],
    evidence: CHROMIUM,
    ..BASE
};
const EDGE_BETA: Rule = Rule {
    aliases: &["microsoft-edge-beta"],
    places: &[config("microsoft-edge-beta"), cache("microsoft-edge-beta")],
    commands: &["microsoft-edge-beta"],
    evidence: CHROMIUM,
    ..BASE
};
const EDGE_DEV: Rule = Rule {
    aliases: &["microsoft-edge-dev"],
    places: &[config("microsoft-edge-dev"), cache("microsoft-edge-dev")],
    commands: &["microsoft-edge-dev"],
    evidence:
        "Edge Dev profile folder: https://man.linuxreviews.org/man1/microsoft-edge-dev.1.html",
    ..BASE
};
const VIVALDI: Rule = Rule {
    aliases: &["vivaldi-stable", "vivaldi"],
    places: &[config("vivaldi"), cache("vivaldi")],
    commands: &["vivaldi-stable", "vivaldi"],
    evidence: CHROMIUM,
    ..BASE
};
const VIVALDI_SNAPSHOT: Rule = Rule {
    aliases: &["vivaldi-snapshot"],
    places: &[config("vivaldi-snapshot"), cache("vivaldi-snapshot")],
    commands: &["vivaldi-snapshot"],
    evidence: CHROMIUM,
    ..BASE
};
const OPERA: Rule = Rule {
    aliases: &["opera-stable", "opera"],
    places: &[config("opera"), cache("opera")],
    commands: &["opera"],
    evidence: CHROMIUM,
    ..BASE
};
const OPERA_BETA: Rule = Rule {
    aliases: &["opera-beta"],
    places: &[config("opera-beta"), cache("opera-beta")],
    commands: &["opera-beta"],
    evidence: CHROMIUM,
    ..BASE
};
const OPERA_DEVELOPER: Rule = Rule {
    aliases: &["opera-developer"],
    places: &[config("opera-developer"), cache("opera-developer")],
    commands: &["opera-developer"],
    evidence: CHROMIUM,
    ..BASE
};
// Observed with helium-bin 0.21.1.1 from pkg.helium.computer on Ubuntu.
const HELIUM: Rule = Rule {
    aliases: &["helium-bin", "helium", "helium-browser", "net.imput.helium"],
    places: &[config("net.imput.helium"), cache("net.imput.helium")],
    commands: &["helium"],
    evidence: "Helium (imput) uses its app ID net.imput.helium for its profile and cache folders: https://github.com/imputnet/helium-linux",
    ..BASE
};
// Firefox channels share ~/.mozilla/firefox through one profiles.ini, so they
// are one rule and keep each other's profiles while any channel remains.
// Ubuntu's firefox package installs the strict Snap, which another
// installation of the same name then claims.
const FIREFOX_ALL: Rule = Rule {
    aliases: &[
        "firefox",
        "firefox-esr",
        "firefox-beta",
        "firefox-nightly",
        "firefox-devedition",
    ],
    places: &[
        home(".mozilla/firefox"),
        config("mozilla/firefox"),
        cache("mozilla/firefox"),
    ],
    commands: &["firefox", "firefox-esr"],
    evidence: FIREFOX,
    ..BASE
};
const THUNDERBIRD: Rule = Rule {
    aliases: &["thunderbird"],
    places: &[home(".thunderbird"), cache("thunderbird")],
    commands: &["thunderbird"],
    evidence: "Thunderbird keeps mail profiles in ~/.thunderbird: https://support.mozilla.org/kb/profiles-where-thunderbird-stores-user-data",
    ..BASE
};
const LIBREWOLF: Rule = Rule {
    aliases: &["librewolf"],
    places: &[home(".librewolf")],
    commands: &["librewolf"],
    evidence: "LibreWolf profile folder: https://librewolf.net/docs/settings/",
    ..BASE
};
const ZEN: Rule = Rule {
    aliases: &["zen", "zen-browser", "zen_browser"],
    places: &[home(".zen"), config("zen")],
    commands: &["zen", "zen-browser"],
    evidence: "Zen keeps profiles in ~/.zen and, since 1.18.6b, ~/.config/zen: https://docs.zen-browser.app/guides/install-linux",
    ..BASE
};

pub(super) const RULES: &[&Rule] = &[
    &CHROME,
    &CHROME_BETA,
    &CHROME_DEV,
    &CHROMIUM_BROWSER,
    &BRAVE_STABLE,
    &BRAVE_BETA,
    &BRAVE_NIGHTLY,
    &EDGE,
    &EDGE_BETA,
    &EDGE_DEV,
    &VIVALDI,
    &VIVALDI_SNAPSHOT,
    &OPERA,
    &OPERA_BETA,
    &OPERA_DEVELOPER,
    &HELIUM,
    &FIREFOX_ALL,
    &THUNDERBIRD,
    &LIBREWOLF,
    &ZEN,
];
