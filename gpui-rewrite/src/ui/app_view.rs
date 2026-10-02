//! The Scope window: header, filters, virtualized-ish package list with inline
//! detail, footer, and the uninstall/update dialog flow.
//!
//! Layout, palette, and copy follow the original Tauri screen
//! (`src/features/packages/*` and `src/App.css`).

use std::path::PathBuf;
use std::time::Duration;

use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use gpui::prelude::*;
use gpui::{
    deferred, div, img, linear_color_stop, linear_gradient, list, px, rgb, rgba, AnyElement, App,
    ClickEvent, Context, Entity, FontWeight, Hsla, IntoElement, ListAlignment, ListState,
    MouseButton, Render, SharedString, Window, WindowControlArea, WeakEntity,
};

use crate::backend::{self, OpKind, OpMsg};
use crate::operations::{OperationPlan, OperationResult, OperationStage, PlanStore};
use crate::package::{AppKind, InstalledPackage, PackageSource};
use crate::theme::{self, display_title};
use crate::ui::text_input::TextInput;

const LOGO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../public/scope-logo.svg");
const ICON_FILTER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/filter.svg");
const ICON_REFRESH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/refresh.svg");

// ---- Palette ---------------------------------------------------------------

fn border() -> Hsla {
    rgb(0x2d2325).into()
}
fn text() -> Hsla {
    rgb(0xefe6e4).into()
}
fn text_dim() -> Hsla {
    rgb(0xa69692).into()
}
fn text_faint() -> Hsla {
    rgb(0x786c68).into()
}
fn accent() -> Hsla {
    rgb(0xd4504a).into()
}
fn danger() -> Hsla {
    rgb(0xc9443e).into()
}
fn update_green() -> Hsla {
    rgb(0x24795f).into()
}
fn elev() -> Hsla {
    rgb(0x171315).into()
}
fn elev2() -> Hsla {
    rgb(0x1e181a).into()
}

// ---- Filters ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Uninstall,
    Updates,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceFilter {
    All,
    Apt,
    Snap,
    Flatpak,
    AppImage,
}

impl SourceFilter {
    fn label(self) -> &'static str {
        match self {
            SourceFilter::All => "Any source",
            SourceFilter::Apt => "APT",
            SourceFilter::Snap => "Snap",
            SourceFilter::Flatpak => "Flatpak",
            SourceFilter::AppImage => "AppImage",
        }
    }

    fn matches(self, source: PackageSource) -> bool {
        match self {
            SourceFilter::All => true,
            SourceFilter::Apt => source == PackageSource::Apt,
            SourceFilter::Snap => source == PackageSource::Snap,
            SourceFilter::Flatpak => source == PackageSource::Flatpak,
            SourceFilter::AppImage => source == PackageSource::AppImage,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KindFilter {
    All,
    Gui,
    Cli,
    Unknown,
}

impl KindFilter {
    fn matches(self, kind: AppKind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Gui => kind == AppKind::Gui,
            KindFilter::Cli => kind == AppKind::Cli,
            KindFilter::Unknown => kind == AppKind::Unknown,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenSelect {
    Source,
    Kind,
}

// ---- Operation dialog ------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Confirm,
    Running,
    Done,
    Error,
}

struct OpDialog {
    kind: OpKind,
    pkg: InstalledPackage,
    phase: Phase,
    plan: Option<OperationPlan>,
    error: Option<String>,
    stage: OperationStage,
    lines: Vec<String>,
    elapsed: u64,
    show_logs: bool,
    result: Option<OperationResult>,
}

// ---- App state -------------------------------------------------------------

pub struct ScopeApp {
    search_input: Entity<TextInput>,
    scan: Option<backend::Scan>,
    loading: bool,
    refreshing: bool,
    error: Option<String>,
    source_filter: SourceFilter,
    kind_filter: KindFilter,
    view_mode: ViewMode,
    selected_key: Option<String>,
    open_select: Option<OpenSelect>,
    dialog: Option<OpDialog>,
    plans: PlanStore,
    /// Virtualized list state; scroll position lives here, not in the element.
    list_state: ListState,
    /// The currently visible rows, owned so the list's render closure can read
    /// them without cloning the whole filtered set every frame.
    entries: Vec<InstalledPackage>,
    entry_keys: Vec<String>,
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

    fn start_scan(&mut self, cx: &mut Context<Self>) {
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

    fn toggle_select(&mut self, key: String, cx: &mut Context<Self>) {
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

    fn open_op(&mut self, kind: OpKind, pkg: InstalledPackage, cx: &mut Context<Self>) {
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

    fn confirm_op(&mut self, cx: &mut Context<Self>) {
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

    fn close_dialog(&mut self, cx: &mut Context<Self>) {
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

impl ScopeApp {
    fn filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let open = self.open_select;

        let view_toggle = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded_full()
            .border_1()
            .border_color(border())
            .child(
                view_toggle_button(
                    "view-uninstall",
                    "Uninstall",
                    self.view_mode == ViewMode::Uninstall,
                    {
                        let entity = entity.clone();
                        move |_ev, _window, cx| {
                            entity
                                .update(cx, |this, cx| {
                                    this.view_mode = ViewMode::Uninstall;
                                    cx.notify();
                                })
                                .ok();
                        }
                    },
                ),
            )
            .child(view_toggle_button(
                "view-updates",
                "Updates",
                self.view_mode == ViewMode::Updates,
                {
                    let entity = entity.clone();
                    move |_ev, _window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                this.view_mode = ViewMode::Updates;
                                cx.notify();
                            })
                            .ok();
                    }
                },
            ));

        let source_select = select_widget(
            "source-select",
            Some(self.source_filter.label()),
            None,
            open == Some(OpenSelect::Source),
            {
                let entity = entity.clone();
                move |_ev, _window, cx| {
                    entity
                        .update(cx, |this, cx| {
                            this.open_select = match this.open_select {
                                Some(OpenSelect::Source) => None,
                                _ => Some(OpenSelect::Source),
                            };
                            cx.notify();
                        })
                        .ok();
                }
            },
            {
                let entity = entity.clone();
                let current = self.source_filter;
                move || -> Vec<AnyElement> {
                    let options = [
                        (SourceFilter::All, "Any source"),
                        (SourceFilter::Apt, "APT"),
                        (SourceFilter::Snap, "Snap"),
                        (SourceFilter::Flatpak, "Flatpak"),
                        (SourceFilter::AppImage, "AppImage"),
                    ];
                    options
                        .into_iter()
                        .enumerate()
                        .map(|(ix, (value, label))| {
                            let entity = entity.clone();
                            menu_item(
                                ("source-item", ix).into(),
                                label,
                                value == current,
                                move |_ev, _window, cx| {
                                    entity
                                        .update(cx, |this, cx| {
                                            this.source_filter = value;
                                            this.open_select = None;
                                            cx.notify();
                                        })
                                        .ok();
                                },
                            )
                            .into_any_element()
                        })
                        .collect()
                }
            },
        );

        let kind_select = select_widget(
            "kind-select",
            None,
            Some(ICON_FILTER),
            open == Some(OpenSelect::Kind),
            {
                let entity = entity.clone();
                move |_ev, _window, cx| {
                    entity
                        .update(cx, |this, cx| {
                            this.open_select = match this.open_select {
                                Some(OpenSelect::Kind) => None,
                                _ => Some(OpenSelect::Kind),
                            };
                            cx.notify();
                        })
                        .ok();
                }
            },
            {
                let entity = entity.clone();
                let current = self.kind_filter;
                move || -> Vec<AnyElement> {
                    let options = [
                        (KindFilter::All, "Any kind"),
                        (KindFilter::Gui, "GUI"),
                        (KindFilter::Cli, "CLI"),
                        (KindFilter::Unknown, "Other"),
                    ];
                    options
                        .into_iter()
                        .enumerate()
                        .map(|(ix, (value, label))| {
                            let entity = entity.clone();
                            menu_item(
                                ("kind-item", ix).into(),
                                label,
                                value == current,
                                move |_ev, _window, cx| {
                                    entity
                                        .update(cx, |this, cx| {
                                            this.kind_filter = value;
                                            this.open_select = None;
                                            cx.notify();
                                        })
                                        .ok();
                                },
                            )
                            .into_any_element()
                        })
                        .collect()
                }
            },
        );

        let rescan = div()
            .id("rescan")
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .p(px(8.))
            .rounded(px(12.))
            .border_1()
            .border_color(border())
            .cursor_pointer()
            .opacity(if self.refreshing { 0.5 } else { 1.0 })
            .hover(|this| this.bg(elev2()))
            .on_click({
                let entity = entity.clone();
                move |_ev, _window, cx| {
                    entity
                        .update(cx, |this, cx| {
                            if !this.refreshing {
                                this.start_scan(cx);
                            }
                        })
                        .ok();
                }
            })
            .child(img(PathBuf::from(ICON_REFRESH)).size(px(16.)).flex_none());

        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(32.))
            .pr(px(16.))
            .py(px(12.))
            .child(self.search_input.clone())
            .child(view_toggle)
            .child(div().flex_1())
            .child(source_select)
            .child(kind_select)
            .child(rescan)
    }

    fn dialog_element(&self, entity: &WeakEntity<ScopeApp>) -> AnyElement {
        let Some(dialog) = &self.dialog else {
            return div().into_any_element();
        };
        let title = display_title(&dialog.pkg);
        let heading = match dialog.kind {
            OpKind::Uninstall => format!("Uninstall {title}"),
            OpKind::Update => format!("Update {title}"),
        };
        let entity = entity.clone();

        let body: AnyElement = match dialog.phase {
            Phase::Loading => div()
                .p(px(20.))
                .text_color(text_dim())
                .text_size(px(14.))
                .child(match dialog.kind {
                    OpKind::Uninstall => "Preparing uninstall preview…",
                    OpKind::Update => "Preparing update preview…",
                })
                .into_any_element(),
            Phase::Error => div()
                .p(px(20.))
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(banner(
                    dialog
                        .error
                        .as_deref()
                        .unwrap_or("Could not prepare the operation plan."),
                    BannerKind::Error,
                ))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .child(button(
                            "dlg-error-close",
                            "Close",
                            ButtonStyle::Neutral,
                            {
                                let entity = entity.clone();
                                move |_ev, _window, cx| {
                                    entity.update(cx, |this, cx| this.close_dialog(cx)).ok();
                                }
                            },
                        )),
                )
                .into_any_element(),
            Phase::Confirm => confirm_body(&entity, dialog),
            Phase::Running => running_body(dialog),
            Phase::Done => done_body(&entity, dialog),
        };

        div()
            .id("modal-overlay")
            .absolute()
            .top_0()
            .right_0()
            .bottom_0()
            .left_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000099))
            .on_click({
                let entity = entity.clone();
                move |_ev, _window, cx| {
                    entity.update(cx, |this, cx| this.close_dialog(cx)).ok();
                }
            })
            .child(
                div()
                    .id("modal-card")
                    .on_click(|_ev: &ClickEvent, _window: &mut Window, cx: &mut App| {
                        cx.stop_propagation();
                    })
                    .w(px(560.))
                    .max_h(px(600.))
                    .overflow_y_scroll()
                    .rounded(px(16.))
                    .border_1()
                    .border_color(border())
                    .bg(elev())
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(20.))
                            .py(px(16.))
                            .border_b_1()
                            .border_color(border())
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(heading),
                            )
                            .child(
                                div()
                                    .id("modal-close")
                                    .cursor_pointer()
                                    .px(px(6.))
                                    .rounded(px(6.))
                                    .text_color(text_dim())
                                    .hover(|this| {
                                        this.text_color(text()).bg(elev2())
                                    })
                                    .on_click({
                                        let entity = entity.clone();
                                        move |_ev, _window, cx| {
                                            entity
                                                .update(cx, |this, cx| this.close_dialog(cx))
                                                .ok();
                                        }
                                    })
                                    .child("✕"),
                            ),
                    )
                    .child(body),
            )
            .into_any_element()
    }
}

// ---- Free render helpers ---------------------------------------------------

/// Client-side title bar. GNOME on Wayland ships no server-side decorations, so
/// the app draws its own controls; GPUI drives them through `Window` methods.
fn title_bar() -> impl IntoElement {
    div()
        .id("titlebar")
        .flex_none()
        .h(px(40.))
        .flex()
        .items_center()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |event, window, _cx| {
            if event.click_count == 2 {
                window.zoom_window();
            } else {
                window.start_window_move();
            }
        })
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(img(PathBuf::from(LOGO)).size(px(28.)).flex_none())
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child("Scope"),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(6.))
                .pr(px(8.))
                .child(window_control_button(
                    "win-min",
                    "–",
                    WindowControlArea::Min,
                    |window| window.minimize_window(),
                ))
                .child(window_control_button(
                    "win-max",
                    "▢",
                    WindowControlArea::Max,
                    |window| window.zoom_window(),
                ))
                .child(window_control_button(
                    "win-close",
                    "✕",
                    WindowControlArea::Close,
                    |window| window.remove_window(),
                )),
        )
}

fn window_control_button(
    id: &'static str,
    glyph: &'static str,
    area: WindowControlArea,
    action: impl Fn(&mut Window) + 'static,
) -> impl IntoElement {
    let hover_bg = danger();
    div()
        .id(id)
        .window_control_area(area)
        // Keep the press from bubbling to the title bar's window-move handler.
        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_event, window, _cx| action(window))
        .size(px(28.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(14.))
        .text_color(text_dim())
        .cursor_pointer()
        .hover(move |this| this.bg(hover_bg).text_color(rgb(0xffffff)))
        .child(glyph)
}

fn view_toggle_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(18.))
        .py(px(5.))
        .rounded_full()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(active, |this| {
            this.bg(linear_gradient(
                135.,
                linear_color_stop(rgb(0xd4504a), 0.),
                linear_color_stop(rgb(0xe0605a), 1.),
            ))
            .text_color(rgb(0xffffff))
        })
        .when(!active, |this| {
            this.text_color(text_dim()).hover(|this| this.text_color(text()))
        })
        .on_click(on_click)
        .child(label)
}

/// A pill select trigger plus its deferred dropdown menu. The trigger shows
/// either a text label or an icon.
fn select_widget(
    trigger_id: &'static str,
    value_label: Option<&'static str>,
    icon: Option<&'static str>,
    open: bool,
    on_trigger: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    menu: impl Fn() -> Vec<AnyElement> + 'static,
) -> impl IntoElement {
    let trigger = div()
        .id(trigger_id)
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .border_1()
        .border_color(if open { accent() } else { border() })
        .cursor_pointer()
        .hover(|this| this.bg(rgba(0xffffff08)).border_color(accent()))
        .on_click(on_trigger)
        .when_some(value_label, |this, label| {
            this.px(px(14.)).py(px(9.)).text_size(px(14.)).child(label)
        })
        .when_some(icon, |this, icon| {
            this.px(px(12.)).py(px(8.)).child(img(PathBuf::from(icon)).size(px(16.)).flex_none())
        })
        .child(div().text_color(text_dim()).child(if open { "▴" } else { "▾" }));

    div()
        .relative()
        .flex_none()
        .child(trigger)
        .when(open, |this| this.child(deferred(menu_popup(menu()))))
}

fn menu_popup(items: Vec<AnyElement>) -> AnyElement {
    div()
        .absolute()
        .top(px(38.))
        .left_0()
        .min_w(px(160.))
        .p(px(6.))
        .rounded(px(14.))
        .border_1()
        .border_color(border())
        .bg(elev())
        .shadow_lg()
        .flex()
        .flex_col()
        .children(items)
        .into_any_element()
}

fn menu_item(
    id: gpui::ElementId,
    label: &'static str,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(12.))
        .py(px(8.))
        .rounded(px(8.))
        .text_size(px(14.))
        .cursor_pointer()
        .when(selected, |this| this.text_color(accent()))
        .when(!selected, |this| this.text_color(text()))
        .hover(|this| this.bg(elev2()))
        .on_click(on_click)
        .child(label)
}

fn row_element(
    entity: &WeakEntity<ScopeApp>,
    pkg: &InstalledPackage,
    selected: bool,
    view_mode: ViewMode,
) -> AnyElement {
    let title = display_title(pkg);
    let key = pkg.key.clone();
    let version = if pkg.version.is_empty() {
        "—".to_string()
    } else {
        pkg.version.clone()
    };

    let action: Option<AnyElement> = if view_mode == ViewMode::Updates && pkg.has_update {
        Some(action_button(entity, pkg, OpKind::Update, "Update", update_green()))
    } else if view_mode == ViewMode::Uninstall {
        Some(action_button(entity, pkg, OpKind::Uninstall, "Uninstall", danger()))
    } else {
        None
    };

    let row = div()
        .id(SharedString::from(format!("row-{key}")))
        .group("row")
        .relative()
        .flex()
        .items_center()
        .gap(px(12.))
        .w_full()
        .px(px(12.))
        .py(px(10.))
        .my(px(2.))
        .rounded(px(12.))
        .border_1()
        .cursor_pointer()
        .border_color(if selected {
            rgba(0xd4504a26)
        } else {
            rgba(0x00000000)
        })
        .bg(if selected {
            rgba(0xd4504a14)
        } else {
            rgba(0x00000000)
        })
        .hover(|this| this.bg(rgba(0xffffff0a)))
        .on_click({
            let entity = entity.clone();
            let key = key.clone();
            move |_ev, _window, cx| {
                entity
                    .update(cx, |this, cx| this.toggle_select(key.clone(), cx))
                    .ok();
            }
        })
        .child(package_icon(pkg, 40.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .line_height(px(16.))
                        .text_color(text_dim())
                        .truncate()
                        .child(version)
                        .child("·")
                        .child(theme::format_size(pkg.size_bytes))
                        .child("·")
                        .child(theme::source_label(pkg.source)),
                ),
        )
        .when_some(action, |this, action| {
            this.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .flex()
                    .items_center()
                    .opacity(if selected { 1.0 } else { 0.0 })
                    .group_hover("row", |this| this.opacity(1.0))
                    .child(action),
            )
        });

    row.into_any_element()
}

fn action_button(
    entity: &WeakEntity<ScopeApp>,
    pkg: &InstalledPackage,
    kind: OpKind,
    label: &'static str,
    color: Hsla,
) -> AnyElement {
    let entity = entity.clone();
    let pkg = pkg.clone();
    let action_id = match kind {
        OpKind::Uninstall => format!("action-uninstall-{}", pkg.key),
        OpKind::Update => format!("action-update-{}", pkg.key),
    };
    div()
        .id(SharedString::from(action_id))
        .h_full()
        .px(px(18.))
        .flex()
        .items_center()
        .rounded_r(px(11.))
        .bg(color)
        .text_color(rgb(0xffffff))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|this| this.opacity(0.9))
        .on_click(move |_ev, _window, cx| {
            cx.stop_propagation();
            entity
                .update(cx, |this, cx| this.open_op(kind, pkg.clone(), cx))
                .ok();
        })
        .child(label)
        .into_any_element()
}

fn package_icon(pkg: &InstalledPackage, size: f32) -> AnyElement {
    if let Some(path) = backend::icon_path(pkg) {
        return img(path)
            .id(SharedString::from(format!("icon-{}", pkg.key)))
            .size(px(size))
            .rounded(px(10.))
            .into_any_element();
    }
    let title = display_title(pkg);
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::source_color(pkg.source))
        .text_color(rgb(0xffffff))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(if size > 48. { 18. } else { 14. }))
        .child(theme::initials(&title))
        .into_any_element()
}

fn detail_element(pkg: &InstalledPackage) -> AnyElement {
    let title = display_title(pkg);

    let mut rows: Vec<(String, String)> = vec![
        ("Package id".to_string(), pkg.package_id.clone()),
        (
            "Version".to_string(),
            if pkg.version.is_empty() {
                "—".to_string()
            } else {
                pkg.version.clone()
            },
        ),
        ("Installed size".to_string(), theme::format_size(pkg.size_bytes)),
        (
            "Categories".to_string(),
            pkg.categories.clone().unwrap_or_else(|| "—".to_string()),
        ),
        (
            "Update available".to_string(),
            if pkg.has_update {
                "Yes".to_string()
            } else {
                "—".to_string()
            },
        ),
    ];
    rows.retain(|(_, value)| !value.is_empty() && value != "—");

    div()
        .mx(px(8.))
        .mb(px(8.))
        .ml(px(12.))
        .p(px(16.))
        .rounded(px(12.))
        .border_1()
        .border_color(rgba(0xffffff0f))
        .bg(rgba(0xffffff08))
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(package_icon(pkg, 56.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_size(px(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(6.))
                                .child(tag(theme::source_label(pkg.source), theme::source_color(pkg.source)))
                                .child(tag(theme::kind_label(pkg.app_kind), theme::kind_color(pkg.app_kind))),
                        ),
                ),
        )
        .when_some(pkg.description.clone(), |this, description| {
            this.child(
                div()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(text_dim())
                    .child(description),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .children(
                    rows.into_iter()
                        .map(|(label, value)| kv_row(label, value)),
                ),
        )
        .into_any_element()
}

fn tag(label: &'static str, color: Hsla) -> impl IntoElement {
    div()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(6.))
        .border_1()
        .border_color(color)
        .text_color(color)
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

fn empty_state() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .py(px(48.))
        .text_color(text_faint())
        .text_size(px(14.))
        .child("No installed apps match your search.")
}

fn loading_state() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .py(px(48.))
        .text_color(text_faint())
        .text_size(px(14.))
        .child("Scanning installed apps across APT, Snap, Flatpak, and AppImage…")
}

enum BannerKind {
    Error,
    Warn,
    Ok,
    Muted,
}

fn banner(message: &str, kind: BannerKind) -> impl IntoElement {
    let (bg, border_c, fg) = match kind {
        BannerKind::Error => (rgba(0xd4504a1a), rgba(0xd4504a40), rgb(0xf0c4be)),
        BannerKind::Warn => (rgba(0xd9a4411a), rgba(0xd9a44140), rgb(0xecd9a8)),
        BannerKind::Ok => (rgba(0x0b8a4f1f), rgba(0x0b8a4f4d), rgb(0xbfe9c8)),
        BannerKind::Muted => (rgba(0xefe6e40a), rgb(0x2d2325), rgb(0xa69692)),
    };
    div()
        .mx(px(18.))
        .mt(px(10.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(12.))
        .border_1()
        .border_color(border_c)
        .bg(bg)
        .text_size(px(13.))
        .text_color(fg)
        .child(message.to_string())
}

#[derive(Clone, Copy)]
enum ButtonStyle {
    Neutral,
    Danger,
    Update,
}

fn button(
    id: &'static str,
    label: &'static str,
    style: ButtonStyle,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let (bg, fg) = match style {
        ButtonStyle::Neutral => (elev2(), text()),
        ButtonStyle::Danger => (danger(), rgb(0xffffff).into()),
        ButtonStyle::Update => (update_green(), rgb(0xffffff).into()),
    };
    let solid = !matches!(style, ButtonStyle::Neutral);
    div()
        .id(id)
        .px(px(16.))
        .py(px(8.))
        .rounded(px(12.))
        .border_1()
        .border_color(if solid { bg } else { border() })
        .bg(bg)
        .text_color(fg)
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|this| this.opacity(0.9))
        .on_click(on_click)
        .child(label)
}

/// One label/value line in the inline detail panel or a plan summary.
fn kv_row(label: impl Into<SharedString>, value: impl Into<SharedString>) -> impl IntoElement {
    let (label, value): (SharedString, SharedString) = (label.into(), value.into());
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap(px(12.))
        .py(px(8.))
        .border_b_1()
        .border_color(border())
        .child(
            div()
                .flex_none()
                .w(px(120.))
                .text_size(px(13.))
                .text_color(text_faint())
                .child(label),
        )
        .child(div().flex_1().text_size(px(14.)).child(value))
}

fn plan_rows(rows: Vec<(&'static str, String)>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .mb(px(14.))
        .children(rows.into_iter().map(|(label, value)| kv_row(label, value)))
}

fn dialog_actions(children: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .justify_end()
        .gap(px(10.))
        .mt(px(18.))
        .children(children)
}

fn confirm_body(entity: &WeakEntity<ScopeApp>, dialog: &OpDialog) -> AnyElement {
    let Some(plan) = &dialog.plan else {
        return div().into_any_element();
    };
    let pkg = &dialog.pkg;

    // Protected covers AppImages too: `safety::check_package` denies every
    // AppImage path, so their preview plans always arrive protected.
    if plan.protected {
        let reason = plan
            .protection_reason
            .clone()
            .unwrap_or_else(|| "This package is protected and cannot be changed.".to_string());
        return div()
            .p(px(20.))
            .flex()
            .flex_col()
            .child(banner(&reason, BannerKind::Warn))
            .child(dialog_actions(vec![button(
                "dlg-protected-cancel",
                "Close",
                ButtonStyle::Neutral,
                {
                    let entity = entity.clone();
                    move |_ev, _window, cx| {
                        entity.update(cx, |this, cx| this.close_dialog(cx)).ok();
                    }
                },
            )
            .into_any_element()]))
            .into_any_element();
    }

    let mut rows: Vec<(&'static str, String)> = vec![("Package", plan.package_id.clone())];
    if let Some(scope) = plan.install_scope {
        rows.push(("Scope", scope.id().to_string()));
    }
    match dialog.kind {
        OpKind::Uninstall => {
            rows.push((
                "Version",
                if plan.current_version.is_empty() {
                    "—".to_string()
                } else {
                    plan.current_version.clone()
                },
            ));
        }
        OpKind::Update => {
            rows.push((
                "Current version",
                if plan.current_version.is_empty() {
                    "—".to_string()
                } else {
                    plan.current_version.clone()
                },
            ));
            rows.push((
                "Target version",
                if plan.target_version.is_empty() {
                    "latest".to_string()
                } else {
                    plan.target_version.clone()
                },
            ));
        }
    }
    rows.push(("Size", theme::format_size(pkg.size_bytes)));

    let confirm_style = match dialog.kind {
        OpKind::Uninstall => ButtonStyle::Danger,
        OpKind::Update => ButtonStyle::Update,
    };

    div()
        .p(px(20.))
        .child(plan_rows(rows))
        .child(dialog_actions(vec![
            button("dlg-cancel", "Cancel", ButtonStyle::Neutral, {
                let entity = entity.clone();
                move |_ev, _window, cx| {
                    entity.update(cx, |this, cx| this.close_dialog(cx)).ok();
                }
            })
            .into_any_element(),
            button("dlg-confirm", "Confirm", confirm_style, {
                let entity = entity.clone();
                move |_ev, _window, cx| {
                    entity
                        .update(cx, |this, cx| this.confirm_op(cx))
                        .ok();
                }
            })
            .into_any_element(),
        ]))
        .into_any_element()
}

fn running_body(dialog: &OpDialog) -> AnyElement {
    let title = display_title(&dialog.pkg);
    let requires_auth = dialog.plan.as_ref().map(|p| p.requires_auth).unwrap_or(false);
    let verb = match dialog.kind {
        OpKind::Uninstall => "removed",
        OpKind::Update => "updated",
    };
    let text = if dialog.stage == OperationStage::Verifying {
        format!("Checking that {title} can be {verb}…")
    } else {
        format!(
            "{} {title}…",
            match dialog.kind {
                OpKind::Uninstall => "Removing",
                OpKind::Update => "Updating",
            }
        )
    };
    let hint = if requires_auth {
        "A system password prompt may appear."
    } else {
        ""
    };
    let logs = if dialog.lines.is_empty() {
        "Waiting for output…".to_string()
    } else {
        dialog.lines.join("\n")
    };

    div()
        .p(px(20.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(spinner())
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(px(14.))
                        .text_color(text_dim())
                        .child(format!("{text} {hint}")),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(text_faint())
                        .child(theme::format_elapsed(dialog.elapsed)),
                ),
        )
        .child(
            div()
                .mt(px(14.))
                .p(px(12.))
                .max_h(px(200.))
                .overflow_hidden()
                .rounded(px(10.))
                .border_1()
                .border_color(border())
                .bg(rgb(0x0b0c0f))
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(text_dim())
                .child(logs),
        )
        .into_any_element()
}

/// A simple pulsing dot standing in for the CSS spinner (GPUI has no
/// transform animation on `Div`, so we animate opacity instead).
fn spinner() -> impl IntoElement {
    use gpui::{Animation, AnimationExt};
    div()
        .flex_none()
        .size(px(10.))
        .rounded_full()
        .bg(accent())
        .with_animation(
            "spinner-pulse",
            Animation::new(Duration::from_millis(800)).repeat(),
            |this, delta| this.opacity(0.3 + delta * 0.7),
        )
}

fn done_body(entity: &WeakEntity<ScopeApp>, dialog: &OpDialog) -> AnyElement {
    let Some(result) = &dialog.result else {
        return div().into_any_element();
    };
    if is_cancelled(result) {
        return div()
            .p(px(20.))
            .child(banner(
                "Authentication was cancelled. Nothing was changed.",
                BannerKind::Muted,
            ))
            .child(dialog_actions(vec![button(
                "dlg-cancelled-close",
                "Close",
                ButtonStyle::Neutral,
                {
                    let entity = entity.clone();
                    move |_ev, _window, cx| {
                        entity.update(cx, |this, cx| this.close_dialog(cx)).ok();
                    }
                },
            )
            .into_any_element()]))
            .into_any_element();
    }

    let kind = if result.success {
        BannerKind::Ok
    } else {
        BannerKind::Error
    };
    let label = if result.success { "Done" } else { "Close" };

    div()
        .p(px(20.))
        .child(banner(&result.message, kind))
        .child(
            div()
                .id("dlg-logtoggle")
                .mt(px(10.))
                .text_size(px(13.))
                .text_color(accent())
                .cursor_pointer()
                .on_click({
                    let entity = entity.clone();
                    move |_ev, _window, cx| {
                        entity
                            .update(cx, |this, cx| {
                                if let Some(dialog) = &mut this.dialog {
                                    dialog.show_logs = !dialog.show_logs;
                                }
                                cx.notify();
                            })
                            .ok();
                    }
                })
                .child(format!(
                    "{} command output",
                    if dialog.show_logs { "Hide" } else { "Show" }
                )),
        )
        .when(dialog.show_logs, |this| {
            this.child(
                div()
                    .id("dlg-logs")
                    .mt(px(8.))
                    .p(px(12.))
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(border())
                    .bg(rgb(0x0b0c0f))
                    .text_size(px(12.))
                    .text_color(text_dim())
                    .child(result.logs.clone()),
            )
        })
        .child(dialog_actions(vec![button(
            "dlg-done",
            label,
            ButtonStyle::Neutral,
            {
                let entity = entity.clone();
                move |_ev, _window, cx| {
                    entity.update(cx, |this, cx| this.close_dialog(cx)).ok();
                }
            },
        )
        .into_any_element()]))
        .into_any_element()
}

/// A dismissed Polkit prompt comes back as exit 126 rather than an error.
fn is_cancelled(result: &OperationResult) -> bool {
    if result.success || result.exit_code != Some(126) {
        return false;
    }
    let haystack = format!("{}\n{}", result.message, result.logs).to_lowercase();
    haystack.contains("dismiss") || haystack.contains("cancel")
}
