//! Uninstall/update flow: preview → confirm dialog → background task.
//!
//! Mirrors the Tauri frontend's `ConfirmOperationDialog` + task store flow:
//! the user always sees the exact plan before confirming, and applying never
//! blocks the UI — a task appears in the task center instead.

use adw::prelude::*;

use std::rc::Rc;

use gtk::glib;

use crate::app::App;
use crate::backend::operations::{
    AuthMethod, Operation, OperationPlan, OperationResult,
};
use crate::backend::package::InstallScope;
use crate::bridge;
use crate::tasks::{Task, TaskStatus};

/// Entry point from any row/button: fetch a preview plan, then confirm.
pub fn request(app: &Rc<App>, op: Operation, key: String) {
    // One operation per package at a time.
    if app.with_state(|s| s.busy.contains_key(&key)) {
        return;
    }

    crate::app::set_busy(app, &key, Some("Preparing…"));

    let backend = app.backend.clone();
    let app2 = app.clone();
    let key2 = key.clone();
    bridge::spawn(
        async move {
            match op {
                Operation::Uninstall => backend.preview_uninstall(&key).await,
                Operation::Update => backend.preview_update(&key).await,
            }
        },
        move |result| on_preview(&app2, op, key2, result),
    );
}

fn on_preview(
    app: &Rc<App>,
    op: Operation,
    key: String,
    result: Result<OperationPlan, String>,
) {
    crate::app::set_busy(app, &key, None);

    let plan = match result {
        Ok(plan) => plan,
        Err(message) => {
            app.toast(&message);
            return;
        }
    };

    if plan.protected {
        show_protected(app, &plan);
        return;
    }

    show_confirm(app, op, key, plan);
}

fn show_protected(app: &Rc<App>, plan: &OperationPlan) {
    let reason = plan
        .protection_reason
        .clone()
        .unwrap_or_else(|| "This package is essential to your system.".to_string());
    let dialog = adw::AlertDialog::new(
        Some(&format!("{} is protected", plan.display_name)),
        Some(&format!("{reason}\n\nRemoval is blocked to keep your system safe.")),
    );
    dialog.add_response("ok", "OK");
    dialog.set_default_response(Some("ok"));
    dialog.set_close_response("ok");
    dialog.present(Some(&app.window));
}

fn show_confirm(app: &Rc<App>, op: Operation, key: String, plan: OperationPlan) {
    let apply_id = match op {
        Operation::Uninstall => "uninstall",
        Operation::Update => "update",
    };

    let title = match op {
        Operation::Uninstall => format!("Uninstall {}?", plan.display_name),
        Operation::Update => format!("Update {}?", plan.display_name),
    };

    let dialog = adw::AlertDialog::new(Some(&title), Some(&plan_body(&plan)));
    dialog.set_body_use_markup(true);
    dialog.add_response("cancel", "Cancel");
    dialog.add_response(
        apply_id,
        match op {
            Operation::Uninstall => "Uninstall",
            Operation::Update => "Update",
        },
    );
    dialog.set_response_appearance(apply_id, adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let window = app.window.clone();
    let app2 = app.clone();
    glib::MainContext::default().spawn_local(async move {
        let answer = dialog.choose_future(Some(&window)).await;
        if answer == apply_id {
            start_task(&app2, op, key, plan);
        }
    });
}

/// Human-readable plan summary for the confirmation body.
fn plan_body(plan: &OperationPlan) -> String {
    let mut lines: Vec<String> = Vec::new();

    if !plan.current_version.is_empty() {
        let target = if plan.target_version.is_empty() {
            "latest".to_string()
        } else {
            plan.target_version.clone()
        };
        lines.push(format!(
            "<b>v{} → v{}</b>",
            esc(&plan.current_version),
            esc(&target)
        ));
    }

    let scope = match plan.install_scope {
        Some(InstallScope::User) => "user",
        Some(InstallScope::System) => "system-wide",
        None => "",
    };
    let mut source_line = format!("Source: {}", source_label(plan.source));
    if !scope.is_empty() {
        source_line.push_str(&format!(" · {scope}"));
    }
    lines.push(esc(&source_line));

    lines.push(match plan.auth_method {
        AuthMethod::Pkexec => {
            "Authentication: you may be asked for your password (Polkit).".to_string()
        }
        AuthMethod::None => "Authentication: none needed.".to_string(),
    });

    if !plan.steps.is_empty() {
        lines.push(String::new());
        lines.push("This will:".to_string());
        for (i, step) in plan.steps.iter().enumerate() {
            lines.push(format!("{}. {}", i + 1, esc(&step.description)));
            if !step.command_summary.is_empty() {
                lines.push(format!("   <tt>{}</tt>", esc(&step.command_summary)));
            }
        }
    }

    lines.join("\n")
}

fn source_label(source: crate::backend::package::PackageSource) -> &'static str {
    match source {
        crate::backend::package::PackageSource::Apt => "APT",
        crate::backend::package::PackageSource::Snap => "Snap",
        crate::backend::package::PackageSource::Flatpak => "Flatpak",
        crate::backend::package::PackageSource::AppImage => "AppImage",
        crate::backend::package::PackageSource::Manual => "Manual",
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Dialog confirmed: close, register a running task, execute in background.
///
/// The apply path revalidates against a fresh full scan (seconds) and then may
/// block on the Polkit password dialog — both silent windows that previously
/// looked hung. Cover them explicitly: the task opens with a stage message
/// naming the revalidation step (and the possible auth prompt), the row shows
/// a spinner, and a toast says the same up front. No extra `pkexec` calls;
/// revalidation still happens inside `backend.apply_*` before anything runs.
fn start_task(app: &Rc<App>, op: Operation, key: String, plan: OperationPlan) {
    let initial_message = match plan.auth_method {
        AuthMethod::Pkexec => {
            "Revalidating plan… a password prompt may appear next.".to_string()
        }
        AuthMethod::None => "Revalidating plan…".to_string(),
    };
    let task_id = app.with_state(|s| {
        let id = s.next_task_id;
        s.next_task_id += 1;
        s.tasks.push(Task {
            id,
            op,
            package_name: plan.display_name.clone(),
            status: TaskStatus::Running,
            message: initial_message,
            logs: String::new(),
        });
        id
    });

    let busy_label = match op {
        Operation::Uninstall => "Removing…",
        Operation::Update => "Updating…",
    };
    crate::app::set_busy(app, &key, Some(busy_label));
    crate::tasks::rebuild(app);
    // Non-blocking heads-up for the silent rescan-to-dialog window. The task
    // row + row spinner carry the ongoing state; this toast names the wait.
    app.toast(match plan.auth_method {
        AuthMethod::Pkexec => "Working… revalidating, then waiting for authentication.",
        AuthMethod::None => "Working… revalidating plan.",
    });

    let backend = app.backend.clone();
    let app2 = app.clone();
    let plan_id = plan.plan_id.clone();
    bridge::spawn(
        async move {
            match op {
                Operation::Uninstall => backend.apply_uninstall(&plan_id).await,
                Operation::Update => backend.apply_update(&plan_id).await,
            }
        },
        move |result| on_settled(&app2, op, key, task_id, result),
    );
}

fn on_settled(
    app: &Rc<App>,
    _op: Operation,
    key: String,
    task_id: u64,
    result: Result<OperationResult, String>,
) {
    let (success, message, logs) = match result {
        Ok(r) => (r.success, r.message, r.logs),
        Err(message) => (false, message, String::new()),
    };

    app.with_state(|s| {
        if let Some(task) = s.tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = if success { TaskStatus::Success } else { TaskStatus::Failed };
            task.message = message.clone();
            task.logs = logs;
        }
    });

    crate::app::set_busy(app, &key, None);

    if success {
        // Backend messages read like "Removed Firefox" / "Updated Firefox".
        app.toast(if message.is_empty() { "Done" } else { &message });
    } else {
        app.toast(&format!("Operation failed: {message}"));
    }

    crate::tasks::rebuild(app);
    if success {
        crate::tasks::auto_dismiss(app, task_id);
        // Refresh data so the list reflects the new system state.
        crate::packages::start_scan(app);
    }
}
