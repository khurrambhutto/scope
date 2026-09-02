//! Package detail page (pushed onto the NavigationView from a row).

use std::rc::Rc;

use adw::prelude::*;
use gtk::{Align, Orientation};

use crate::app::App;
use crate::backend::package::{AppKind, PackageSource};
use crate::backend::safety;

/// Push the detail page for a package key, filling in all metadata.
pub fn show(app: &Rc<App>, key: String) {
    let Some(pkg) = app.package(&key) else {
        app.toast("Package is no longer in the current scan.");
        return;
    };

    *app.detail_key.borrow_mut() = Some(key);

    // Icon.
    let icon = &app.d_icon;
    icon.set_icon_name(Some(match pkg.app_kind {
        AppKind::Gui => "application-x-executable-symbolic",
        AppKind::Cli => "utilities-terminal-symbolic",
        AppKind::Unknown => "package-x-generic-symbolic",
    }));
    load_detail_icon(app, &pkg);

    app.d_title.set_text(&pkg.display_name.clone().unwrap_or_else(|| pkg.name.clone()));

    match &pkg.description {
        Some(d) if !d.is_empty() => {
            app.d_desc.set_text(d);
            app.d_desc.set_visible(true);
        }
        _ => app.d_desc.set_visible(false),
    }

    app.d_id.set_text(&crate::bridge::display_id(&pkg));
    app.d_source.set_text(
        match pkg.source {
            PackageSource::Apt => "APT",
            PackageSource::Snap => "Snap",
            PackageSource::Flatpak => "Flatpak",
            PackageSource::AppImage => "AppImage",
            PackageSource::Manual => "Manual install",
        },
    );
    app.d_scope.set_text(match pkg.install_scope {
        Some(crate::backend::package::InstallScope::User) => "User",
        Some(crate::backend::package::InstallScope::System) => "System-wide",
        None => "—",
    });

    let version = if pkg.has_update {
        let target = pkg.update_version.clone().unwrap_or_else(|| "latest".into());
        format!("{} → {}", pkg.version, target)
    } else if pkg.version.is_empty() {
        "—".to_string()
    } else {
        pkg.version.clone()
    };
    app.d_version.set_text(&version);

    let size_text = if pkg.size_bytes > 0 {
        gtk::glib::format_size(pkg.size_bytes).to_string()
    } else {
        "—".to_string()
    };
    app.d_size.set_text(&size_text);

    let kind = match pkg.app_kind {
        AppKind::Gui => "GUI app".to_string(),
        AppKind::Cli => "CLI tool".to_string(),
        AppKind::Unknown => "Unclassified".to_string(),
    };
    let kind = if pkg.terminal { format!("{kind} (terminal)") } else { kind };
    app.d_kind.set_text(&kind);

    app.d_categories.set_text(match &pkg.categories {
        Some(c) if !c.is_empty() => c,
        _ => "—",
    });

    let protection = safety::check_package(pkg.source, &pkg.package_id);
    if protection.protected {
        app.d_protection.set_text(
            protection
                .reason
                .as_deref()
                .unwrap_or("System-critical; removal is blocked."),
        );
        app.d_protection.add_css_class("error");
        app.d_uninstall_btn.set_sensitive(false);
    } else {
        app.d_protection.set_text("Not protected");
        app.d_protection.remove_css_class("error");
        app.d_uninstall_btn.set_sensitive(true);
    }

    refresh_buttons(app);

    app.nav_view.push(&app.detail_page);
}

/// Recompute detail action sensitivity from busy state.
pub fn refresh_buttons(app: &App) {
    let Some(key) = app.detail_key.borrow().clone() else {
        return;
    };
    let Some(pkg) = app.package(&key) else {
        // Package vanished from the current snapshot (e.g. removed).
        let visible = app
            .nav_view
            .visible_page()
            .map(|p| p == app.detail_page)
            .unwrap_or(false);
        if visible {
            app.nav_view.pop();
        }
        *app.detail_key.borrow_mut() = None;
        return;
    };

    let busy = app.with_state(|s| s.busy.contains_key(&key));
    let protection = safety::check_package(pkg.source, &pkg.package_id);

    app.d_update_btn.set_sensitive(pkg.has_update && !busy);
    app.d_uninstall_btn.set_sensitive(!protection.protected && !busy);

    if busy {
        if let Some(label) = app.with_state(|s| s.busy.get(&key).cloned()) {
            app.d_update_btn.set_label(label.trim_end_matches('…'));
            app.d_uninstall_btn.set_label(label.trim_end_matches('…'));
        }
    } else {
        app.d_update_btn.set_label("Update");
        app.d_uninstall_btn.set_label("Uninstall");
    }
}

/// All detail-page widgets the `App` needs handles to.
pub struct DetailWidgets {
    pub page: adw::NavigationPage,
    pub icon: gtk::Image,
    pub title_label: gtk::Label,
    pub desc: gtk::Label,
    pub id: gtk::Label,
    pub source: gtk::Label,
    pub scope: gtk::Label,
    pub version: gtk::Label,
    pub size: gtk::Label,
    pub kind: gtk::Label,
    pub categories: gtk::Label,
    pub protection: gtk::Label,
    pub update_btn: gtk::Button,
    pub uninstall_btn: gtk::Button,
}

/// Build the detail page widgets. Wiring happens in `window.rs` once the
/// `Rc<App>` exists.
#[allow(unused_assignments)]
pub fn build_widgets() -> DetailWidgets {
    let page_box = gtk::Box::new(Orientation::Vertical, 12);
    page_box.set_margin_top(24);
    page_box.set_margin_bottom(24);
    page_box.set_margin_start(24);
    page_box.set_margin_end(24);

    // Header: icon + name + description.
    let header = gtk::Box::new(Orientation::Horizontal, 16);
    let icon = gtk::Image::from_icon_name("application-x-executable-symbolic");
    icon.set_pixel_size(64);
    icon.set_valign(Align::Start);
    header.append(&icon);

    let head_text = gtk::Box::new(Orientation::Vertical, 4);
    let title_label = gtk::Label::new(None);
    title_label.set_xalign(0.0);
    title_label.set_wrap(true);
    title_label.add_css_class("title-2");
    head_text.append(&title_label);

    let desc = gtk::Label::new(None);
    desc.set_xalign(0.0);
    desc.set_wrap(true);
    desc.add_css_class("dim-label");
    head_text.append(&desc);
    header.append(&head_text);
    header.set_hexpand(true);
    page_box.append(&header);

    // Metadata list.
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.set_margin_top(8);

    macro_rules! meta_row {
        ($label:expr, $value:ident) => {{
            let row = adw::ActionRow::new();
            row.set_title($label);
            let value = gtk::Label::new(Some("—"));
            value.set_xalign(1.0);
            value.set_wrap(true);
            value.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            row.add_suffix(&value);
            $value = value;
            list.append(&row);
        }};
    }

    let mut id = gtk::Label::default();
    let mut source = gtk::Label::default();
    let mut scope = gtk::Label::default();
    let mut version = gtk::Label::default();
    let mut size = gtk::Label::default();
    let mut kind = gtk::Label::default();
    let mut categories = gtk::Label::default();
    let mut protection = gtk::Label::default();

    meta_row!("Package ID", id);
    meta_row!("Source", source);
    meta_row!("Install scope", scope);
    meta_row!("Version", version);
    meta_row!("Installed size", size);
    meta_row!("Kind", kind);
    meta_row!("Categories", categories);
    meta_row!("Protection", protection);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_child(Some(&list));
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_vexpand(true);
    page_box.append(&scrolled);

    // Actions.
    let actions = gtk::Box::new(Orientation::Horizontal, 12);
    actions.set_halign(Align::End);

    let update_btn = gtk::Button::with_label("Update");
    update_btn.add_css_class("suggested-action");
    update_btn.set_sensitive(false);

    let uninstall_btn = gtk::Button::with_label("Uninstall");
    uninstall_btn.add_css_class("destructive-action");
    uninstall_btn.set_sensitive(false);

    actions.append(&update_btn);
    actions.append(&uninstall_btn);
    page_box.append(&actions);

    let hint = gtk::Label::new(Some("Preview-first · protected packages are blocked"));
    hint.add_css_class("dim-label");
    hint.set_halign(Align::End);
    page_box.append(&hint);

    let title = adw::WindowTitle::new("Details", "");
    let header_bar = adw::HeaderBar::new();
    header_bar.set_title_widget(Some(&title));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header_bar);
    toolbar.set_content(Some(&page_box));

    let page = adw::NavigationPage::new(&toolbar, "Details");

    DetailWidgets {
        page,
        icon,
        title_label,
        desc,
        id,
        source,
        scope,
        version,
        size,
        kind,
        categories,
        protection,
        update_btn,
        uninstall_btn,
    }
}

/// Load the detail icon from the backend-resolved path (whitelist-checked).
/// Uses the shared pixbuf cache in `packages` so re-opening details never
/// re-reads or re-decodes the icon from disk.
#[allow(deprecated)]
fn load_detail_icon(app: &Rc<App>, pkg: &crate::backend::package::InstalledPackage) {
    const SIZE: i32 = 128;
    let path = pkg
        .icon
        .as_deref()
        .and_then(crate::bridge::icon_path_from_url)
        .filter(|p| crate::backend::icons::is_registered_path(p));

    let Some(path) = path else { return };
    let image = app.d_icon.clone();
    if let Some(pixbuf) = crate::packages::cached_pixbuf(&path, SIZE) {
        image.set_from_pixbuf(Some(&pixbuf));
        return;
    }
    let cache_key = path.clone();
    crate::bridge::spawn(
        async move {
            let bytes = tokio::task::spawn_blocking(move || std::fs::read(&path).ok())
                .await
                .unwrap_or(None);
            (cache_key, bytes)
        },
        move |(cache_key, bytes)| {
            if let Some(bytes) = bytes {
                if let Some(pixbuf) = crate::packages::decode_pixbuf(&bytes, SIZE) {
                    crate::packages::store_pixbuf(cache_key, SIZE, &pixbuf);
                    image.set_from_pixbuf(Some(&pixbuf));
                }
            }
        },
    );
}
