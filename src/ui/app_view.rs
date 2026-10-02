//! The Scope window: header, filters, virtualized-ish package list with inline
//! detail, footer, and the uninstall/update dialog flow.
//!
//! Layout, palette, and copy follow the original Tauri screen
//! (`src/features/packages/*` and `src/App.css`). Supporting pieces live in
//! sibling modules: `filters`, `row`, `detail`, `dialog`, `title_bar`,
//! `widgets`.

use std::time::Duration;

use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use gpui::prelude::*;
use gpui::{
    actions, div, linear_color_stop, linear_gradient, list, px, rgb, AnyElement, App, ClickEvent,
    Context, Entity, IntoElement, KeyBinding, ListAlignment, ListState, Render, Window,
    WeakEntity,
};

use crate::backend::{self, OpKind, OpMsg};
use crate::domain::operations::{OperationPlan, OperationStage, PlanStore};
use crate::domain::package::InstalledPackage;
use crate::theme::{self, text, text_faint};
use crate::ui::text_input::TextInput;

use super::detail::detail_element;
use super::dialog::Dialog;
use super::filters::{OpenSelect, SourceFilter, ViewMode};
use super::row::row_element;
use super::title_bar::title_bar;
use super::updater::{updater_banner, UpdaterStatus, UpdaterUi};
use super::widgets::{banner, empty_state, loading_state, BannerKind};

// ---- Keyboard actions ------------------------------------------------------

actions!(scope, [CloseDialog, Rescan]);

/// Scope-level keybindings, registered alongside the search input's at startup.
pub fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("escape", CloseDialog, None),
        KeyBinding::new("ctrl-r", Rescan, None),
        KeyBinding::new("cmd-r", Rescan, None),
    ]
}

/// Build an `on_click` handler that updates this view, replacing the
/// clone-entity / `update` / `.ok()` dance repeated at every call site.
pub(super) fn act(
    entity: &WeakEntity<ScopeApp>,
    f: impl Fn(&mut ScopeApp, &mut Context<ScopeApp>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let entity = entity.clone();
    move |_ev, _window, cx| {
        // `&f` implements `Fn` too, so the handler stays callable per click.
        entity.update(cx, &f).ok();
    }
}

// ---- App state -------------------------------------------------------------

/// Query + filters + scan identity: everything [`ScopeApp::sync_entries`]
/// needs to know the visible set is already up to date.
type FilterInputs = (String, SourceFilter, ViewMode, u64);

/// The Scope window: header, filters, a virtualized package list with inline
/// detail, and the uninstall/update dialog flow.
pub struct ScopeApp {
    pub(super) search_input: Entity<TextInput>,
    pub(super) scan: Option<backend::Scan>,
    pub(super) loading: bool,
    pub(super) refreshing: bool,
    pub(super) error: Option<String>,
    pub(super) source_filter: SourceFilter,
    pub(super) view_mode: ViewMode,
    pub(super) selected_key: Option<String>,
    pub(super) open_select: Option<OpenSelect>,
    pub(super) dialog: Option<Dialog>,
    pub(super) plans: PlanStore,
    pub(super) updater: UpdaterUi,
    pub(super) updater_busy: bool,
    /// Virtualized list state; scroll position lives here, not in the element.
    pub(super) list_state: ListState,
    /// The currently visible rows, owned so the list's render closure can read
    /// them without cloning the whole filtered set every frame.
    pub(super) entries: Vec<InstalledPackage>,
    pub(super) entry_keys: Vec<String>,
    /// Last inputs [`ScopeApp::sync_entries`] rebuilt from; unchanged inputs
    /// skip the rebuild (and its package clones) entirely.
    filter_inputs: Option<FilterInputs>,
    /// Lowercased search blob per package in `scan.packages`, rebuilt only when
    /// a new scan arrives so filtering never re-joins and re-lowercases every
    /// package on each keystroke.
    search_blobs: Vec<String>,
    /// `scan_gen` value [`Self::search_blobs`] was built from.
    blob_gen: u64,
    /// Bumped whenever a fresh scan replaces `scan`, so a rescan that arrives
    /// with identical filters still triggers a rebuild.
    scan_gen: u64,
}

impl ScopeApp {
    /// Create the app state and kick off the first scan and updater check.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(TextInput::new);
        cx.observe(&search_input, |_this, _input, cx| cx.notify())
            .detach();

        // Paint the previous session's list immediately, then always rescan.
        let scan = backend::read_cache();

        let mut app = Self {
            search_input,
            scan,
            loading: true,
            refreshing: false,
            error: None,
            source_filter: SourceFilter::All,
            view_mode: ViewMode::Uninstall,
            selected_key: None,
            open_select: None,
            dialog: None,
            plans: PlanStore::default(),
            updater: UpdaterUi::default(),
            updater_busy: false,
            // A non-zero overdraw is required: the list measures rows lazily,
            // and unmeasured rows count as zero height, so without look-ahead
            // the scrollable extent would equal only the visible rows.
            list_state: ListState::new(0, ListAlignment::Top, px(1000.)),
            entries: Vec::new(),
            entry_keys: Vec::new(),
            filter_inputs: None,
            search_blobs: Vec::new(),
            blob_gen: u64::MAX,
            scan_gen: 0,
        };
        app.start_scan(cx);
        app.check_updater(cx);
        app
    }

    /// Focus the search box once the window exists.
    pub fn focus_search(&mut self, window: &mut Window, cx: &mut App) {
        let handle = self.search_input.read(cx).focus_handle();
        window.focus(&handle);
    }

    pub(super) fn start_scan(&mut self, cx: &mut Context<Self>) {
        self.refreshing = true;
        self.error = None;
        let (tx, rx) = oneshot::channel::<backend::Scan>();
        std::thread::spawn(move || {
            let scan = backend::scan_blocking();
            // Cache write happens on this worker thread, not the UI thread.
            backend::write_cache(&scan);
            let _ = tx.send(scan);
        });
        cx.spawn(async move |this, cx| {
            let scan = rx.await;
            this.update(cx, |this, cx| {
                if let Ok(scan) = scan {
                    this.scan = Some(scan);
                    this.scan_gen += 1;
                }
                this.loading = false;
                this.refreshing = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn toggle_select(&mut self, key: &str, cx: &mut Context<Self>) {
        let previous = self.selected_key.clone();
        let next = if previous.as_deref() == Some(key) {
            None
        } else {
            Some(key.to_owned())
        };

        // Mark the rows whose height changed (the row that now carries an
        // inline detail panel, and the one that lost it) for re-measuring,
        // without disturbing the list's scroll offset.
        for changed in [previous.as_deref(), next.as_deref()].into_iter().flatten() {
            if let Some(index) = self.entries.iter().position(|pkg| pkg.key == changed) {
                self.list_state.splice(index..index + 1, 1);
            }
        }
        self.selected_key = next;
        cx.notify();
    }

    /// Keep [`Self::entries`] and the list's item count in sync with the active
    /// filters. Rebuilds only when the query, filters, or scan actually changed,
    /// so a plain re-render never re-clones the package list. Resets the virtual
    /// list only when the visible set actually changed, so a plain rescan keeps
    /// the user's scroll position.
    fn sync_entries(&mut self, cx: &App) {
        let query = self.search_input.read(cx).text().trim().to_lowercase();
        let unchanged = self.filter_inputs.as_ref().is_some_and(|(q, s, v, g)| {
            *q == query
                && *s == self.source_filter
                && *v == self.view_mode
                && *g == self.scan_gen
        });
        if unchanged {
            return;
        }
        self.filter_inputs = Some((
            query.clone(),
            self.source_filter,
            self.view_mode,
            self.scan_gen,
        ));

        // Rebuild the lowercased search blobs only when a new scan arrives, not
        // on every keystroke.
        if self.blob_gen != self.scan_gen {
            self.search_blobs = match &self.scan {
                Some(scan) => scan.packages.iter().map(theme::search_text).collect(),
                None => Vec::new(),
            };
            self.blob_gen = self.scan_gen;
        }

        let filtered = self.filtered(&query);
        let keys: Vec<String> = filtered.iter().map(|pkg| pkg.key.clone()).collect();
        if keys != self.entry_keys {
            self.entry_keys = keys;
            self.entries = filtered;
            self.list_state.reset(self.entries.len());
        }
    }

    pub(super) fn open_op(&mut self, kind: OpKind, pkg: InstalledPackage, cx: &mut Context<Self>) {
        let (tx, rx) = oneshot::channel::<Result<OperationPlan, String>>();
        backend::spawn_preview(self.plans.clone(), kind, pkg.clone(), tx);
        self.dialog = Some(Dialog::Loading {
            kind,
            pkg: pkg.clone(),
        });
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            let result = rx.await;
            entity
                .update(cx, |this, cx| {
                    // Never resurrect a dialog the user already dismissed.
                    if this.dialog.is_some() {
                        this.dialog = Some(match result {
                            Ok(Ok(plan)) => Dialog::Confirm { kind, pkg, plan },
                            Ok(Err(message)) => Dialog::Failed { kind, pkg, message },
                            Err(_) => Dialog::Failed {
                                kind,
                                pkg,
                                message: "Could not prepare the operation plan.".to_string(),
                            },
                        });
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    /// Open the operation dialog for a visible row by its stable key. The row
    /// handler captures only the key, so no package is cloned per frame.
    pub(super) fn open_op_by_key(&mut self, kind: OpKind, key: &str, cx: &mut Context<Self>) {
        let Some(pkg) = self.entries.iter().find(|p| p.key == key).cloned() else {
            return;
        };
        self.open_op(kind, pkg, cx);
    }

    pub(super) fn confirm_op(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Confirm { kind, pkg, plan }) = self.dialog.as_ref() else {
            return;
        };
        let kind = *kind;
        let pkg = pkg.clone();
        let plan = plan.clone();
        self.dialog = Some(Dialog::Running {
            kind,
            pkg: pkg.clone(),
            plan: plan.clone(),
            stage: OperationStage::Verifying,
            lines: Vec::new(),
            elapsed: 0,
        });

        let (tx, mut rx) = mpsc::unbounded::<OpMsg>();
        backend::spawn_apply(self.plans.clone(), plan, tx);

        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            while let Some(message) = rx.next().await {
                entity
                    .update(cx, |this, cx| match message {
                        OpMsg::Stage(stage) => {
                            if let Some(Dialog::Running { stage: current, .. }) = &mut this.dialog {
                                *current = stage;
                            }
                            cx.notify();
                        }
                        OpMsg::Log(line) => {
                            if let Some(Dialog::Running { lines, .. }) = &mut this.dialog {
                                lines.push(line);
                            }
                            cx.notify();
                        }
                        OpMsg::Done(result) => {
                            // Refresh even when the dialog was dismissed
                            // mid-run, so a successful operation is never lost.
                            let refresh = result.success;
                            if matches!(this.dialog, Some(Dialog::Running { .. })) {
                                this.dialog = Some(Dialog::Done {
                                    kind,
                                    pkg: pkg.clone(),
                                    result,
                                    show_logs: false,
                                });
                            }
                            cx.notify();
                            if refresh {
                                this.selected_key = None;
                                this.start_scan(cx);
                            }
                        }
                    })
                    .ok();
            }
        })
        .detach();

        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let running = entity
                .update(cx, |this, cx| {
                    if let Some(Dialog::Running { elapsed, .. }) = &mut this.dialog {
                        *elapsed += 1;
                        cx.notify();
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
            if !running {
                break;
            }
        })
        .detach();
    }

    pub(super) fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    pub(super) fn check_updater(&mut self, cx: &mut Context<Self>) {
        self.updater.status = UpdaterStatus::Checking;
        let (tx, rx) = oneshot::channel::<Option<crate::domain::updater::UpdateCheck>>();
        std::thread::spawn(move || {
            // Small delay so the list paints first, like the old shell.
            std::thread::sleep(Duration::from_millis(1000));
            backend::spawn_updater_check(tx);
        });
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            let result = rx.await;
            entity
                .update(cx, |this, cx| {
                    match result {
                        Ok(Some(check)) => this.updater.status = UpdaterStatus::Available(check),
                        _ => this.updater.status = UpdaterStatus::Dismissed,
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub(super) fn dismiss_updater(&mut self, cx: &mut Context<Self>) {
        self.updater.status = UpdaterStatus::Dismissed;
        cx.notify();
    }

    pub(super) fn start_update(&mut self, cx: &mut Context<Self>) {
        if self.updater_busy {
            return;
        }
        let Some(check) = self.updater.begin_install() else {
            return;
        };
        self.updater_busy = true;
        let (tx, mut rx) = mpsc::unbounded::<OpMsg>();
        backend::spawn_updater_install(check, tx);
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            while let Some(message) = rx.next().await {
                entity
                    .update(cx, |this, cx| match message {
                        OpMsg::Stage(_) => {
                            cx.notify();
                        }
                        OpMsg::Log(line) => {
                            this.updater.push_line(line);
                            cx.notify();
                        }
                        OpMsg::Done(result) => {
                            this.updater_busy = false;
                            this.updater.finish(result);
                            cx.notify();
                        }
                    })
                    .ok();
            }
        })
        .detach();
    }

    fn filtered(&self, query: &str) -> Vec<InstalledPackage> {
        let mut out = Vec::new();
        let Some(scan) = &self.scan else {
            return out;
        };
        for (index, pkg) in scan.packages.iter().enumerate() {
            if !self.source_filter.matches(pkg.source) {
                continue;
            }
            if self.view_mode == ViewMode::Updates && !pkg.has_update {
                continue;
            }
            if !query.is_empty()
                && !self
                    .search_blobs
                    .get(index)
                    .is_some_and(|blob| blob.contains(query))
            {
                continue;
            }
            out.push(pkg.clone());
        }
        out
    }

    /// Updater banner (if any) plus either the scan error or the per-source
    /// warnings, in the order they should stack.
    fn banner_stack(&self, entity: &WeakEntity<ScopeApp>) -> Vec<AnyElement> {
        let mut banners: Vec<AnyElement> = Vec::new();
        if self.updater.show_banner() {
            let busy = self.updater_busy;
            banners.push(updater_banner(
                &self.updater,
                !busy,
                act(entity, |this, cx| this.start_update(cx)),
                act(entity, |this, cx| this.dismiss_updater(cx)),
            ));
        }
        if let Some(error) = &self.error {
            banners.push(banner(error, BannerKind::Error).into_any_element());
        } else {
            for (label, message) in self.source_warnings() {
                banners.push(
                    banner(&format!("{label}: {message}"), BannerKind::Warn).into_any_element(),
                );
            }
        }
        banners
    }

    /// The virtualized package list, or a loading/empty state.
    fn list_element(&self, list_entity: &Entity<ScopeApp>) -> AnyElement {
        if self.loading && self.entries.is_empty() {
            loading_state().into_any_element()
        } else if self.entries.is_empty() {
            empty_state().into_any_element()
        } else {
            let state = self.list_state.clone();
            let list_entity = list_entity.clone();
            list(state, move |index, _window, cx| {
                let this = list_entity.read(cx);
                match this.entries.get(index) {
                    Some(pkg) => {
                        let selected = this.selected_key.as_deref() == Some(pkg.key.as_str());
                        let mut column = div().flex().flex_col().w_full();
                        column = column.child(row_element(
                            &list_entity.downgrade(),
                            pkg,
                            selected,
                            this.view_mode,
                            index,
                        ));
                        if selected {
                            column = column.child(detail_element(pkg));
                        }
                        column.into_any_element()
                    }
                    None => div().into_any_element(),
                }
            })
            .size_full()
            .into_any_element()
        }
    }

    fn source_warnings(&self) -> Vec<(String, String)> {
        let Some(scan) = &self.scan else {
            return Vec::new();
        };
        let availability = &scan.availability;
        [
            ("APT", availability.apt_error.as_ref()),
            ("Snap", availability.snap_error.as_ref()),
            ("Flatpak", availability.flatpak_error.as_ref()),
        ]
        .into_iter()
        .filter_map(|(label, message)| message.map(|m| (label.to_string(), m.clone())))
        .collect()
    }
}

// ---- Render ----------------------------------------------------------------

impl Render for ScopeApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_entries(cx);
        let entity = cx.entity().downgrade();
        let list_entity = cx.entity();
        let total = self.scan.as_ref().map(|s| s.packages.len()).unwrap_or(0);
        let rows_len = self.entries.len();

        let list_element = self.list_element(&list_entity);

        let banner_children = self.banner_stack(&entity);

        let footer = if self.loading {
            String::new()
        } else {
            theme::format_app_count(rows_len, total)
        };

        let dialog_el: Option<AnyElement> = if self.dialog.is_some() {
            Some(self.dialog_element(&entity))
        } else {
            None
        };

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(linear_gradient(
                180.,
                linear_color_stop(rgb(0x2d1414), 0.),
                linear_color_stop(rgb(0x0b0c0f), 1.),
            ))
            .text_color(text())
            // Keyboard shortcuts bubble up from the focused search box to the
            // root: Escape closes the operation dialog, Ctrl/Cmd+R rescans.
            .on_action(cx.listener(|this, _: &CloseDialog, _window, cx| {
                this.close_dialog(cx)
            }))
            .on_action(cx.listener(|this, _: &Rescan, _window, cx| {
                if !this.refreshing {
                    this.start_scan(cx);
                }
            }))
            .child(title_bar())
            .child(self.filters(cx))
            .children(banner_children)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .pl(px(32.))
                    .pr(px(16.))
                    .pb(px(16.))
                    .child(list_element),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(32.))
                    .py(px(10.))
                    .text_size(px(12.))
                    .text_color(text_faint())
                    .child(footer),
            )
            .when_some(dialog_el, |this, dialog| this.child(dialog))
    }
}
