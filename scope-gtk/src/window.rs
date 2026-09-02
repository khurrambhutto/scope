//! Main window: shell, sidebar navigation, apps page, tasks page, wiring.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, Align, Orientation};

use crate::app::App;
use crate::detail;
use crate::packages;

const STYLE: &str = include_str!("style.css");

pub fn build(gtk_app: &adw::Application) {
    load_css();

    let window = adw::ApplicationWindow::builder()
        .application(gtk_app)
        .title("Scope")
        .default_width(1020)
        .default_height(700)
        .build();

    // ---- Apps page ---------------------------------------------------------
    let window_title = adw::WindowTitle::new("Apps", "");

    let refresh_icon = gtk::Image::from_icon_name("view-refresh-symbolic");
    let refresh_spinner = gtk::Spinner::new();
    refresh_spinner.set_visible(false);
    let refresh_btn = gtk::Button::new();
    refresh_btn.set_child(Some(&refresh_icon));
    refresh_btn.set_tooltip_text(Some("Rescan packages"));
    refresh_btn.add_css_class("flat");

    let search_btn = gtk::Button::from_icon_name("system-search-symbolic");
    search_btn.add_css_class("flat");
    search_btn.set_tooltip_text(Some("Search (Ctrl+F)"));

    let header_bar = adw::HeaderBar::new();
    header_bar.set_title_widget(Some(&window_title));
    header_bar.pack_end(&refresh_btn);
    header_bar.pack_end(&search_btn);

    // Search bar (revealed by the header button or Ctrl+F).
    let search_entry = gtk::SearchEntry::new();
    search_entry.set_hexpand(true);
    let search_bar = gtk::SearchBar::new();
    search_bar.set_child(Some(&search_entry));
    search_bar.set_search_mode(false);
    search_bar.set_key_capture_widget(Some(&window));

    // Banner for per-source scan errors.
    let banner_label = gtk::Label::new(None);
    banner_label.set_wrap(true);
    banner_label.set_xalign(0.0);
    banner_label.set_margin_top(8);
    banner_label.set_margin_bottom(8);
    banner_label.set_margin_start(12);
    banner_label.set_margin_end(12);
    banner_label.add_css_class("warning");
    let banner_revealer = gtk::Revealer::new();
    banner_revealer.set_child(Some(&banner_label));
    banner_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);

    // Filter bar (bottom toolbar, GNOME Software style).
    let source_model = gtk::StringList::new(&[
        "All sources",
        "APT",
        "Snap",
        "Flatpak",
        "AppImage",
        "Manual",
    ]);
    let source_dd = gtk::DropDown::new(
        Some(source_model),
        Some(gtk::StringObject::this_expression("string").upcast_ref()),
    );
    source_dd.set_tooltip_text(Some("Package source"));

    let kind_model =
        gtk::StringList::new(&["All kinds", "GUI apps", "CLI tools", "Unclassified"]);
    let kind_dd = gtk::DropDown::new(
        Some(kind_model),
        Some(gtk::StringObject::this_expression("string").upcast_ref()),
    );
    kind_dd.set_tooltip_text(Some("App kind"));

    let updates_tb = gtk::ToggleButton::with_label("Updates (0)");
    updates_tb.set_tooltip_text(Some("Show only packages with updates"));
    let largest_tb = gtk::ToggleButton::with_label("Largest first");
    largest_tb.set_tooltip_text(Some("Sort by installed size"));

    let filter_bar = gtk::Box::new(Orientation::Horizontal, 8);
    filter_bar.set_margin_top(6);
    filter_bar.set_margin_bottom(6);
    filter_bar.set_margin_start(12);
    filter_bar.set_margin_end(12);
    filter_bar.append(&source_dd);
    filter_bar.append(&kind_dd);
    let spacer = gtk::Box::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    filter_bar.append(&spacer);
    filter_bar.append(&largest_tb);
    filter_bar.append(&updates_tb);

    // Package list with loading/empty states.
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::None);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_top(12);
    list_box.set_margin_bottom(12);
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);

    let list_clamp = adw::Clamp::new();
    list_clamp.set_maximum_size(900);
    list_clamp.set_child(Some(&list_box));

    let list_scroll = gtk::ScrolledWindow::new();
    list_scroll.set_child(Some(&list_clamp));
    list_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    list_scroll.set_vexpand(true);

    let loading_box = gtk::Box::new(Orientation::Vertical, 12);
    loading_box.set_valign(Align::Center);
    loading_box.set_halign(Align::Center);
    let loading_spinner = gtk::Spinner::new();
    loading_spinner.start();
    loading_spinner.set_size_request(32, 32);
    loading_box.append(&loading_spinner);
    loading_box.append(&gtk::Label::new(Some("Scanning installed apps…")));

    let empty_label = gtk::Label::new(Some("No packages found."));
    empty_label.add_css_class("dim-label");
    empty_label.set_valign(Align::Center);

    let list_stack = gtk::Stack::new();
    list_stack.add_named(&loading_box, Some("loading"));
    list_stack.add_named(&list_scroll, Some("list"));
    list_stack.add_named(&empty_label, Some("empty"));
    list_stack.set_vhomogeneous(false);

    let apps_box = gtk::Box::new(Orientation::Vertical, 0);
    apps_box.append(&search_bar);
    apps_box.append(&banner_revealer);
    apps_box.append(&list_stack);

    let apps_toolbar = adw::ToolbarView::new();
    apps_toolbar.add_top_bar(&header_bar);
    apps_toolbar.add_bottom_bar(&filter_bar);
    apps_toolbar.set_content(Some(&apps_box));
    let apps_page = adw::NavigationPage::new(&apps_toolbar, "Apps");

    // ---- Detail page -------------------------------------------------------
    let detail_widgets = detail::build_widgets();

    // ---- Tasks page --------------------------------------------------------
    let tasks_list = gtk::ListBox::new();
    tasks_list.set_selection_mode(gtk::SelectionMode::None);
    tasks_list.add_css_class("boxed-list");
    tasks_list.set_margin_top(12);
    tasks_list.set_margin_bottom(12);
    tasks_list.set_margin_start(12);
    tasks_list.set_margin_end(12);
    tasks_list.set_valign(Align::Start);

    let tasks_empty = gtk::Label::new(Some(
        "No operations yet.\nUninstall or update a package to see it here.",
    ));
    tasks_empty.set_justify(gtk::Justification::Center);
    tasks_empty.add_css_class("dim-label");
    tasks_empty.set_valign(Align::Center);

    let tasks_stack = gtk::Stack::new();
    tasks_stack.add_named(&tasks_empty, Some("empty"));
    tasks_stack.add_named(&tasks_list, Some("list"));
    tasks_stack.set_vhomogeneous(false);

    let tasks_scroll = gtk::ScrolledWindow::new();
    let tasks_clamp = adw::Clamp::new();
    tasks_clamp.set_maximum_size(900);
    tasks_clamp.set_child(Some(&tasks_stack));
    tasks_scroll.set_child(Some(&tasks_clamp));
    tasks_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    let tasks_title = adw::WindowTitle::new("Tasks", "Uninstall and update operations");
    let tasks_header = adw::HeaderBar::new();
    tasks_header.set_title_widget(Some(&tasks_title));
    let tasks_toolbar = adw::ToolbarView::new();
    tasks_toolbar.add_top_bar(&tasks_header);
    tasks_toolbar.set_content(Some(&tasks_scroll));
    let tasks_page = adw::NavigationPage::new(&tasks_toolbar, "Tasks");

    // ---- Content navigation ------------------------------------------------
    let nav_view = adw::NavigationView::new();
    nav_view.add(&apps_page);
    nav_view.add(&detail_widgets.page);
    nav_view.add(&tasks_page);

    // ---- Sidebar -----------------------------------------------------------
    let nav_list = gtk::ListBox::new();
    nav_list.set_selection_mode(gtk::SelectionMode::Single);
    nav_list.add_css_class("navigation-sidebar");
    nav_list.set_margin_top(6);
    nav_list.set_margin_bottom(6);
    nav_list.set_margin_start(6);
    nav_list.set_margin_end(6);

    let tasks_badge = gtk::Label::new(Some("0"));
    tasks_badge.add_css_class("chip");
    tasks_badge.add_css_class("accent");
    tasks_badge.set_visible(false);
    tasks_badge.set_valign(Align::Center);

    add_nav_row(&nav_list, "view-grid-symbolic", "Apps", None);
    add_nav_row(&nav_list, "task-due-symbolic", "Tasks", Some(tasks_badge.upcast_ref()));
    add_nav_row_disabled(&nav_list, "brush-symbolic", "Clean");
    add_nav_row_disabled(&nav_list, "drive-harddisk-symbolic", "Analyze");
    add_nav_row_disabled(&nav_list, "heart-symbolic", "Status");

    let sidebar_header = adw::WindowTitle::new("Scope", "Package manager");
    let sidebar_box = gtk::Box::new(Orientation::Vertical, 0);
    let sidebar_header_bar = adw::HeaderBar::new();
    sidebar_header_bar.set_title_widget(Some(&sidebar_header));
    sidebar_box.append(&sidebar_header_bar);
    sidebar_box.append(&nav_list);

    let sidebar_page = adw::NavigationPage::new(&sidebar_box, "Scope");

    // ---- Toasts + split shell ---------------------------------------------
    let toast = adw::ToastOverlay::new();
    let split = adw::OverlaySplitView::new();
    split.set_sidebar(Some(&sidebar_page));
    split.set_content(Some(&nav_view));
    split.set_min_sidebar_width(200.0);
    split.set_max_sidebar_width(280.0);
    split.set_sidebar_width_fraction(0.22);
    toast.set_child(Some(&split));

    window.set_content(Some(&toast));

    // ---- App handle --------------------------------------------------------
    let app = Rc::new(App {
        backend: Default::default(),
        window,
        toast,
        tasks_badge,
        nav_list,
        search_btn,
        window_title,
        search_bar,
        search_entry,
        source_dd,
        kind_dd,
        updates_tb,
        largest_tb,
        refresh_btn,
        refresh_icon,
        refresh_spinner,
        banner_revealer,
        banner_label,
        list_stack,
        list_box,
        nav_view,
        detail_page: detail_widgets.page,
        detail_key: Default::default(),
        d_icon: detail_widgets.icon,
        d_title: detail_widgets.title_label,
        d_desc: detail_widgets.desc,
        d_id: detail_widgets.id,
        d_source: detail_widgets.source,
        d_scope: detail_widgets.scope,
        d_version: detail_widgets.version,
        d_size: detail_widgets.size,
        d_kind: detail_widgets.kind,
        d_categories: detail_widgets.categories,
        d_protection: detail_widgets.protection,
        d_update_btn: detail_widgets.update_btn,
        d_uninstall_btn: detail_widgets.uninstall_btn,
        tasks_stack,
        tasks_list,
        state: Default::default(),
    });

    // ---- Wiring ------------------------------------------------------------
    wire(&app, gtk_app, &apps_page, &tasks_page);

    app.window.present();

    // Initial scan.
    packages::start_scan(&app);
}

fn wire(
    app: &Rc<App>,
    gtk_app: &adw::Application,
    apps_page: &adw::NavigationPage,
    tasks_page: &adw::NavigationPage,
) {
    let window = app.window.clone();

    // Sidebar navigation: Apps / Tasks; disabled rows never activate.
    let nav_list = &app.nav_list;
    nav_list.select_row(nav_list.row_at_index(0).as_ref());
    {
        let nav_view = app.nav_view.clone();
        let apps_page = apps_page.clone();
        let tasks_page = tasks_page.clone();
        nav_list.connect_row_activated(move |_, row| {
            match row.widget_name().as_str() {
                "Apps" => {
                    nav_view.pop_to_page(&apps_page);
                }
                "Tasks" => {
                    nav_view.pop_to_page(&tasks_page);
                }
                _ => {}
            }
        });
    }

    // Search: header button toggles the bar; typing filters the list.
    {
        let search_bar = app.search_bar.clone();
        let search_btn = app.search_btn.clone();
        search_btn.connect_clicked(move |_| {
            search_bar.set_search_mode(!search_bar.is_search_mode());
        });
    }
    {
        let app2 = app.clone();
        app.search_entry.connect_search_changed(move |entry| {
            let text = entry.text().to_string();
            // Debounce: every keystroke records the query but only the latest
            // generation rebuilds, 150 ms after typing settles. Without this,
            // each keystroke cleared and rebuilt the whole ListBox (rows +
            // icon disk reads) while the user was still typing.
            let generation = app2.with_state(|s| {
                s.query = text;
                s.search_generation += 1;
                s.search_generation
            });
            let app3 = app2.clone();
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(150),
                move || {
                    let current = app3.with_state(|s| s.search_generation);
                    if current == generation {
                        packages::rebuild_list(&app3);
                    }
                },
            );
        });
    }

    // Filters.
    {
        let app2 = app.clone();
        app.source_dd
            .connect_notify_local(Some("selected"), move |_, _| {
                packages::rebuild_list(&app2);
            });
    }
    {
        let app2 = app.clone();
        app.kind_dd
            .connect_notify_local(Some("selected"), move |_, _| {
                packages::rebuild_list(&app2);
            });
    }
    {
        let app2 = app.clone();
        app.updates_tb.connect_toggled(move |tb| {
            let active = tb.is_active();
            app2.with_state(|s| s.updates_only = active);
            packages::rebuild_list(&app2);
        });
    }
    {
        let app2 = app.clone();
        app.largest_tb.connect_toggled(move |tb| {
            let active = tb.is_active();
            app2.with_state(|s| s.largest_first = active);
            packages::rebuild_list(&app2);
        });
    }

    // Rescan.
    {
        let app2 = app.clone();
        app.refresh_btn.connect_clicked(move |_| {
            packages::start_scan(&app2);
        });
    }

    // Ctrl+F toggles search.
    let search_action = gio::SimpleAction::new("toggle-search", None);
    {
        let search_bar = app.search_bar.clone();
        search_action.connect_activate(move |_, _| {
            search_bar.set_search_mode(!search_bar.is_search_mode());
        });
    }
    window.add_action(&search_action);
    gtk_app.set_accels_for_action("win.toggle-search", &["<Ctrl>f"]);

    // Detail actions.
    {
        let app2 = app.clone();
        app.d_update_btn.connect_clicked(move |_| {
            if let Some(key) = app2.detail_key.borrow().clone() {
                crate::ops::request(&app2, crate::backend::operations::Operation::Update, key);
            }
        });
    }
    {
        let app2 = app.clone();
        app.d_uninstall_btn.connect_clicked(move |_| {
            if let Some(key) = app2.detail_key.borrow().clone() {
                crate::ops::request(&app2, crate::backend::operations::Operation::Uninstall, key);
            }
        });
    }

    // About dialog.
    let about = gio::SimpleAction::new("about", None);
    {
        about.connect_activate(move |_, _| {
            let dialog = gtk::AboutDialog::new();
            dialog.set_program_name(Some("Scope"));
            dialog.set_version(Some(env!("CARGO_PKG_VERSION")));
            dialog.set_comments(Some(
                "See, update, and uninstall every app on your Linux system.",
            ));
            dialog.set_license_type(gtk::License::MitX11);
            dialog.present();
        });
    }
    gtk_app.add_action(&about);
}

fn add_nav_row(
    list: &gtk::ListBox,
    icon_name: &str,
    label: &str,
    suffix: Option<&gtk::Widget>,
) {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(label);

    let hbox = gtk::Box::new(Orientation::Horizontal, 10);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);
    hbox.set_margin_start(10);
    hbox.set_margin_end(10);

    let icon = gtk::Image::from_icon_name(icon_name);
    hbox.append(&icon);

    let text = gtk::Label::new(Some(label));
    text.set_hexpand(true);
    text.set_xalign(0.0);
    hbox.append(&text);

    if let Some(suffix) = suffix {
        hbox.append(suffix);
    }

    row.set_child(Some(&hbox));
    list.append(&row);
}

fn add_nav_row_disabled(list: &gtk::ListBox, icon_name: &str, label: &str) {
    let row = gtk::ListBoxRow::new();
    row.set_sensitive(false);

    let hbox = gtk::Box::new(Orientation::Horizontal, 10);
    hbox.set_margin_top(8);
    hbox.set_margin_bottom(8);
    hbox.set_margin_start(10);
    hbox.set_margin_end(10);

    let icon = gtk::Image::from_icon_name(icon_name);
    hbox.append(&icon);

    let text = gtk::Label::new(Some(label));
    text.set_hexpand(true);
    text.set_xalign(0.0);
    hbox.append(&text);

    let soon = gtk::Label::new(Some("Soon"));
    soon.add_css_class("chip");
    soon.add_css_class("dim-label");
    hbox.append(&soon);

    row.set_child(Some(&hbox));
    list.append(&row);
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(STYLE);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
