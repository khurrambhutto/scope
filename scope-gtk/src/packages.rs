//! Unified package list: scanning, filtering, search, and row building.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{glib, Align, Orientation};

use crate::app::App;
use crate::backend::package::{AppKind, InstalledPackage, PackageSource};
use crate::bridge;
use crate::tasks::TaskStatus;

/// Kick off a full background scan. Never blocks the UI; the list stays
/// usable during refreshes and is replaced when the scan completes.
pub fn start_scan(app: &Rc<App>) {
    let already_scanning = app.with_state(|s| {
        if s.scanning {
            true
        } else {
            s.scanning = true;
            false
        }
    });
    if already_scanning {
        return;
    }

    set_scan_ui(app, true);

    let backend = app.backend.clone();
    let app2 = app.clone();
    bridge::spawn(
        async move { backend.scan().await },
        move |scan| crate::app::apply_scan(&app2, scan),
    );
}

/// Reset scan-mode UI (spinner, subtitle, counters) after a scan settles.
pub fn set_scan_ui(app: &App, scanning: bool) {
    app.refresh_btn.set_sensitive(!scanning);
    app.refresh_icon.set_visible(!scanning);
    app.refresh_spinner.set_visible(scanning);
    if scanning {
        app.refresh_spinner.start();
    } else {
        app.refresh_spinner.stop();
    }
    update_counters(app);
}

/// Header/footer counters: subtitle, updates toggle label, sidebar task badge.
pub fn update_counters(app: &App) {
    let (total, updates, running, scanning) = app.with_state(|s| {
        (
            s.packages.len(),
            s.packages.iter().filter(|p| p.has_update).count(),
            s.tasks.iter().filter(|t| t.status == TaskStatus::Running).count(),
            s.scanning,
        )
    });

    let subtitle = if scanning {
        "Scanning… (first scan can take a minute)".to_string()
    } else {
        format!("{total} apps · {updates} updates")
    };
    app.window_title.set_subtitle(&subtitle);
    app.updates_tb.set_label(&format!("Updates ({updates})"));

    let badge = &app.tasks_badge;
    badge.set_visible(running > 0);
    badge.set_text(&running.to_string());
}

/// Apply current filters/search to the snapshot, then rebuild rows.
pub fn rebuild_list(app: &Rc<App>) {
    let visible = visible_packages(app);

    let empty_kind: &str = if app.with_state(|s| s.scanning) && visible.is_empty() {
        "loading"
    } else if visible.is_empty() {
        "empty"
    } else {
        "list"
    };
    app.list_stack.set_visible_child_name(empty_kind);

    if empty_kind != "list" {
        let empty_label = app
            .list_stack
            .child_by_name("empty")
            .and_downcast::<gtk::Label>()
            .expect("empty page is a label");
        let msg = if app.with_state(|s| !s.query.is_empty()) {
            "No packages match your search."
        } else if app.with_state(|s| s.updates_only) {
            "No updates available. Everything is up to date."
        } else {
            "No packages found."
        };
        empty_label.set_text(msg);
        return;
    }

    let list = &app.list_box;
    clear(list);

    for pkg in &visible {
        list.append(&build_row(app, pkg));
    }

    update_banners(app);

    if std::env::var_os("SCOPE_GTK_DEBUG").is_some() {
        let first_title = app
            .list_box
            .first_child()
            .and_downcast::<gtk::ListBoxRow>()
            .and_then(|row| row.child())
            .and_downcast::<adw::ActionRow>()
            .map(|r| r.title().to_string());
        let page = app.list_stack.visible_child_name().map(|n| n.to_string());
        bridge::debug_log(&format!(
            "rebuild_list done: {} visible, page={page:?}, first row title={first_title:?}",
            visible.len(),
        ));
    }
}

/// Filter + search + sort, mirroring the Tauri frontend's `usePackages`.
fn visible_packages(app: &App) -> Vec<InstalledPackage> {
    app.with_state(|s| {
        let query = s.query.to_lowercase();
        let updates_only = s.updates_only;
        let largest = s.largest_first;

        let source = dropdown_source(app);
        let kind = dropdown_kind(app);

        let mut items: Vec<InstalledPackage> = s
            .packages
            .iter()
            .filter(|p| !updates_only || p.has_update)
            .filter(|p| source.map_or(true, |src| p.source == src))
            .filter(|p| kind.map_or(true, |k| p.app_kind == k))
            .filter(|p| {
                query.is_empty() || matches_query(p, &query)
            })
            .cloned()
            .collect();

        if largest {
            items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        }
        items
    })
}

fn matches_query(p: &InstalledPackage, query: &str) -> bool {
    let name = p.display_name.as_deref().unwrap_or(&p.name);
    name.to_lowercase().contains(query)
        || p.name.to_lowercase().contains(query)
        || p.package_id.to_lowercase().contains(query)
        || p.version.to_lowercase().contains(query)
        || p.description
            .as_deref()
            .map_or(false, |d| d.to_lowercase().contains(query))
        || p.categories
            .as_deref()
            .map_or(false, |c| c.to_lowercase().contains(query))
}

// Filter mappings ------------------------------------------------------------

/// Dropdown index -> source filter (index 0 = "All sources").
pub fn dropdown_source(app: &App) -> Option<PackageSource> {
    match app.source_dd.selected() {
        1 => Some(PackageSource::Apt),
        2 => Some(PackageSource::Snap),
        3 => Some(PackageSource::Flatpak),
        4 => Some(PackageSource::AppImage),
        5 => Some(PackageSource::Manual),
        _ => None,
    }
}

/// Dropdown index -> kind filter (index 0 = "All kinds").
pub fn dropdown_kind(app: &App) -> Option<AppKind> {
    match app.kind_dd.selected() {
        1 => Some(AppKind::Gui),
        2 => Some(AppKind::Cli),
        3 => Some(AppKind::Unknown),
        _ => None,
    }
}

// Rows -----------------------------------------------------------------------

fn build_row(app: &Rc<App>, pkg: &InstalledPackage) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();

    let action = adw::ActionRow::new();

    // Icon: backend-resolved file (registered path only), else themed fallback.
    let icon = gtk::Image::new();
    icon.set_pixel_size(36);
    icon.set_valign(Align::Center);
    load_icon(&icon, pkg);
    action.add_prefix(&icon);

    let name = pkg.display_name.clone().unwrap_or_else(|| pkg.name.clone());
    action.set_title(&name);
    action.set_title_lines(1);

    let busy_label = app.with_state(|s| s.busy.get(&pkg.key).cloned());

    if let Some(kind) = busy_label.as_deref() {
        action.set_subtitle(&kind);
    } else if pkg.has_update && app.with_state(|s| s.updates_only) {
        let target = pkg.update_version.clone().unwrap_or_else(|| "latest".into());
        action.set_subtitle(&format!("v{} → v{} · {}", pkg.version, target, source_line(pkg)));
    } else {
        action.set_subtitle(&subtitle(pkg));
    }

    // Update pill button.
    if pkg.has_update && busy_label.is_none() {
        let update_btn = gtk::Button::with_label("Update");
        update_btn.add_css_class("suggested-action");
        update_btn.add_css_class("pill");
        update_btn.set_valign(Align::Center);
        let app2 = app.clone();
        let key = pkg.key.clone();
        update_btn.connect_clicked(move |_| {
            crate::ops::request(&app2, crate::backend::operations::Operation::Update, key.clone());
        });
        action.add_suffix(&update_btn);
    }

    // Uninstall icon button.
    if busy_label.is_none() {
        let remove_btn = gtk::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_valign(Align::Center);
        remove_btn.set_tooltip_text(Some("Uninstall"));
        let app2 = app.clone();
        let key = pkg.key.clone();
        remove_btn.connect_clicked(move |_| {
            crate::ops::request(&app2, crate::backend::operations::Operation::Uninstall, key.clone());
        });
        action.add_suffix(&remove_btn);
    }

    // Busy spinner.
    if busy_label.is_some() {
        let spinner = gtk::Spinner::new();
        spinner.start();
        spinner.set_valign(Align::Center);
        action.add_suffix(&spinner);
    }

    // Kebab menu: Details / Update / Uninstall.
    let kebab = build_kebab(app, pkg);
    action.add_suffix(&kebab);

    let app2 = app.clone();
    let key = pkg.key.clone();
    action.connect_activated(move |_| {
        crate::detail::show(&app2, key.clone());
    });

    row.set_child(Some(&action));
    row.set_activatable(true);
    row
}

fn build_kebab(app: &Rc<App>, pkg: &InstalledPackage) -> gtk::MenuButton {
    let kebab = gtk::MenuButton::new();
    kebab.set_icon_name("view-more-symbolic");
    kebab.add_css_class("flat");
    kebab.set_valign(Align::Center);

    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(Orientation::Vertical, 0);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    let add_item = |menu: &gtk::Box, label: &str, enabled: bool, app: &Rc<App>, key: &str, op: Option<crate::backend::operations::Operation>| {
        let btn = gtk::Button::with_label(label);
        btn.add_css_class("flat");
        btn.set_sensitive(enabled);
        if let Some(op) = op {
            let app2 = app.clone();
            let key = key.to_string();
            let pop = popover.clone();
            btn.connect_clicked(move |_| {
                pop.popdown();
                crate::ops::request(&app2, op, key.clone());
            });
        } else {
            // Details
            let app2 = app.clone();
            let key = key.to_string();
            let pop = popover.clone();
            btn.connect_clicked(move |_| {
                pop.popdown();
                crate::detail::show(&app2, key.clone());
            });
        }
        menu.append(&btn);
    };

    add_item(&menu, "Details", true, app, &pkg.key, None);
    add_item(
        &menu,
        "Update",
        pkg.has_update,
        app,
        &pkg.key,
        Some(crate::backend::operations::Operation::Update),
    );
    add_item(
        &menu,
        "Uninstall",
        true,
        app,
        &pkg.key,
        Some(crate::backend::operations::Operation::Uninstall),
    );

    popover.set_child(Some(&menu));
    kebab.set_popover(Some(&popover));
    kebab
}

// Helpers --------------------------------------------------------------------

fn subtitle(pkg: &InstalledPackage) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !pkg.version.is_empty() {
        parts.push(format!("v{}", pkg.version));
    }
    if pkg.size_bytes > 0 {
        parts.push(glib::format_size(pkg.size_bytes).to_string());
    }
    parts.push(source_line(pkg));
    parts.join(" · ")
}

fn source_line(pkg: &InstalledPackage) -> String {
    let source = match pkg.source {
        PackageSource::Apt => "APT",
        PackageSource::Snap => "Snap",
        PackageSource::Flatpak => "Flatpak",
        PackageSource::AppImage => "AppImage",
        PackageSource::Manual => "Manual",
    };
    match pkg.install_scope {
        Some(crate::backend::package::InstallScope::User) => format!("{source} · user"),
        Some(crate::backend::package::InstallScope::System) => format!("{source} · system"),
        None => source.to_string(),
    }
}

/// Load the row icon. The URL path is validated against the backend's icon
/// whitelist before any filesystem read, and the file read happens off-thread.
#[allow(deprecated)]
fn load_icon(image: &gtk::Image, pkg: &InstalledPackage) {
    let path = pkg
        .icon
        .as_deref()
        .and_then(bridge::icon_path_from_url)
        .filter(|p| crate::backend::icons::is_registered_path(p));

    match path {
        Some(path) => {
            let image = image.clone();
            let fallback = fallback_name(pkg).to_string();
            bridge::spawn(
                async move {
                    tokio::task::spawn_blocking(move || std::fs::read(&path).ok())
                        .await
                        .unwrap_or(None)
                },
                move |bytes| match bytes {
                    Some(bytes) => {
                        let loader = gtk::gdk_pixbuf::PixbufLoader::new();
                        loader.set_size(72, 72);
                        if loader.write(&bytes).is_ok() {
                            if let Some(pixbuf) = loader.pixbuf() {
                                image.set_from_pixbuf(Some(&pixbuf));
                            }
                        }
                        let _ = loader.close();
                    }
                    None => set_fallback_icon(&image, &fallback),
                },
            );
        }
        None => set_fallback_icon(image, fallback_name(pkg)),
    }
}

fn fallback_name(pkg: &InstalledPackage) -> &'static str {
    match pkg.app_kind {
        AppKind::Gui => "application-x-executable-symbolic",
        AppKind::Cli => "utilities-terminal-symbolic",
        AppKind::Unknown => "package-x-generic-symbolic",
    }
}

fn set_fallback_icon(image: &gtk::Image, name: &str) {
    image.set_icon_name(Some(name));
}

/// Show per-source scan errors as a dismissible warning banner.
fn update_banners(app: &App) {
    let availability = app.with_state(|s| s.availability.clone());
    let mut messages: Vec<String> = Vec::new();
    if let Some(e) = &availability.apt_error {
        messages.push(format!("APT: {e}"));
    }
    if let Some(e) = &availability.snap_error {
        messages.push(format!("Snap: {e}"));
    }
    if let Some(e) = &availability.flatpak_error {
        messages.push(format!("Flatpak: {e}"));
    }

    if messages.is_empty() {
        app.banner_revealer.set_reveal_child(false);
    } else {
        app.banner_label.set_text(&messages.join("\n"));
        app.banner_revealer.set_reveal_child(true);
    }
}

fn clear(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}
