(function () {
  "use strict";

  var reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- Shared source facts (mirror AGENTS.md command table) ---------- */

  var SOURCES = {
    APT: {
      color: "var(--apt)", tag: "Manual installs",
      what: "Packages you installed yourself, from apt-mark showmanual. Libraries, dependencies and metapackages stay out of the main list.",
      uninstall: "pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y {pkg}",
      update: "pkexec env DEBIAN_FRONTEND=noninteractive apt install -y {pkg}",
      sample: "cap", auth: true,
      fine: "Each transaction is simulated with LC_ALL=C before the preview and again right before it runs."
    },
    Snap: {
      color: "var(--snap)", tag: "Apps and tools",
      what: "Apps and command-line tools from the Snap Store. Runtimes and bases such as core, snapd and gnome-* stay hidden.",
      uninstall: "pkexec snap remove {pkg}",
      update: "pkexec snap refresh {pkg}",
      sample: "firefox", auth: true,
      fine: "Details show the current and older revisions, user data and system data, each measured separately. snap remove may keep a recovery snapshot."
    },
    Flatpak: {
      color: "var(--flatpak)", tag: "User and system",
      what: "User and system installs are separate rows. The scope recorded at scan time is the one every later command uses.",
      uninstall: "flatpak uninstall -y --user {pkg}\npkexec flatpak uninstall -y --system {pkg}",
      update: "flatpak update -y --user {pkg}\npkexec flatpak update -y --system {pkg}",
      sample: "com.github.tchx84.Flatseal", auth: false,
      fine: "Shared runtimes are never counted. Data in ~/.var/app is shown and kept."
    },
    uv: {
      color: "var(--uv)", tag: "Next release", soon: true,
      what: "Persistent uv tool installs, one row per tool rather than per command. uvx caches and project environments are left alone.",
      uninstall: "uv tool uninstall {pkg}",
      update: "uv tool upgrade {pkg}",
      sample: "ruff", auth: false,
      fine: "Update checks resolve with your stored requirements and constraints. Commands run in the recorded tool and bin directories, never as root."
    },
    pipx: {
      color: "var(--pipx)", tag: "Next release", soon: true,
      what: "pipx environments in your home directory, with their pip options and injected packages.",
      uninstall: "pipx uninstall {pkg}",
      update: "pipx upgrade {pkg}",
      sample: "httpie", auth: false,
      fine: "The preview lists injected packages that leave with the environment. Pinned versions block updates."
    },
    Cargo: {
      color: "var(--cargo)", tag: "Next release", soon: true,
      what: "Binary crates tracked by cargo install, cross-checked against Cargo's own receipts.",
      uninstall: "cargo uninstall --root ~/.cargo {pkg}",
      update: "cargo install --root ~/.cargo {pkg}",
      sample: "ripgrep", auth: false,
      fine: "Rebuilds keep the original source, features, profile and target, and never force an overwrite."
    },
    AppImage: {
      color: "var(--appimage)", tag: "Listed only",
      what: "AppImages in ~/Applications, ~/Downloads, /opt and other common folders. Scope finds them so you know they're there.",
      uninstall: null, update: null,
      fine: "Uninstall and update aren't supported yet, so the actions stay disabled."
    },
    Desktop: {
      color: "var(--desktop)", tag: "Listed only",
      what: "Launchers in your applications folder that no package manager owns, such as apps installed by their own scripts.",
      uninstall: null, update: null,
      fine: "Listed for discovery. Scope won't touch files it can't attribute to a package manager."
    }
  };

  function cmd(source, kind, pkg) {
    var s = SOURCES[source];
    var tpl = s && s[kind];
    if (!tpl) return null;
    return tpl.split("\n")[0].replace("{pkg}", pkg);
  }

  /* ---------- Demo data ---------- */

  var KB = 1024, MB = KB * 1024, GB = MB * 1024;

  function fmt(bytes) {
    if (bytes == null) return "—";
    if (bytes >= GB) return (bytes / GB).toFixed(1).replace(/\.0$/, "") + " GB";
    if (bytes >= MB) return (bytes >= 10 * MB ? Math.round(bytes / MB) : (bytes / MB).toFixed(1).replace(/\.0$/, "")) + " MB";
    return Math.max(1, Math.round(bytes / KB)) + " KB";
  }

  // [label, path, bytes, kept]
  var APPS = [
    { name: "Antigravity", pkg: "antigravity", src: "Desktop", ver: null, size: null, icon: "antigravity",
      parts: [["Launcher", "~/.local/share/applications/antigravity.desktop", 4 * KB], ["Executable", "~/.local/opt/antigravity/antigravity", 186 * MB]] },
    { name: "App Center", pkg: "snap-store", src: "Snap", ver: "0+git.d7d3fbe8", size: 12 * MB, icon: "app-center",
      parts: [["Application", "/var/lib/snapd/snaps/snap-store_1447.snap", 12 * MB], ["Older revision", "/var/lib/snapd/snaps/snap-store_1427.snap", 12 * MB], ["User data", "~/snap/snap-store", 51 * MB], ["System data", "/var/snap/snap-store", 1.4 * MB]] },
    { name: "Bitwarden", pkg: "bitwarden", src: "Snap", ver: "2026.9.1", size: 160 * MB, icon: "bitwarden",
      parts: [["Application", "/var/lib/snapd/snaps/bitwarden_142.snap", 160 * MB], ["Older revision", "/var/lib/snapd/snaps/bitwarden_139.snap", 158 * MB], ["User data", "~/snap/bitwarden", 38 * MB], ["System data", "/var/snap/bitwarden", 12 * KB]] },
    { name: "Brave Web Browser", pkg: "brave", src: "Snap", ver: "1.97.56", next: "1.98.4", size: 218 * MB, icon: "brave",
      parts: [["Application", "/var/lib/snapd/snaps/brave_588.snap", 218 * MB], ["Older revision", "/var/lib/snapd/snaps/brave_581.snap", 216 * MB], ["User data", "~/snap/brave", 412 * MB], ["System data", "/var/snap/brave", 8 * KB]] },
    { name: "Cap", pkg: "cap", src: "APT", ver: "0.6.0", size: 442 * MB, icon: "cap",
      parts: [["Program files", "/usr/lib/cap", 438 * MB], ["Icons", "/usr/share/icons/hicolor", 3.6 * MB], ["Launcher", "/usr/share/applications/Cap.desktop", 4 * KB]] },
    { name: "ChatGPT", pkg: "chatgpt", src: "APT", ver: "26.1002.52244", size: 1.6 * GB, icon: "chatgpt",
      parts: [["Program files", "/opt/ChatGPT", 1.59 * GB], ["Icons", "/usr/share/icons/hicolor", 2.1 * MB], ["Launcher", "/usr/share/applications/chatgpt.desktop", 4 * KB]] },
    { name: "Cursor", pkg: "cursor", src: "APT", ver: "3.23.23-1791166813", next: "3.24.2-1791904417", size: 942 * MB, icon: "cursor",
      parts: [["Program files", "/usr/share/cursor", 939 * MB], ["Configuration", "/etc/apt/sources.list.d/cursor.list", 4 * KB, true], ["Launcher", "/usr/share/applications/cursor.desktop", 4 * KB]] },
    { name: "diary", pkg: "diary", src: "APT", ver: "0.1.5", size: 19 * MB, icon: "diary",
      parts: [["Program files", "/usr/bin/diary", 19 * MB], ["Launcher", "/usr/share/applications/diary.desktop", 4 * KB]] },
    { name: "Firefox", pkg: "firefox", src: "Snap", ver: "157.0.1-1", next: "157.0.2-1", size: 262 * MB, icon: "firefox",
      parts: [["Application", "/var/lib/snapd/snaps/firefox_7120.snap", 262 * MB], ["Older revision", "/var/lib/snapd/snaps/firefox_7084.snap", 259 * MB], ["User data", "~/snap/firefox", 1.2 * GB], ["System data", "/var/snap/firefox", 16 * KB]] },
    { name: "Firmware Updater", pkg: "firmware-updater", src: "Snap", ver: "0+git.64c41a0", size: 13 * MB, icon: "firmware",
      parts: [["Application", "/var/lib/snapd/snaps/firmware-updater_210.snap", 13 * MB], ["User data", "~/snap/firmware-updater", 1.1 * MB], ["System data", "/var/snap/firmware-updater", 4 * KB]] },
    { name: "Flatseal", pkg: "com.github.tchx84.Flatseal", src: "Flatpak", scope: "user", ver: "2.3.1", size: 4.1 * MB, initials: "Fs",
      parts: [["Deployment", "~/.local/share/flatpak/app/com.github.tchx84.Flatseal", 4.1 * MB], ["App data", "~/.var/app/com.github.tchx84.Flatseal", 220 * KB, true]] },
    { name: "httpie", pkg: "httpie", src: "pipx", ver: "3.2.4", size: 21 * MB, initials: "ht", cli: true,
      parts: [["Environment", "~/.local/share/pipx/venvs/httpie", 21 * MB], ["Commands", "~/.local/bin/http, https, httpie", 12 * KB]] },
    { name: "Obsidian", pkg: "Obsidian-1.9.14.AppImage", src: "AppImage", ver: "1.9.14", size: 128 * MB, initials: "Ob",
      parts: [["AppImage", "~/Applications/Obsidian-1.9.14.AppImage", 128 * MB]] },
    { name: "ripgrep", pkg: "ripgrep", src: "Cargo", ver: "14.1.1", size: 5.2 * MB, initials: "rg", cli: true,
      parts: [["Binary", "~/.cargo/bin/rg", 5.2 * MB]] },
    { name: "ruff", pkg: "ruff", src: "uv", ver: "0.13.3", next: "0.14.0", size: 33 * MB, initials: "ru", cli: true,
      parts: [["Environment", "~/.local/share/uv/tools/ruff", 33 * MB], ["Command", "~/.local/bin/ruff", 4 * KB]] },
    // Revealed by Show all
    { name: "build-essential", pkg: "build-essential", src: "APT", ver: "12.10ubuntu1", size: 21 * KB, initials: "be", cli: true, hidden: true,
      parts: [["Documentation", "/usr/share/doc/build-essential", 21 * KB]] },
    { name: "curl", pkg: "curl", src: "APT", ver: "8.12.1-3ubuntu1", size: 520 * KB, initials: "cu", cli: true, hidden: true,
      parts: [["Program files", "/usr/bin/curl", 330 * KB], ["Documentation", "/usr/share/doc/curl", 190 * KB]] },
    { name: "linux-generic", pkg: "linux-generic", src: "APT", ver: "6.17.0-34.34", size: 12 * KB, initials: "lg", cli: true, hidden: true, protected: true,
      parts: [] },
    { name: "ubuntu-desktop", pkg: "ubuntu-desktop", src: "APT", ver: "1.539", size: 8 * KB, initials: "ud", cli: true, hidden: true, protected: true,
      parts: [] }
  ];

  APPS.forEach(function (a, i) { a.key = a.src + ":" + a.pkg; a.order = i; });

  /* ---------- Demo state and rendering ---------- */

  var root = document.getElementById("demo");
  if (root) initDemo(root);

  function initDemo(root) {
    var $ = function (sel) { return root.querySelector(sel); };
    var list = $("[data-list]");
    var search = $("[data-search]");
    var count = $("[data-count]");
    var sourceSel = $("[data-source]");
    var showAllBtn = $("[data-showall]");
    var rescanBtn = $("[data-rescan]");
    var footer = $("[data-footer]");
    var scrim = $("[data-scrim]");
    var modal = $("[data-modal]");
    var badge = $("[data-update-count]");
    var tabs = root.querySelectorAll(".sw-tab");

    var state = {
      tab: "apps", query: "", source: "", showAll: false,
      open: null, measured: {}, busy: false, removed: {}, updated: {}
    };
    var timers = [];
    var lastFocus = null;

    function later(fn, ms) { var t = setTimeout(fn, reduceMotion ? 0 : ms); timers.push(t); return t; }

    function locked(a) { return a.protected || a.src === "AppImage" || a.src === "Desktop"; }

    function visible() {
      var q = state.query.trim().toLowerCase();
      return APPS.filter(function (a) {
        if (state.removed[a.key]) return false;
        if (a.hidden && !state.showAll) return false;
        if (state.source && a.src !== state.source) return false;
        if (state.tab === "updates" && !(a.next && !state.updated[a.key])) return false;
        if (q && a.name.toLowerCase().indexOf(q) < 0 && a.pkg.toLowerCase().indexOf(q) < 0) return false;
        return true;
      }).sort(function (x, y) { return x.name.toLowerCase() < y.name.toLowerCase() ? -1 : 1; });
    }

    function pendingUpdates() {
      return APPS.filter(function (a) { return a.next && !state.updated[a.key] && !state.removed[a.key]; }).length;
    }

    function esc(s) {
      return String(s).replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; });
    }

    function iconHTML(a) {
      if (a.icon) return '<span class="sw-icon"><img src="demo/' + a.icon + '.png" width="36" height="36" alt=""></span>';
      var bg = SOURCES[a.src] ? SOURCES[a.src].color : "var(--line)";
      return '<span class="sw-icon" style="background:' + bg + '">' + esc(a.initials) + "</span>";
    }

    function metaHTML(a) {
      var src = a.src === "Flatpak" ? "Flatpak (" + a.scope + ")" : a.src;
      if (state.tab === "updates" && a.next) {
        return esc(a.ver) + ' <span class="sw-arrow" aria-label="to">→</span> ' + esc(a.next) + " · " + src;
      }
      return (a.ver ? esc(state.updated[a.key] || a.ver) : "—") +
        '<span class="sw-size"> · ' + (a.size == null ? "—" : fmt(a.size)) + "</span> · " + src;
    }

    function removable(a) {
      var m = state.measured[a.key];
      if (!m) return null;
      var total = 0;
      a.parts.forEach(function (p, i) { if (!p[3] && m[i]) total += p[2]; });
      return total;
    }

    function breakdownHTML(a, forModal) {
      var m = state.measured[a.key] || {};
      if (a.protected) return '<p class="sw-note">This package is protected and cannot be changed.</p>';
      var rows = a.parts.map(function (p, i) {
        var s = m[i] ? '<span class="s' + (p[3] ? " is-kept" : "") + '">' + (p[3] ? "Kept" : fmt(p[2])) + "</span>"
                     : '<span class="s is-pending" aria-label="Measuring"></span>';
        return '<div class="sw-brow"><span class="k">' + esc(p[0]) + '</span><span class="p">' + esc(p[1]) + "</span>" + s + "</div>";
      }).join("");
      if (!forModal && locked(a)) rows += '<p class="sw-note">Listed for discovery. Scope can’t uninstall or update ' + (a.src === "AppImage" ? "AppImages" : "launchers it can’t attribute to a package manager") + " yet.</p>";
      return rows;
    }

    function actionsHTML(a, open) {
      if (state.tab === "updates") {
        return '<span class="sw-actions"><button class="sw-pill is-quiet sw-update-pill" data-act="update" data-key="' + esc(a.key) + '"' + (state.busy ? " disabled" : "") + ">Update</button></span>";
      }
      if (!open) return "";
      if (locked(a)) return '<span class="sw-actions"><button class="sw-pill" disabled>' + (a.protected ? "Protected" : "Not supported") + "</button></span>";
      var r = removable(a);
      var done = state.measured[a.key] && Object.keys(state.measured[a.key]).length === a.parts.length;
      var label = r == null ? "" : "Removes ~" + fmt(r) + (done ? "" : "+");
      return '<span class="sw-actions"><span class="sw-removes">' + label + '</span><button class="sw-pill" data-act="uninstall" data-key="' + esc(a.key) + '"' + (state.busy ? " disabled" : "") + ">Uninstall</button></span>";
    }

    function itemHTML(a) {
      var open = state.open === a.key && state.tab === "apps";
      return '<div class="sw-item' + (open ? " is-open" : "") + '" data-item="' + esc(a.key) + '">' +
        '<div class="sw-row" role="button" tabindex="0" aria-expanded="' + open + '" data-toggle="' + esc(a.key) + '">' +
          iconHTML(a) +
          '<span class="sw-main"><span class="sw-name">' + esc(a.name) + '</span><span class="sw-meta" style="display:block">' + metaHTML(a) + "</span></span>" +
          actionsHTML(a, open) +
        "</div>" +
        '<div class="sw-detail"><div class="sw-detail-inner"><div class="sw-breakdown">' + (open ? breakdownHTML(a) : "") + "</div></div></div>" +
      "</div>";
    }

    function render(opts) {
      var items = visible();
      count.textContent = items.length + (items.length === 1 ? " result" : " results");
      var n = pendingUpdates();
      badge.textContent = n;
      badge.hidden = n === 0;
      if (!items.length) {
        list.innerHTML = '<p class="sw-empty">' + (state.tab === "updates" && !state.query ? "Everything is up to date." : "No installed apps match your search.") + "</p>";
        return;
      }
      list.innerHTML = items.map(itemHTML).join("");
      if (opts && opts.scan) {
        list.classList.add("is-scanning");
        Array.prototype.forEach.call(list.children, function (el, i) { el.style.animationDelay = (i * 45) + "ms"; });
        later(function () { list.classList.remove("is-scanning"); }, 1200);
      }
    }

    // Update only the open row in place so its height can ease.
    function refreshItem(key) {
      var el = list.querySelector('[data-item="' + CSS.escape(key) + '"]');
      var a = APPS.find(function (x) { return x.key === key; });
      if (!el || !a) return;
      var open = state.open === key;
      var actions = el.querySelector(".sw-actions");
      var html = actionsHTML(a, open);
      if (actions) actions.outerHTML = html || "";
      else if (html) el.querySelector(".sw-row").insertAdjacentHTML("beforeend", html);
      if (open) el.querySelector(".sw-breakdown").innerHTML = breakdownHTML(a);
    }

    function measure(a) {
      if (state.measured[a.key]) return;
      state.measured[a.key] = {};
      a.parts.forEach(function (_, i) {
        later(function () {
          state.measured[a.key][i] = true;
          if (state.open === a.key) refreshItem(a.key);
        }, 260 + i * 210 + Math.random() * 160);
      });
    }

    function toggle(key) {
      if (state.tab !== "apps") return;
      var prev = state.open;
      state.open = prev === key ? null : key;
      if (prev) {
        var p = list.querySelector('[data-item="' + CSS.escape(prev) + '"]');
        if (p) {
          p.classList.remove("is-open");
          p.querySelector(".sw-row").setAttribute("aria-expanded", "false");
          var act = p.querySelector(".sw-actions"); if (act) act.remove();
        }
      }
      if (state.open) {
        var a = APPS.find(function (x) { return x.key === key; });
        measure(a);
        var el = list.querySelector('[data-item="' + CSS.escape(key) + '"]');
        el.querySelector(".sw-breakdown").innerHTML = breakdownHTML(a);
        refreshItem(key);
        // Force layout so the grid-row transition runs.
        void el.offsetHeight;
        el.classList.add("is-open");
        el.querySelector(".sw-row").setAttribute("aria-expanded", "true");
        later(function () {
          var lr = list.getBoundingClientRect(), er = el.getBoundingClientRect();
          if (er.bottom > lr.bottom) list.scrollBy({ top: er.bottom - lr.bottom + 12, behavior: reduceMotion ? "auto" : "smooth" });
        }, 380);
      }
    }

    /* Preview modal */

    function openModal(kind, key) {
      var a = APPS.find(function (x) { return x.key === key; });
      if (!a || state.busy) return;
      lastFocus = document.activeElement;
      var title = (kind === "uninstall" ? "Uninstall " : "Update ") + a.name;
      var command = cmd(a.src, kind, a.pkg);
      var rows = [["Package", a.pkg]];
      if (a.scope) rows.push(["Scope", a.scope]);
      if (kind === "update") { rows.push(["Current version", a.ver]); rows.push(["Target version", a.next]); }
      else { rows.push(["Version", a.ver]); rows.push(["Size", fmt(a.size)]); }
      rows.push(["Command", command]);

      var html = '<div class="sw-modal-head"><h3 id="sw-modal-title">' + esc(title) + '</h3>' +
        '<button class="sw-iconbtn" style="border:0" aria-label="Close dialog" data-close><svg aria-hidden="true"><use href="#i-close"/></svg></button></div>' +
        '<div class="sw-modal-body"><div class="sw-meta-rows">' +
        rows.map(function (r) {
          return '<div class="sw-brow"><span class="k">' + esc(r[0]) + '</span><span class="p' + (r[0] === "Command" ? " is-cmd" : "") + '">' + esc(r[1]) + "</span></div>";
        }).join("") + "</div>";
      if (a.src === "APT") {
        html += '<p class="sw-sim">' + (kind === "uninstall" ? "APT simulation removes 1 package(s), including dependencies." : "APT simulation changes 1 package(s) and removes 0.") + "</p>";
      }
      if (kind === "uninstall" && a.parts.length) {
        state.measured[a.key] = {};
        html += '<div data-modal-breakdown>' + breakdownHTML(a, true) + "</div>";
      }
      html += '</div><div class="sw-modal-foot"><button class="sw-pill is-quiet" data-close>Cancel</button><button class="sw-pill" data-confirm>Confirm</button></div>';
      modal.innerHTML = html;
      modal.dataset.kind = kind;
      modal.dataset.key = key;
      scrim.classList.add("is-open");

      // Uninstall previews refresh inspection, like the app does.
      if (kind === "uninstall") {
        a.parts.forEach(function (_, i) {
          later(function () {
            state.measured[a.key][i] = true;
            var mb = modal.querySelector("[data-modal-breakdown]");
            if (mb) mb.innerHTML = breakdownHTML(a, true);
          }, 200 + i * 160);
        });
      }
      modal.querySelector("[data-confirm]").focus();
    }

    function closeModal() {
      scrim.classList.remove("is-open");
      if (lastFocus && document.contains(lastFocus)) lastFocus.focus();
    }

    /* Apply: verifying, authentication, running, result */

    function setFooter(html) { footer.innerHTML = html; footer.classList.add("is-open"); }

    function apply(kind, key) {
      var a = APPS.find(function (x) { return x.key === key; });
      closeModal();
      state.busy = true;
      render();
      var needsAuth = SOURCES[a.src] && SOURCES[a.src].auth;
      var hint = needsAuth ? '<span class="t">A password prompt may appear.</span>' : "";
      var running = (kind === "uninstall" ? "Uninstalling " : "Updating ") + a.name + "…";
      var log = kind === "uninstall"
        ? (a.src === "Snap" ? ["Save data of snap “" + a.pkg + "” in automatic snapshot set", "Remove data for snap “" + a.pkg + "”", a.pkg + " removed"]
          : a.src === "APT" ? ["Reading package lists… Done", "Removing " + a.pkg + " (" + a.ver + ") …", "Processing triggers for desktop-file-utils …"]
          : ["Uninstalling " + a.pkg + " …", "Done."])
        : (a.src === "Snap" ? ["Download snap “" + a.pkg + "” (" + a.next + ")", "Mount snap “" + a.pkg + "”", a.pkg + " " + a.next + " refreshed"]
          : a.src === "APT" ? ["Get:1 " + a.pkg + " amd64 " + a.next, "Unpacking " + a.pkg + " (" + a.next + ") over (" + a.ver + ") …", "Setting up " + a.pkg + " (" + a.next + ") …"]
          : ["Resolving " + a.pkg + " …", "Upgraded " + a.pkg + " from " + a.ver + " to " + a.next]);

      setFooter('<span class="sw-spin"></span><span><span class="h" style="display:block">Checking ' + esc(a.name) + "…</span>" + hint + "</span>");
      later(function () {
        setFooter('<span class="sw-spin"></span><span><span class="h" style="display:block">' + esc(running) + "</span>" + hint + '</span><span class="sw-log" data-log></span>');
        log.forEach(function (line, i) {
          later(function () { var l = footer.querySelector("[data-log]"); if (l) l.textContent = line; }, 300 + i * 650);
        });
        later(function () { finish(kind, a); }, 400 + log.length * 650);
      }, 900);
    }

    function finish(kind, a) {
      state.busy = false;
      var heading = a.name + (kind === "uninstall" ? " was uninstalled" : " was updated");
      setFooter('<svg class="sw-check" aria-hidden="true"><use href="#i-check"/></svg><span class="h">' + esc(heading) + '</span><button class="sw-dismiss" aria-label="Dismiss operation result" data-dismiss><svg aria-hidden="true"><use href="#i-close"/></svg></button>');
      if (kind === "uninstall") {
        var el = list.querySelector('[data-item="' + CSS.escape(a.key) + '"]');
        state.removed[a.key] = true;
        state.open = null;
        if (el && !reduceMotion) {
          el.style.height = el.offsetHeight + "px";
          el.classList.add("is-leaving");
          setTimeout(render, 480);
        } else render();
      } else {
        state.updated[a.key] = a.next;
        render();
      }
    }

    function resetDemo() {
      timers.forEach(clearTimeout); timers = [];
      state.removed = {}; state.updated = {}; state.measured = {}; state.open = null; state.busy = false;
      footer.classList.remove("is-open");
      scrim.classList.remove("is-open");
    }

    /* Events */

    list.addEventListener("click", function (e) {
      var act = e.target.closest("[data-act]");
      if (act) { e.stopPropagation(); openModal(act.dataset.act, act.dataset.key); return; }
      var row = e.target.closest("[data-toggle]");
      if (row) toggle(row.dataset.toggle);
    });
    list.addEventListener("keydown", function (e) {
      if ((e.key === "Enter" || e.key === " ") && e.target.matches("[data-toggle]")) { e.preventDefault(); toggle(e.target.dataset.toggle); }
    });

    search.addEventListener("input", function () { state.query = search.value; state.open = null; render(); });
    sourceSel.addEventListener("change", function () { state.source = sourceSel.value; state.open = null; render(); });
    showAllBtn.addEventListener("click", function () {
      state.showAll = !state.showAll;
      showAllBtn.setAttribute("aria-pressed", state.showAll);
      showAllBtn.textContent = state.showAll ? "Show apps" : "Show all";
      render();
    });
    rescanBtn.addEventListener("click", function () {
      if (state.busy) return;
      resetDemo();
      rescanBtn.classList.add("is-spinning");
      list.innerHTML = "";
      count.textContent = "Scanning…";
      later(function () { rescanBtn.classList.remove("is-spinning"); render({ scan: true }); }, 700);
    });
    Array.prototype.forEach.call(tabs, function (t) {
      t.addEventListener("click", function () {
        state.tab = t.dataset.tab; state.open = null;
        Array.prototype.forEach.call(tabs, function (x) { x.setAttribute("aria-selected", x === t); });
        render();
      });
    });

    scrim.addEventListener("click", function (e) {
      if (e.target === scrim || e.target.closest("[data-close]")) closeModal();
      else if (e.target.closest("[data-confirm]")) apply(modal.dataset.kind, modal.dataset.key);
    });
    root.addEventListener("keydown", function (e) {
      if (e.key === "Escape" && scrim.classList.contains("is-open")) closeModal();
    });
    footer.addEventListener("click", function (e) {
      if (e.target.closest("[data-dismiss]")) footer.classList.remove("is-open");
    });

    // First scan when the window scrolls into view.
    list.innerHTML = "";
    count.textContent = "Scanning…";
    badge.hidden = true;
    var started = false;
    function start() {
      if (started) return; started = true;
      later(function () { render({ scan: true }); }, 900);
    }
    if ("IntersectionObserver" in window) {
      var io = new IntersectionObserver(function (entries) {
        if (entries[0].isIntersecting) { start(); io.disconnect(); }
      }, { threshold: 0.25 });
      io.observe(root);
    } else start();
  }

  /* ---------- Sources explorer ---------- */

  var tabsEl = document.querySelector("[data-source-tabs]");
  var panel = document.querySelector("[data-source-panel]");
  if (tabsEl && panel) {
    var names = Object.keys(SOURCES);
    tabsEl.innerHTML = names.map(function (n, i) {
      var s = SOURCES[n];
      return '<button class="source-tab" role="tab" id="src-' + n + '" aria-selected="' + (i === 0) + '" tabindex="' + (i === 0 ? 0 : -1) + '" data-src="' + n + '">' +
        '<span class="source-dot" style="background:' + s.color + '"></span><span class="name">' + n + '</span><span class="tag">' + s.tag + "</span></button>";
    }).join("");

    var showSource = function (n, focus) {
      var s = SOURCES[n];
      Array.prototype.forEach.call(tabsEl.children, function (b) {
        var on = b.dataset.src === n;
        b.setAttribute("aria-selected", on); b.tabIndex = on ? 0 : -1;
        if (on && focus) b.focus();
      });
      panel.setAttribute("aria-labelledby", "src-" + n);
      var block = function (label, tpl) {
        if (!tpl) return '<div><p class="cmd-label">' + label + '</p><div class="cmd is-none">Not supported yet. No command runs.</div></div>';
        var body = tpl.replace(/\{pkg\}/g, s.sample).replace(/^pkexec /gm, '<span class="tok-priv">pkexec</span> ');
        return '<div><p class="cmd-label">' + label + '</p><div class="cmd">' + body + "</div></div>";
      };
      panel.innerHTML = '<div class="source-swap"><h3><span class="source-dot" style="background:' + s.color + '"></span>' + n +
        (s.soon ? ' <span class="soon">Next release</span>' : "") + "</h3>" +
        '<p class="what">' + s.what + "</p>" +
        '<div class="cmd-grid">' + block("Uninstall", s.uninstall) + block("Update", s.update) + "</div>" +
        '<p class="fine">' + s.fine + "</p></div>";
    };
    tabsEl.addEventListener("click", function (e) { var b = e.target.closest("[data-src]"); if (b) showSource(b.dataset.src); });
    tabsEl.addEventListener("keydown", function (e) {
      var keys = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 };
      if (!(e.key in keys)) return;
      e.preventDefault();
      var i = names.indexOf(document.activeElement.dataset.src);
      showSource(names[(i + keys[e.key] + names.length) % names.length], true);
    });
    showSource(names[0]);
  }

  /* ---------- Screenshot switch ---------- */

  var captions = ["Apps, tools and launchers from every source, sorted together.", "Open a row to see what it occupies on disk before you remove it."];
  var shotBtns = document.querySelectorAll("[data-shot]");
  Array.prototype.forEach.call(shotBtns, function (b) {
    b.addEventListener("click", function () {
      var n = b.dataset.shot;
      Array.prototype.forEach.call(shotBtns, function (x) { x.setAttribute("aria-selected", x === b); });
      Array.prototype.forEach.call(document.querySelectorAll("[data-shot-img]"), function (img) { img.hidden = img.dataset.shotImg !== n; });
      document.querySelector("[data-shot-caption]").textContent = captions[n];
    });
  });

  /* ---------- Install tabs ---------- */

  var INSTALL = {
    deb: { lines: ["sudo apt install ./scope_0.3.7_amd64.deb"], note: "Run it from the folder you downloaded the .deb to. The ./ tells apt to use the local file." },
    appimage: { lines: ["chmod +x Scope-0.3.7-x86_64.AppImage", "./Scope-0.3.7-x86_64.AppImage"], note: "Portable and self-contained. Self-updates are available on the .deb build." },
    source: { lines: ["git clone https://github.com/khurrambhutto/scope.git", "cd scope && cargo run"], note: "Needs Rust stable and GPUI's system libraries. The install guide lists them." }
  };
  var installCmd = document.querySelector("[data-install-cmd]");
  var installNote = document.querySelector("[data-install-note]");
  var installBtns = document.querySelectorAll("[data-install]");
  function showInstall(k) {
    var it = INSTALL[k];
    installCmd.innerHTML = '<button class="copy-button" type="button" data-copy="' + it.lines.join("\n").replace(/"/g, "&quot;") + '">Copy</button>' +
      it.lines.map(function (l) { return '<span class="line"><span class="prompt">$ </span>' + l + "</span>"; }).join("");
    installNote.textContent = it.note;
    Array.prototype.forEach.call(installBtns, function (b) { b.setAttribute("aria-selected", b.dataset.install === k); });
  }
  if (installCmd) {
    Array.prototype.forEach.call(installBtns, function (b) { b.addEventListener("click", function () { showInstall(b.dataset.install); }); });
    showInstall("deb");
  }

  /* ---------- Copy buttons (all pages) ---------- */

  document.addEventListener("click", function (e) {
    var btn = e.target.closest(".copy-button");
    if (!btn || !navigator.clipboard) return;
    navigator.clipboard.writeText(btn.dataset.copy).then(function () {
      btn.textContent = "Copied";
      btn.classList.add("is-copied");
      setTimeout(function () { btn.textContent = "Copy"; btn.classList.remove("is-copied"); }, 1600);
    });
  });

  /* ---------- Header border on scroll ---------- */

  var header = document.querySelector(".site-header");
  if (header) {
    var onScroll = function () { header.classList.toggle("is-scrolled", window.scrollY > 8); };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
  }
})();
