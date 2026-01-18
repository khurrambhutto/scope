//! UI module for scope TUI

pub mod details_view;
pub mod dialogs;
pub mod main_view;
mod sidebar;

use crate::app::{App, View};
use crate::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Clear},
    Frame,
};

/// Render the current view (full-screen version - deprecated)
#[allow(dead_code)]
pub fn render(frame: &mut Frame, app: &App) {
    match app.view {
        View::Main => main_view::render(frame, app),
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

/// Render the current view within a specific area (for floating window mode)
pub fn render_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();

    // Clear the window area and fill with background color
    frame.render_widget(Clear, area);

    // Fill background
    let bg_block = Block::default().style(theme.base_style());
    frame.render_widget(bg_block, area);

    // Render outer window border
    let window_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style())
        .title(" SCOPE ")
        .title_style(theme.title_style())
        .style(theme.base_style());

    frame.render_widget(window_block, area);

    // Create inner area (accounting for outer border)
    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Split into sidebar (20%) and main content (80%)
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20), // Sidebar
            Constraint::Percentage(80), // Main content
        ])
        .split(inner_area);

    // Render sidebar
    sidebar::render(frame, app, horizontal_chunks[0]);

    // Render main content area
    let content_area = horizontal_chunks[1];

    // Render the app content within the content area
    match app.view {
        View::Main => main_view::render_in_area(frame, app, content_area),
        View::Details => details_view::render_in_area(frame, app, content_area),
        View::Confirm => {
            main_view::render_in_area(frame, app, content_area);
            dialogs::render_confirm_in_area(frame, app, content_area);
        }
        View::UpdateSelect => main_view::render_update_select_in_area(frame, app, content_area),
        View::Loading => dialogs::render_loading_in_area(frame, app, content_area),
        View::Error => dialogs::render_error_in_area(frame, app, content_area),
    }
}
