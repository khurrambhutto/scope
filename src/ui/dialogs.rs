//! Dialog widgets - confirmation, loading, error

use crate::app::{App, ConfirmAction};
use crate::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

/// Render confirmation dialog (full-screen version - deprecated)
#[allow(dead_code)]
pub fn render_confirm(frame: &mut Frame, app: &App) {
    let theme = get_theme();
    let area = centered_rect(50, 9, frame.area());
    frame.render_widget(Clear, area);

    let (title, message) = match app.confirm_action {
        Some(ConfirmAction::Uninstall) => {
            let pkg_name = app
                .selected_package()
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            (
                " Confirm Uninstall ",
                format!(
                    "Are you sure you want to uninstall '{}'?\n\nThis action cannot be undone.",
                    pkg_name
                ),
            )
        }
        Some(ConfirmAction::Update) => {
            let pkg_name = app
                .selected_package()
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            let new_ver = app
                .selected_package()
                .and_then(|p| p.update_version.as_deref())
                .unwrap_or("?");
            (
                " Confirm Update ",
                format!("Update '{}' to version {}?", pkg_name, new_ver),
            )
        }
        None => (" Confirm ", "Are you sure?".to_string()),
    };

    let border_style = match app.confirm_action {
        Some(ConfirmAction::Uninstall) => theme.error_style(),
        Some(ConfirmAction::Update) => theme.warning_style(),
        None => theme.border_style(),
    };

    let dialog = Paragraph::new(vec![
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", theme.success_style().add_modifier(Modifier::BOLD)),
            Span::raw(" Yes  "),
            Span::styled("[n]", theme.error_style().add_modifier(Modifier::BOLD)),
            Span::raw(" No"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .title_style(theme.title_style())
            .border_style(border_style),
    )
    .style(theme.base_style());

    frame.render_widget(dialog, area);
}

/// Render loading indicator (full-screen version - deprecated)
#[allow(dead_code)]
pub fn render_loading(frame: &mut Frame, app: &App) {
    let theme = get_theme();
    let area = centered_rect(50, 5, frame.area());
    frame.render_widget(Clear, area);

    let spinner_frames = ['|', '/', '-', '\\'];
    let spinner_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        / 100) as usize
        % spinner_frames.len();

    let loading = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" {} ", spinner_frames[spinner_idx]),
                theme.primary_bold(),
            ),
            Span::raw(&app.loading_message),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Loading ")
            .title_style(theme.title_style())
            .border_style(theme.border_style()),
    )
    .style(theme.base_style());

    frame.render_widget(loading, area);
}

/// Render error dialog (full-screen version - deprecated)
#[allow(dead_code)]
pub fn render_error(frame: &mut Frame, app: &App) {
    let theme = get_theme();
    let area = centered_rect(60, 10, frame.area());
    frame.render_widget(Clear, area);

    let error = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            " Error: ",
            theme.error_style().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(app.error_message.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter]", theme.primary_style()),
            Span::raw(" Continue  "),
            Span::styled("[q]", theme.primary_style()),
            Span::raw(" Quit"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Error ")
            .title_style(theme.title_style())
            .border_style(theme.error_style()),
    )
    .style(theme.base_style());

    frame.render_widget(error, area);
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical_margin = area.height.saturating_sub(height) / 2;

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(vertical_margin),
            Constraint::Length(height),
            Constraint::Length(vertical_margin),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Render confirmation dialog within a specific area
pub fn render_confirm_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();
    let dialog_area = centered_rect_in_area(70, 8, area);
    frame.render_widget(Clear, dialog_area);

    let (title, message) = match app.confirm_action {
        Some(ConfirmAction::Uninstall) => {
            let pkg_name = app
                .selected_package()
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            (" Confirm ", format!("Uninstall '{}'?", pkg_name))
        }
        Some(ConfirmAction::Update) => {
            let pkg_name = app
                .selected_package()
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            let new_ver = app
                .selected_package()
                .and_then(|p| p.update_version.as_deref())
                .unwrap_or("?");
            (
                " Confirm ",
                format!("Update '{}' to {}?", pkg_name, new_ver),
            )
        }
        None => (" Confirm ", "Are you sure?".to_string()),
    };

    let border_style = match app.confirm_action {
        Some(ConfirmAction::Uninstall) => theme.error_style(),
        Some(ConfirmAction::Update) => theme.warning_style(),
        None => theme.border_style(),
    };

    let dialog = Paragraph::new(vec![
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", theme.success_style().add_modifier(Modifier::BOLD)),
            Span::raw(" Yes  "),
            Span::styled("[n]", theme.error_style().add_modifier(Modifier::BOLD)),
            Span::raw(" No"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(title)
            .title_style(theme.title_style())
            .border_style(border_style),
    )
    .style(theme.base_style());

    frame.render_widget(dialog, dialog_area);
}

/// Render loading indicator within a specific area
pub fn render_loading_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();
    let loading_area = centered_rect_in_area(50, 3, area);
    frame.render_widget(Clear, loading_area);

    let spinner_frames = ['|', '/', '-', '\\'];
    let spinner_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        / 100) as usize
        % spinner_frames.len();

    let loading = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" {} ", spinner_frames[spinner_idx]),
                theme.primary_bold(),
            ),
            Span::raw(&app.loading_message),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Loading ")
            .title_style(theme.title_style())
            .border_style(theme.border_style()),
    )
    .style(theme.base_style());

    frame.render_widget(loading, loading_area);
}

/// Render error dialog within a specific area
pub fn render_error_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();
    let error_area = centered_rect_in_area(70, 7, area);
    frame.render_widget(Clear, error_area);

    let error = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            " Error: ",
            theme.error_style().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(app.error_message.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter]", theme.primary_style()),
            Span::raw(" Continue  "),
            Span::styled("[q]", theme.primary_style()),
            Span::raw(" Quit"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Error ")
            .title_style(theme.title_style())
            .border_style(theme.error_style()),
    )
    .style(theme.base_style());

    frame.render_widget(error, error_area);
}

/// Create a centered rectangle within a parent area
fn centered_rect_in_area(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vertical_margin = area.height.saturating_sub(height) / 2;

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(vertical_margin),
            Constraint::Length(height),
            Constraint::Length(vertical_margin),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
