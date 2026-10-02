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
use crate::operations::{OperationPlan, OperationStage, PlanStore};
use crate::package::InstalledPackage;
use crate::theme::{self, text, text_faint};
use crate::ui::text_input::TextInput;

use super::detail::detail_element;
use super::dialog::{OpDialog, Phase};
use super::filters::{KindFilter, OpenSelect, SourceFilter, ViewMode};
use super::row::row_element;
use super::title_bar::title_bar;
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

pub struct ScopeApp {
    pub(super) search_input: Entity<TextInput>,
    pub(super) scan: Option<backend::Scan>,
    pub(super) loading: bool,
    pub(super) refreshing: bool,
    pub(super) error: Option<String>,
    pub(super) source_filter: SourceFilter,
    pub(super) kind_filter: KindFilter,
    pub(super) view_mode: ViewMode,
    pub(super) selected_key: Option<String>,
    pub(super) open_select: Option<OpenSelect>,
    pub(super) dialog: Option<OpDialog>,
    pub(super) plans: PlanStore,
    /// Virtualized list state; scroll position lives here, not in the element.
    pub(super) list_state: ListState,
    /// The currently visible rows, owned so the list's render closure can read
    /// them without cloning the whole filtered set every frame.
    pub(super) entries: Vec<InstalledPackage>,
    pub(super) entry_keys: Vec<String>,
}

impl ScopeApp {
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
            kind_filter: KindFilter::All,
            view_mode: ViewMode::Uninstall,
            selected_key: None,
            open_select: None,
            dialog: None,
            plans: PlanStore::default(),
            // A non-zero overdraw is required: the list measures rows lazily,
            // and unmeasured rows count as zero height, so without look-ahead
            // the scrollable extent would equal only the visible rows.
            list_state: ListState::new(0, ListAlignment::Top, px(1000.)),
            entries: Vec::new(),
            entry_keys: Vec::new(),
        };
        app.start_scan(cx);
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
            let _ = tx.send(backend::scan_blocking());
        });
        cx.spawn(async move |this, cx| {
            let scan = rx.await;
            this.update(cx, |this, cx| {
                if let Ok(scan) = scan {
                    backend::write_cache(&scan);
                    this.scan = Some(scan);
                }
                this.loading = false;
                this.refreshing = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn toggle_select(&mut self, key: String, cx: &mut Context<Self>) {
        let previous = self.selected_key.clone();
        let next = if previous.as_deref() == Some(key.as_str()) {
            None
        } else {
            Some(key)
        };
        self.selected_key = next.clone();

        // Mark the rows whose height changed (the row that now carries an
        // inline detail panel, and the one that lost it) for re-measuring,
        // without disturbing the list's scroll offset.
        for changed in [previous.as_deref(), next.as_deref()].into_iter().flatten() {
            if let Some(index) = self.entries.iter().position(|pkg| pkg.key == changed) {
                self.list_state.splice(index..index + 1, 1);
            }
        }
        cx.notify();
    }

    /// Keep [`Self::entries`] and the list's item count in sync with the active
    /// filters. Resets the virtual list only when the visible set actually
    /// changed, so a plain rescan keeps the user's scroll position.
    fn sync_entries(&mut self, cx: &App) {
        let filtered = self.filtered(cx);
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
        self.dialog = Some(OpDialog {
            kind,
            pkg,
            phase: Phase::Loading,
            plan: None,
            error: None,
            stage: OperationStage::Verifying,
            lines: Vec::new(),
            elapsed: 0,
            show_logs: false,
            result: None,
        });
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            let result = rx.await;
            entity
                .update(cx, |this, cx| {
                    if let Some(dialog) = &mut this.dialog {
                        match result {
                            Ok(Ok(plan)) => {
                                dialog.plan = Some(plan);
                                dialog.phase = Phase::Confirm;
                            }
                            Ok(Err(error)) => {
                                dialog.error = Some(error);
                                dialog.phase = Phase::Error;
                            }
                            Err(_) => {
                                dialog.error =
                                    Some("Could not prepare the operation plan.".to_string());
                                dialog.phase = Phase::Error;
                            }
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub(super) fn confirm_op(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.dialog.as_ref().and_then(|d| d.plan.clone()) else {
            return;
        };
        if let Some(dialog) = &mut self.dialog {
            dialog.phase = Phase::Running;
            dialog.stage = OperationStage::Verifying;
            dialog.lines.clear();
            dialog.elapsed = 0;
            dialog.show_logs = false;
            dialog.result = None;
        }

        let (tx, mut rx) = mpsc::unbounded::<OpMsg>();
        backend::spawn_apply(self.plans.clone(), plan, tx);

        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            while let Some(message) = rx.next().await {
                let mut refresh = false;
                entity
                    .update(cx, |this, cx| {
                        if let Some(dialog) = &mut this.dialog {
                            match message {
                                OpMsg::Stage(stage) => dialog.stage = stage,
                                OpMsg::Log(line) => dialog.lines.push(line),
                                OpMsg::Done(result) => {
                                    refresh = result.success;
                                    dialog.result = Some(result);
                                    dialog.phase = Phase::Done;
                                }
                            }
                        }
                        cx.notify();
                        if refresh {
                            this.selected_key = None;
                            this.start_scan(cx);
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
                    if let Some(dialog) = &mut this.dialog {
                        if dialog.phase == Phase::Running {
                            dialog.elapsed += 1;
                            cx.notify();
                            true
                        } else {
                            false
                        }
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

    fn filtered(&self, cx: &App) -> Vec<InstalledPackage> {
        let query = self.search_input.read(cx).text().trim().to_lowercase();
        let mut out = Vec::new();
        let Some(scan) = &self.scan else {
            return out;
        };
        for pkg in &scan.packages {
            if !self.source_filter.matches(pkg.source) {
                continue;
            }
            if !self.kind_filter.matches(pkg.app_kind) {
                continue;
            }
            if self.view_mode == ViewMode::Updates && !pkg.has_update {
                continue;
            }
            if !query.is_empty() && !theme::search_text(pkg).contains(&query) {
                continue;
            }
            out.push(pkg.clone());
        }
        out
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

        let list_element: AnyElement = if self.loading && rows_len == 0 {
            loading_state().into_any_element()
        } else if rows_len == 0 {
            empty_state().into_any_element()
        } else {
            let state = self.list_state.clone();
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
        };

        let warnings = self.source_warnings();
        let mut banner_children: Vec<AnyElement> = Vec::new();
        if let Some(error) = self.error.clone() {
            banner_children.push(banner(&error, BannerKind::Error).into_any_element());
        } else {
            for (label, message) in warnings {
                banner_children.push(
                    banner(&format!("{label}: {message}"), BannerKind::Warn).into_any_element(),
                );
            }
        }

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
