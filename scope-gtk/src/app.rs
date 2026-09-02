//! Shared application state and the `App` handle passed through the UI.


use std::cell::RefCell;
use std::collections::HashMap;

use crate::backend::package::InstalledPackage;
use crate::backend::scanner::ScanAvailability;
use crate::bridge::{Backend, CachedScan};
use crate::tasks::{Task, TaskStatus};

/// Filter state and mutable UI data.
pub struct AppState {
    pub packages: Vec<InstalledPackage>,
    pub availability: ScanAvailability,
    pub scanned_at_ms: Option<u64>,
    pub scanning: bool,
    pub query: String,
    pub updates_only: bool,
    pub largest_first: bool,
    /// package key -> short busy label ("Removing…", "Updating…").
    pub busy: HashMap<String, String>,
    pub tasks: Vec<Task>,
    pub next_task_id: u64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            packages: Vec::new(),
            availability: ScanAvailability::default(),
            scanned_at_ms: None,
            scanning: false,
            query: String::new(),
            updates_only: false,
            largest_first: false,
            busy: HashMap::new(),
            tasks: Vec::new(),
            next_task_id: 1,
        }
    }
}

/// Central application handle. Widgets are owned here; behavior lives in the
/// feature modules (`packages`, `detail`, `ops`, `tasks`) as `impl`-style free
/// functions over `Rc<App>`.
pub struct App {
    pub backend: Backend,

    // Shell
    pub window: adw::ApplicationWindow,
    pub toast: adw::ToastOverlay,
    pub tasks_badge: gtk::Label,
    pub nav_list: gtk::ListBox,
    pub search_btn: gtk::Button,

    // Apps page
    pub window_title: adw::WindowTitle,
    pub search_bar: gtk::SearchBar,
    pub search_entry: gtk::SearchEntry,
    pub source_dd: gtk::DropDown,
    pub kind_dd: gtk::DropDown,
    pub updates_tb: gtk::ToggleButton,
    pub largest_tb: gtk::ToggleButton,
    pub refresh_btn: gtk::Button,
    pub refresh_icon: gtk::Image,
    pub refresh_spinner: gtk::Spinner,
    pub banner_revealer: gtk::Revealer,
    pub banner_label: gtk::Label,
    pub list_stack: gtk::Stack,
    pub list_box: gtk::ListBox,

    // Detail page
    pub nav_view: adw::NavigationView,
    pub detail_page: adw::NavigationPage,
    pub detail_key: RefCell<Option<String>>,
    pub d_icon: gtk::Image,
    pub d_title: gtk::Label,
    pub d_desc: gtk::Label,
    pub d_id: gtk::Label,
    pub d_source: gtk::Label,
    pub d_scope: gtk::Label,
    pub d_version: gtk::Label,
    pub d_size: gtk::Label,
    pub d_kind: gtk::Label,
    pub d_categories: gtk::Label,
    pub d_protection: gtk::Label,
    pub d_update_btn: gtk::Button,
    pub d_uninstall_btn: gtk::Button,

    // Tasks page
    pub tasks_stack: gtk::Stack,
    pub tasks_list: gtk::ListBox,

    pub state: RefCell<AppState>,
}

impl App {
    /// Mutate the shared state and return the closure's result.
    pub fn with_state<R>(&self, f: impl FnOnce(&mut AppState) -> R) -> R {
        let mut state = self.state.borrow_mut();
        f(&mut state)
    }

    /// Read-only clone of the current package list.
    #[allow(dead_code)]
    pub fn packages(&self) -> Vec<InstalledPackage> {
        self.with_state(|s| s.packages.clone())
    }

    pub fn toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(4);
        self.toast.add_toast(toast);
    }

    /// Mark/unmark a package as busy (state only; call `set_busy` to also
    /// rebuild dependent UI).
    pub fn mark_busy(&self, key: &str, label: Option<&str>) {
        self.with_state(|s| match label {
            Some(l) => {
                s.busy.insert(key.to_string(), l.to_string());
            }
            None => {
                s.busy.remove(key);
            }
        });
    }

    /// Number of packages with a known available update.
    #[allow(dead_code)]
    pub fn updates_count(&self) -> usize {
        self.with_state(|s| s.packages.iter().filter(|p| p.has_update).count())
    }

    /// Number of tasks currently running.
    #[allow(dead_code)]
    pub fn running_tasks(&self) -> usize {
        self.with_state(|s| {
            s.tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Running)
                .count()
        })
    }

    /// Find a package by backend key in the current snapshot.
    pub fn package(&self, key: &str) -> Option<InstalledPackage> {
        self.with_state(|s| s.packages.iter().find(|p| p.key == key).cloned())
    }
}

/// Replace the scan snapshot and refresh everything that depends on it.
pub fn apply_scan(app: &std::rc::Rc<App>, scan: CachedScan) {
    app.with_state(|s| {
        s.packages = scan.packages;
        s.availability = scan.availability;
        s.scanned_at_ms = Some(scan.scanned_at_ms);
        s.scanning = false;
    });
    crate::packages::rebuild_list(app);
    crate::packages::set_scan_ui(app, false);
    crate::detail::refresh_buttons(app);
}

/// Mark/unmark a package as busy and rebuild rows + detail buttons.
pub fn set_busy(app: &std::rc::Rc<App>, key: &str, label: Option<&str>) {
    app.mark_busy(key, label);
    crate::packages::rebuild_list(app);
    crate::detail::refresh_buttons(app);
}
