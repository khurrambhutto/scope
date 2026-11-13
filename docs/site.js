/* Scope for Linux website: replica, tabs, copy, menu. Works without the motion CDN.
   Facts mirror AGENTS.md (command table) for v0.3.9. */
(function () {
  "use strict";

  var reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- Source facts (AGENTS.md command table) ---------- */

  var APT_PURGE = "pkexec env DEBIAN_FRONTEND=noninteractive apt-get -o APT::Get::AutomaticRemove=false purge -y -- {pkg}";
  var SOURCES = {
    APT: { color: "var(--apt)", auth: true, uninstall: APT_PURGE, update: "pkexec env DEBIAN_FRONTEND=noninteractive apt install -y {pkg}" },
    Snap: { color: "var(--snap)", auth: true, uninstall: "pkexec snap remove {pkg}", update: "pkexec snap refresh {pkg}" },
    Flatpak: { color: "var(--flatpak)", auth: false, uninstall: "flatpak uninstall -y --user {pkg}", update: "flatpak update -y --user {pkg}" },
    AppImage: { color: "var(--appimage)", auth: false, uninstall: "gio trash ~/AppImages/{pkg}", update: null },
    uv: { color: "var(--uv)", auth: false, uninstall: "uv tool uninstall {pkg}", update: "uv tool upgrade {pkg}" },
    pipx: { color: "var(--pipx)", auth: false, uninstall: "pipx uninstall {pkg}", update: "pipx upgrade {pkg}" },
    Cargo: { color: "var(--cargo)", auth: false, uninstall: "cargo uninstall --root ~/.cargo {pkg}", update: "cargo install --root ~/.cargo {pkg}" },
    npm: { color: "var(--npm)", auth: false, uninstall: "npm uninstall --global --prefix ~/.local {pkg}", update: "npm update --global --prefix ~/.local {pkg}" },
    Bun: { color: "var(--bun)", auth: false, uninstall: "bun remove --global {pkg}", update: "bun update --global {pkg}" },
    Desktop: { color: "var(--desktop)", readOnly: "Listed for discovery. Scope won’t touch files it can’t attribute to a package manager." },
    Steam: { color: "var(--steam)", readOnly: "Read-only. Open it in Steam to update or uninstall." },
    JetBrains: { color: "var(--jetbrains)", readOnly: "Read-only. Open Toolbox to update or uninstall IDEs." }
  };

  function cmd(source, kind, pkg) {
    var s = SOURCES[source];
    var tpl = s && s[kind];
    return tpl ? tpl.replace("{pkg}", pkg) : null;
  }

  /* ---------- Demo data ---------- */

  var KB = 1024, MB = KB * 1024, GB = MB * 1024;
  function fmt(bytes) {
    if (bytes == null) return "—";
    if (bytes >= GB) return (bytes / GB).toFixed(1).replace(/\.0$/, "") + " GB";
    if (bytes >= MB) return (bytes >= 10 * MB ? Math.round(bytes / MB) : (bytes / MB).toFixed(1).replace(/\.0$/, "")) + " MB";
    return Math.max(1, Math.round(bytes / KB)) + " KB";
  }

  // parts: [label, path, bytes, kept]
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
    { name: "Counter-Strike 2", pkg: "730", src: "Steam", ver: null, size: 34 * GB, initials: "CS",
      parts: [["Game files", "~/.local/share/Steam/steamapps/common/Counter-Strike Global Offensive", 34 * GB]] },
    { name: "Cursor", pkg: "cursor", src: "APT", ver: "3.23.23-1791166813", next: "3.24.2-1791904417", size: 942 * MB, icon: "cursor",
      parts: [["Program files", "/usr/share/cursor", 939 * MB], ["Package source", "/etc/apt/sources.list.d/cursor.list", 4 * KB], ["Launcher", "/usr/share/applications/cursor.desktop", 4 * KB]] },
    { name: "diary", pkg: "diary", src: "APT", ver: "0.1.5", size: 19 * MB, icon: "diary",
      parts: [["Program files", "/usr/bin/diary", 19 * MB], ["Launcher", "/usr/share/applications/diary.desktop", 4 * KB]] },
    { name: "Firefox", pkg: "firefox", src: "Snap", ver: "157.0.1-1", next: "157.0.2-1", size: 262 * MB, icon: "firefox",
      parts: [["Application", "/var/lib/snapd/snaps/firefox_7120.snap", 262 * MB], ["Older revision", "/var/lib/snapd/snaps/firefox_7084.snap", 259 * MB], ["User data", "~/snap/firefox", 1.2 * GB], ["System data", "/var/snap/firefox", 16 * KB]] },
    { name: "Firmware Updater", pkg: "firmware-updater", src: "Snap", ver: "0+git.64c41a0", size: 13 * MB, icon: "firmware",
      parts: [["Application", "/var/lib/snapd/snaps/firmware-updater_210.snap", 13 * MB], ["User data", "~/snap/firmware-updater", 1.1 * MB], ["System data", "/var/snap/firmware-updater", 4 * KB]] },
    { name: "Flatseal", pkg: "com.github.tchx84.Flatseal", src: "Flatpak", scope: "user", ver: "2.3.1", size: 4.1 * MB, initials: "Fs",
      parts: [["Deployment", "~/.local/share/flatpak/app/com.github.tchx84.Flatseal", 4.1 * MB], ["App data", "~/.var/app/com.github.tchx84.Flatseal", 220 * KB, true]] },
    { name: "httpie", pkg: "httpie", src: "pipx", ver: "3.2.4", size: 21 * MB, initials: "ht",
      parts: [["Environment", "~/.local/share/pipx/venvs/httpie", 21 * MB], ["Commands", "~/.local/bin/http, https, httpie", 12 * KB]] },
    { name: "Obsidian", pkg: "Obsidian-1.9.14.AppImage", src: "AppImage", ver: "1.9.14", size: 128 * MB, initials: "Ob",
      parts: [["AppImage", "~/AppImages/Obsidian-1.9.14.AppImage", 128 * MB], ["Launcher", "~/.local/share/applications/obsidian.desktop", 4 * KB], ["App data", "~/.config/obsidian", 9 * MB, true]] },
    { name: "prettier", pkg: "prettier", src: "Bun", ver: "3.6.2", size: 8.4 * MB, initials: "pr",
      parts: [["Package", "~/.bun/install/global/node_modules/prettier", 8.4 * MB], ["Command", "~/.bun/bin/prettier", 4 * KB]] },
    { name: "ripgrep", pkg: "ripgrep", src: "Cargo", ver: "14.1.1", size: 5.2 * MB, initials: "rg",
      parts: [["Binary", "~/.cargo/bin/rg", 5.2 * MB]] },
    { name: "ruff", pkg: "ruff", src: "uv", ver: "0.13.3", next: "0.14.0", size: 33 * MB, initials: "ru",
      parts: [["Environment", "~/.local/share/uv/tools/ruff", 33 * MB], ["Command", "~/.local/bin/ruff", 4 * KB]] },
    { name: "RustRover", pkg: "RustRover", src: "JetBrains", ver: "2025.2.3", size: 2.1 * GB, initials: "RR",
      parts: [["IDE", "~/.local/share/JetBrains/Toolbox/apps/rustrover", 2.1 * GB]] },
    { name: "typescript", pkg: "typescript", src: "npm", ver: "5.9.2", next: "5.9.3", size: 23 * MB, initials: "ts",
      parts: [["Package", "~/.local/lib/node_modules/typescript", 23 * MB], ["Commands", "~/.local/bin/tsc, tsserver", 8 * KB]] },
    // Revealed by Show all
    { name: "build-essential", pkg: "build-essential", src: "APT", ver: "12.10ubuntu1", size: 21 * KB, initials: "be", hidden: true,
      parts: [["Documentation", "/usr/share/doc/build-essential", 21 * KB]] },
    { name: "curl", pkg: "curl", src: "APT", ver: "8.12.1-3ubuntu1", size: 520 * KB, initials: "cu", hidden: true,
      parts: [["Program files", "/usr/bin/curl", 330 * KB], ["Documentation", "/usr/share/doc/curl", 190 * KB]] },
    { name: "linux-generic", pkg: "linux-generic", src: "APT", ver: "6.17.0-34.34", size: 12 * KB, initials: "lg", hidden: true, protected: true, parts: [] },
    { name: "ubuntu-desktop", pkg: "ubuntu-desktop", src: "APT", ver: "1.539", size: 8 * KB, initials: "ud", hidden: true, protected: true, parts: [] }
  ];
  APPS.forEach(function (a) { a.key = a.src + ":" + a.pkg; });

  function esc(s) {
    return String(s).replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; });
  }

  /* ---------- Replica ---------- */

  var root = document.getElementById("demo");
  if (root) initDemo(root);

  function initDemo(root) {
    var $ = function (sel) { return root.querySelector(sel); };
    var list = $("[data-list]"), search = $("[data-search]"), count = $("[data-count]");
    var sourceSel = $("[data-source]"), showAllBtn = $("[data-showall]"), rescanBtn = $("[data-rescan]");
    var footer = $("[data-footer]"), scrim = $("[data-scrim]"), modal = $("[data-modal]"), badge = $("[data-update-count]");
    var tabs = root.querySelectorAll(".sw-tab");
    var state = { tab: "apps", query: "", source: "", showAll: false, open: null, measured: {}, busy: false, removed: {}, updated: {} };
    var timers = [], lastFocus = null;

    function later(fn, ms) { var t = setTimeout(fn, reduceMotion ? 0 : ms); timers.push(t); return t; }
    function find(key) { return APPS.find(function (x) { return x.key === key; }); }
    function locked(a) { return a.protected || !!SOURCES[a.src].readOnly; }

    function visible() {
      var q = state.query.trim().toLowerCase();
      return APPS.filter(function (a) {
        if (state.removed[a.key]) return false;
        if (a.hidden && !state.showAll) return false;
        if (state.source && a.src !== state.source) return false;
        if (state.tab === "updates" && !(a.next && !state.updated[a.key])) return false;
        if (q && a.name.toLowerCase().indexOf(q) < 0 && a.pkg.toLowerCase().indexOf(q) < 0 && a.src.toLowerCase() !== q) return false;
        return true;
      }).sort(function (x, y) { return x.name.toLowerCase() < y.name.toLowerCase() ? -1 : 1; });
    }
    function pendingUpdates() {
      return APPS.filter(function (a) { return a.next && !state.updated[a.key] && !state.removed[a.key]; }).length;
    }

    function iconHTML(a) {
      if (a.icon) return '<span class="sw-icon"><img src="demo/' + a.icon + '.png" width="36" height="36" alt=""></span>';
      return '<span class="sw-icon" style="background:' + SOURCES[a.src].color + '">' + esc(a.initials) + "</span>";
    }
    function srcLabel(a) { return a.src === "Flatpak" ? "Flatpak (" + a.scope + ")" : a.src === "JetBrains" ? "JetBrains Toolbox" : a.src; }
    function metaHTML(a) {
      if (state.tab === "updates" && a.next) return esc(a.ver) + ' <span class="sw-arrow" aria-label="to">→</span> ' + esc(a.next) + " · " + srcLabel(a);
      return (a.ver ? esc(state.updated[a.key] || a.ver) : "—") + '<span class="sw-size"> · ' + fmt(a.size) + "</span> · " + srcLabel(a);
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
        var s = m[i] ? '<span class="s' + (p[3] ? " is-kept" : "") + '">' + (p[3] ? "Kept" : fmt(p[2])) + "</span>" : '<span class="s is-pending" aria-label="Measuring"></span>';
        return '<div class="sw-brow"><span class="k">' + esc(p[0]) + '</span><span class="p">' + esc(p[1]) + "</span>" + s + "</div>";
      }).join("");
      if (!forModal && SOURCES[a.src].readOnly) rows += '<p class="sw-note">' + SOURCES[a.src].readOnly + "</p>";
      return rows;
    }
    function actionsHTML(a, open) {
      if (state.tab === "updates") {
        return '<span class="sw-actions"><button class="sw-pill is-quiet sw-update-pill" data-act="update" data-key="' + esc(a.key) + '"' + (state.busy ? " disabled" : "") + ">Update</button></span>";
      }
      if (!open) return "";
      if (locked(a)) {
        var label = a.protected ? "Protected" : a.src === "Steam" ? "Open in Steam" : a.src === "JetBrains" ? "Open Toolbox" : "Not supported";
        return '<span class="sw-actions"><button class="sw-pill" disabled>' + label + "</button></span>";
      }
      var r = removable(a);
      var done = state.measured[a.key] && Object.keys(state.measured[a.key]).length === a.parts.length;
      return '<span class="sw-actions"><span class="sw-removes">' + (r == null ? "" : "Removes ~" + fmt(r) + (done ? "" : "+")) +
        '</span><button class="sw-pill" data-act="uninstall" data-key="' + esc(a.key) + '"' + (state.busy ? " disabled" : "") + ">Uninstall</button></span>";
    }
    function itemHTML(a) {
      var open = state.open === a.key && state.tab === "apps";
      return '<div class="sw-item' + (open ? " is-open" : "") + '" data-item="' + esc(a.key) + '">' +
        '<div class="sw-row" role="button" tabindex="0" aria-expanded="' + open + '" data-toggle="' + esc(a.key) + '">' + iconHTML(a) +
        '<span class="sw-main"><span class="sw-name">' + esc(a.name) + '</span><span class="sw-meta">' + metaHTML(a) + "</span></span>" + actionsHTML(a, open) + "</div>" +
        '<div class="sw-detail"><div class="sw-detail-inner"><div class="sw-breakdown">' + (open ? breakdownHTML(a) : "") + "</div></div></div></div>";
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
        later(function () { list.classList.remove("is-scanning"); }, 1300);
      }
    }
    function refreshItem(key) {
      var el = list.querySelector('[data-item="' + CSS.escape(key) + '"]');
      var a = find(key);
      if (!el || !a) return;
      var open = state.open === key;
      var actions = el.querySelector(".sw-actions"), html = actionsHTML(a, open);
      if (actions) actions.outerHTML = html || "";
      else if (html) el.querySelector(".sw-row").insertAdjacentHTML("beforeend", html);
      if (open) el.querySelector(".sw-breakdown").innerHTML = breakdownHTML(a);
    }
    function measure(a) {
      if (state.measured[a.key]) return;
      state.measured[a.key] = {};
      a.parts.forEach(function (_, i) {
        later(function () { state.measured[a.key][i] = true; if (state.open === a.key) refreshItem(a.key); }, 260 + i * 210 + Math.random() * 160);
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
      if (!state.open) return;
      var a = find(key);
      measure(a);
      var el = list.querySelector('[data-item="' + CSS.escape(key) + '"]');
      el.querySelector(".sw-breakdown").innerHTML = breakdownHTML(a);
      refreshItem(key);
      void el.offsetHeight;
      el.classList.add("is-open");
      el.querySelector(".sw-row").setAttribute("aria-expanded", "true");
      later(function () {
        var lr = list.getBoundingClientRect(), er = el.getBoundingClientRect();
        if (er.bottom > lr.bottom) list.scrollBy({ top: er.bottom - lr.bottom + 12, behavior: reduceMotion ? "auto" : "smooth" });
      }, 380);
    }

    function openModal(kind, key) {
      var a = find(key);
      if (!a || state.busy) return;
      lastFocus = document.activeElement;
      var rows = [["Package", a.pkg]];
      if (a.scope) rows.push(["Scope", a.scope]);
      if (kind === "update") { rows.push(["Current version", a.ver]); rows.push(["Target version", a.next]); }
      else { rows.push(["Version", a.ver]); rows.push(["Size", fmt(a.size)]); }
      rows.push(["Command", cmd(a.src, kind, a.pkg)]);
      var html = '<div class="sw-modal-head"><h3 id="sw-modal-title">' + esc((kind === "uninstall" ? "Uninstall " : "Update ") + a.name) + "</h3>" +
        '<button class="sw-iconbtn" style="border:0" aria-label="Close dialog" data-close><svg aria-hidden="true"><use href="#i-close"/></svg></button></div>' +
        '<div class="sw-modal-body"><div class="sw-meta-rows">' + rows.map(function (r) {
          return '<div class="sw-brow"><span class="k">' + esc(r[0]) + '</span><span class="p' + (r[0] === "Command" ? " is-cmd" : "") + '">' + esc(r[1]) + "</span></div>";
        }).join("") + "</div>";
      if (a.src === "APT") html += '<p class="sw-sim">' + (kind === "uninstall" ? "APT simulation purges 1 package. Settings in your home folder stay." : "APT simulation changes 1 package and removes 0.") + "</p>";
      if (kind === "uninstall" && a.parts.length) { state.measured[a.key] = {}; html += "<div data-modal-breakdown>" + breakdownHTML(a, true) + "</div>"; }
      html += '</div><div class="sw-modal-foot"><button class="sw-pill is-quiet" data-close>Cancel</button><button class="sw-pill" data-confirm>Confirm</button></div>';
      modal.innerHTML = html;
      modal.dataset.kind = kind;
      modal.dataset.key = key;
      scrim.classList.add("is-open");
      document.dispatchEvent(new CustomEvent("scope:modal", { detail: true }));
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
      document.dispatchEvent(new CustomEvent("scope:modal", { detail: false }));
      if (lastFocus && document.contains(lastFocus)) lastFocus.focus();
    }
    function setFooter(html) { footer.innerHTML = html; footer.classList.add("is-open"); }

    function apply(kind, key) {
      var a = find(key);
      closeModal();
      state.busy = true;
      render();
      var hint = SOURCES[a.src].auth ? '<span class="t">Polkit asks you to authenticate.</span>' : "";
      var log = kind === "uninstall"
        ? (a.src === "Snap" ? ["Save data of snap “" + a.pkg + "” in automatic snapshot set", "Remove data for snap “" + a.pkg + "”", a.pkg + " removed"]
          : a.src === "APT" ? ["Removing " + a.pkg + " (" + a.ver + ") …", "Purging configuration files for " + a.pkg + " (" + a.ver + ") …", "Processing triggers for desktop-file-utils …"]
          : a.src === "AppImage" ? ["Moving " + a.pkg + " to the Trash", "Removing its launcher and icons", "Done."]
          : ["Uninstalling " + a.pkg + " …", "Done."])
        : (a.src === "Snap" ? ["Download snap “" + a.pkg + "” (" + a.next + ")", "Mount snap “" + a.pkg + "”", a.pkg + " " + a.next + " refreshed"]
          : a.src === "APT" ? ["Get:1 " + a.pkg + " amd64 " + a.next, "Unpacking " + a.pkg + " (" + a.next + ") over (" + a.ver + ") …", "Setting up " + a.pkg + " (" + a.next + ") …"]
          : ["Resolving " + a.pkg + " …", "Upgraded " + a.pkg + " from " + a.ver + " to " + a.next]);
      setFooter('<span class="sw-spin"></span><span><span class="h" style="display:block">Checking ' + esc(a.name) + "…</span>" + hint + "</span>");
      later(function () {
        setFooter('<span class="sw-spin"></span><span><span class="h" style="display:block">' + esc((kind === "uninstall" ? "Uninstalling " : "Updating ") + a.name + "…") + "</span>" + hint + '</span><span class="sw-log" data-log></span>');
        log.forEach(function (line, i) { later(function () { var l = footer.querySelector("[data-log]"); if (l) l.textContent = line; }, 300 + i * 650); });
        later(function () { finish(kind, a); }, 400 + log.length * 650);
      }, 900);
    }
    function finish(kind, a) {
      state.busy = false;
      setFooter('<svg class="sw-check" aria-hidden="true"><use href="#i-check"/></svg><span class="h">' + esc(a.name + (kind === "uninstall" ? " was uninstalled" : " was updated")) +
        '</span><button class="sw-dismiss" aria-label="Dismiss operation result" data-dismiss><svg aria-hidden="true"><use href="#i-close"/></svg></button>');
      if (kind === "uninstall") {
        var el = list.querySelector('[data-item="' + CSS.escape(a.key) + '"]');
        state.removed[a.key] = true;
        state.open = null;
        if (el && !reduceMotion) { el.style.height = el.offsetHeight + "px"; el.classList.add("is-leaving"); setTimeout(render, 480); }
        else render();
      } else { state.updated[a.key] = a.next; render(); }
    }
    function resetDemo() {
      timers.forEach(clearTimeout); timers = [];
      state.removed = {}; state.updated = {}; state.measured = {}; state.open = null; state.busy = false;
      footer.classList.remove("is-open");
      scrim.classList.remove("is-open");
    }

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
    root.addEventListener("keydown", function (e) { if (e.key === "Escape" && scrim.classList.contains("is-open")) closeModal(); });
    footer.addEventListener("click", function (e) { if (e.target.closest("[data-dismiss]")) footer.classList.remove("is-open"); });

    // "/" focuses the replica search, unless typing somewhere already.
    document.addEventListener("keydown", function (e) {
      if (e.key !== "/" || e.metaKey || e.ctrlKey || e.altKey) return;
      var t = e.target;
      if (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName)) return;
      e.preventDefault();
      search.focus({ preventScroll: true });
      root.scrollIntoView({ block: "center", behavior: reduceMotion ? "auto" : "smooth" });
    });

    // First scan when the window scrolls into view.
    list.innerHTML = "";
    count.textContent = "Scanning…";
    var started = false;
    function start() { if (started) return; started = true; later(function () { render({ scan: true }); }, 700); }
    if ("IntersectionObserver" in window) {
      var io = new IntersectionObserver(function (entries) { if (entries[0].isIntersecting) { start(); io.disconnect(); } }, { threshold: 0 });
      io.observe(root);
    } else start();
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
    deb: { lines: ["sudo apt install ./scope_0.3.9_amd64.deb"], note: "Run it from the folder you downloaded the .deb to. The ./ tells apt to use the local file." },
    appimage: { lines: ["chmod +x Scope-0.3.9-x86_64.AppImage", "./Scope-0.3.9-x86_64.AppImage"], note: "Portable and self-contained. Most AppImages need libfuse2t64 to run. Self-updates are available on the .deb build." },
    source: { lines: ["git clone https://github.com/khurrambhutto/scope.git", "cd scope && cargo run"], note: "Needs Rust stable and GPUI's system libraries. The install guide lists them." }
  };
  var installCmd = document.querySelector("[data-install-cmd]");
  var installNote = document.querySelector("[data-install-note]");
  var installBtns = document.querySelectorAll("[data-install]");
  function showInstall(k) {
    var it = INSTALL[k];
    installCmd.innerHTML = '<button class="copy-button" type="button" data-copy="' + esc(it.lines.join("\n")) + '">Copy</button>' +
      it.lines.map(function (l) { return '<span class="line"><span class="prompt">$ </span>' + esc(l) + "</span>"; }).join("");
    installNote.textContent = it.note;
    Array.prototype.forEach.call(installBtns, function (b) { b.setAttribute("aria-selected", b.dataset.install === k); });
  }
  if (installCmd && installBtns.length) {
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

  /* ---------- Header: hairline on scroll, mobile menu ---------- */

  var header = document.querySelector(".site-header");
  if (header) {
    var onScroll = function () { header.classList.toggle("is-scrolled", window.scrollY > 8); };
    window.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
  }
  var menuBtn = document.querySelector("[data-menu]");
  var navLinks = document.getElementById("nav-links");
  if (menuBtn && navLinks) {
    var setMenu = function (open, focusBack) {
      navLinks.classList.toggle("is-open", open);
      menuBtn.setAttribute("aria-expanded", open);
      document.dispatchEvent(new CustomEvent("scope:modal", { detail: open }));
      if (!open && focusBack) menuBtn.focus();
    };
    menuBtn.addEventListener("click", function () { setMenu(menuBtn.getAttribute("aria-expanded") !== "true"); });
    navLinks.addEventListener("click", function (e) { if (e.target.closest("a")) setMenu(false); });
    document.addEventListener("keydown", function (e) { if (e.key === "Escape" && navLinks.classList.contains("is-open")) setMenu(false, true); });
  }
})();
