//! UI module for scope TUI

pub mod main_view;
pub mod details_view;
pub mod dialogs;

use crate::app::{App, View};
use ratatui::Frame;

/// Render the current view based on app state
pub fn render(frame: &mut Frame, app: &App) {
    match app.view {
        View::Main | View::Search => main_view::render(frame, app),
        View::Details => details_view::render(frame, app),
        View::Confirm => {
            main_view::render(frame, app);
            dialogs::render_confirm(frame, app);
        }
        View::UpdateSelect => main_view::render_update_select(frame, app),
        View::Loading => dialogs::render_loading(frame, app),
        View::Error => dialogs::render_error(frame, app),
    }
}
