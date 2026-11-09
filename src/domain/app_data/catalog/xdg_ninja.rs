//! Home-folder names from the xdg-ninja catalog (MIT, see
//! THIRD-PARTY-NOTICES.md), paired by hand with Ubuntu/Debian package names.
//! Only the installs that put files directly in HOME are listed; each rule
//! adds to the exact-name search instead of replacing it, because these apps
//! also use normal XDG folders. Hand-written settings shared by every build
//! of a program family stay out (for example ~/.vimrc).
use super::{home, home_cache, Rule, BASE};

macro_rules! ninja {
    ($file:literal, [$($alias:literal),+ $(,)?], [$($place:expr),+ $(,)?]) => {
        Rule {
            aliases: &[$($alias),+],
            places: &[$($place),+],
            evidence: concat!(
                "xdg-ninja catalog (MIT): https://github.com/b3nj5m1n/xdg-ninja/blob/ebf481aee21326cacb87bcb628b19be53b00db82/programs/",
                $file,
                ".json"
            ),
            name_match: true,
            ..BASE
        }
    };
}

pub(super) const RULES: &[Rule] = &[
    ninja!("alacritty", ["alacritty"], [home(".alacritty.yml")]),
    ninja!("amule", ["amule"], [home(".aMule")]),
    ninja!(
        "antimicrox",
        ["antimicrox"],
        [home(".antimicro"), home(".antimicrox")]
    ),
    ninja!("arandr", ["arandr"], [home(".screenlayout")]),
    ninja!("audacity", ["audacity"], [home(".audacity-data")]),
    ninja!(
        "boinc",
        ["boinc", "boinc-manager"],
        [home(".BOINC"), home(".BOINC Manager")]
    ),
    ninja!("bpython", ["bpython", "bpython3"], [home(".pythonhist")]),
    ninja!(
        "conky",
        ["conky", "conky-all", "conky-std", "conky-cli"],
        [home(".conkyrc")]
    ),
    ninja!(
        "dropbox",
        ["dropbox", "nautilus-dropbox"],
        [home(".dropbox"), home_cache(".dropbox-dist")]
    ),
    ninja!(
        "emacs",
        [
            "emacs",
            "emacs-gtk",
            "emacs-pgtk",
            "emacs-nox",
            "emacs-lucid"
        ],
        [home(".emacs.d"), home(".emacs")]
    ),
    ninja!("feh", ["feh"], [home(".fehbg")]),
    ninja!(
        "fontforge",
        ["fontforge"],
        [home(".FontForge"), home(".PfaEdit")]
    ),
    ninja!("htop", ["htop"], [home(".htoprc")]),
    ninja!("jgmenu", ["jgmenu"], [home_cache(".jgmenu-lockfile")]),
    ninja!("lmms", ["lmms"], [home(".lmmsrc.xml")]),
    ninja!(
        "milkytracker",
        ["milkytracker"],
        [home(".milkytracker_config")]
    ),
    ninja!(
        "octave",
        ["octave"],
        [
            home(".octaverc"),
            home(".octave_hist"),
            home(".octave_packages")
        ]
    ),
    ninja!("racket", ["racket"], [home(".racket"), home(".racketrc")]),
    ninja!("rofi", ["rofi"], [home_cache(".rofi.pid")]),
    ninja!(
        "scite",
        ["scite"],
        [home(".SciTE.session"), home(".SciTEUser.properties")]
    ),
    ninja!(
        "scummvm",
        ["scummvm"],
        [home(".scummvmrc"), home(".scummvm")]
    ),
    ninja!(
        "virtualbox",
        [
            "virtualbox",
            "virtualbox-qt",
            "virtualbox-7.0",
            "virtualbox-7.1",
            "virtualbox-7.2"
        ],
        [home(".VirtualBox")]
    ),
    ninja!(
        "visidata",
        ["visidata"],
        [home(".visidata"), home(".visidatarc")]
    ),
    ninja!("xpdf", ["xpdf"], [home(".xpdfrc")]),
    ninja!(
        "asunder",
        ["asunder"],
        [
            home(".asunder"),
            home_cache(".asunder_album_artist"),
            home_cache(".asunder_album_genre"),
            home_cache(".asunder_album_title"),
        ]
    ),
    ninja!("dia", ["dia"], [home(".dia")]),
    ninja!(
        "google-earth",
        ["google-earth-pro-stable"],
        [home(".googleearth")]
    ),
    ninja!(
        "jameica",
        ["jameica"],
        [home(".jameica"), home(".jameica.properties")]
    ),
    ninja!("minetest", ["minetest", "luanti"], [home(".minetest")]),
    ninja!("openshot", ["openshot-qt"], [home(".openshot_qt")]),
    ninja!("scilab", ["scilab"], [home(".Scilab")]),
    ninja!(
        "simplescreenrecorder",
        ["simplescreenrecorder"],
        [home(".ssr")]
    ),
    ninja!("texmacs", ["texmacs"], [home(".TeXmacs")]),
    ninja!(
        "x3270",
        ["x3270"],
        [home(".x3270pro"), home(".x3270connect")]
    ),
    ninja!("viber", ["viber"], [home(".ViberPC")]),
    ninja!(
        "gitkraken",
        ["gitkraken"],
        [home(".gitkraken"), home(".gk")]
    ),
    ninja!("cryptomator", ["cryptomator"], [home(".Cryptomator")]),
];
