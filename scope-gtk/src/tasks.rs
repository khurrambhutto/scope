//! Background task model and the Tasks page.
//!
//! Operations never block the UI: confirming a preview closes the dialog and
//! hands the work to the backend on the tokio runtime. Tasks show up here with
//! live status and expandable logs on failure.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{glib, Align};

use crate::app::App;
use crate::backend::operations::Operation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u64,
    pub op: Operation,
    pub package_name: String,
    pub status: TaskStatus,
    pub message: String,
    pub logs: String,
}

impl Task {
    pub fn title(&self) -> String {
        format!("{} {}", crate::bridge::Backend::operation_label(self.op), self.package_name)
    }
}

/// Rebuild the task list from the current state and refresh counters.
pub fn rebuild(app: &Rc<App>) {
    let tasks = app.with_state(|s| s.tasks.clone());

    let list = &app.tasks_list;
    clear(list);

    for task in &tasks {
        list.append(&build_row(app, task));
    }

    let empty = tasks.is_empty();
    app.tasks_stack.set_visible_child_name(if empty { "empty" } else { "list" });

    crate::packages::update_counters(app);
}

fn build_row(app: &Rc<App>, task: &Task) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("task-row");

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    hbox.set_margin_top(10);
    hbox.set_margin_bottom(10);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);

    // Status icon column.
    let status: gtk::Widget = match task.status {
        TaskStatus::Running => {
            let spinner = gtk::Spinner::new();
            spinner.start();
            spinner.set_valign(Align::Start);
            spinner.upcast()
        }
        TaskStatus::Success => {
            let icon = gtk::Image::from_icon_name("object-select-symbolic");
            icon.add_css_class("success");
            icon.set_valign(Align::Start);
            icon.upcast()
        }
        TaskStatus::Failed => {
            let icon = gtk::Image::from_icon_name("x-circle-symbolic");
            icon.add_css_class("error");
            icon.set_valign(Align::Start);
            icon.upcast()
        }
    };
    hbox.append(&status);

    // Text column: title, message, logs (on failure).
    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_valign(Align::Start);

    let title = gtk::Label::new(Some(&task.title()));
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.add_css_class("heading");
    text.append(&title);

    let message = gtk::Label::new(Some(&task.message));
    message.set_xalign(0.0);
    message.set_wrap(true);
    message.add_css_class("dim-label");
    text.append(&message);

    if !task.logs.is_empty() {
        let expander = gtk::Expander::new(Some("Show log"));
        let log = gtk::Label::new(Some(task.logs.trim_end()));
        log.set_wrap(true);
        log.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        log.set_xalign(0.0);
        log.set_selectable(true);
        log.add_css_class("monospace");
        log.add_css_class("log-label");
        expander.set_child(Some(&log));
        text.append(&expander);
    }

    hbox.append(&text);

    // Dismiss button (only when settled).
    if task.status != TaskStatus::Running {
        let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
        dismiss.add_css_class("flat");
        dismiss.set_valign(Align::Start);
        dismiss.set_tooltip_text(Some("Dismiss"));
        let app2 = app.clone();
        let id = task.id;
        dismiss.connect_clicked(move |_| {
            app2.with_state(|s| s.tasks.retain(|t| t.id != id));
            rebuild(&app2);
        });
        hbox.append(&dismiss);
    }

    row.set_child(Some(&hbox));
    row
}

fn clear(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

/// Schedule automatic dismissal of a successful task after a short delay.
pub fn auto_dismiss(app: &Rc<App>, task_id: u64) {
    let app = app.clone();
    glib::timeout_add_seconds_local(6, move || {
        let still_success = app.with_state(|s| {
            if let Some(task) = s.tasks.iter_mut().find(|t| t.id == task_id) {
                if task.status == TaskStatus::Success {
                    s.tasks.retain(|t| t.id != task_id);
                    return true;
                }
            }
            false
        });
        if still_success {
            rebuild(&app);
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}
