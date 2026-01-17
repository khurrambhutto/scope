//! Dialog widgets - confirmation, loading, error

use crate::app::{App, ConfirmAction};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Render confirmation dialog
pub fn render_confirm(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 9, frame.area());
    frame.render_widget(Clear, area);

    let (title, message) = match app.confirm_action {
        Some(ConfirmAction::Uninstall) => {
            let pkg_name = app.selected_package().map(|p| p.name.as_str()).unwrap_or("?");
            (
                " Confirm Uninstall ",
                format!(
                    "Are you sure you want to uninstall '{}'?\n\nThis action cannot be undone.",
                    pkg_name
                ),
            )
        }
        Some(ConfirmAction::Update) => {
            let pkg_name = app.selected_package().map(|p| p.name.as_str()).unwrap_or("?");
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

    let border_color = match app.confirm_action {
        Some(ConfirmAction::Uninstall) => Color::Red,
        Some(ConfirmAction::Update) => Color::Yellow,
        None => Color::White,
    };

    let dialog = Paragraph::new(vec![
        Line::from(""),
        Line::from(message),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" Yes  "),
            Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" No"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color)),
    )
    .style(Style::default().fg(Color::White));

    frame.render_widget(dialog, area);
}

/// Render loading indicator
pub fn render_loading(frame: &mut Frame, app: &App) {
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
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw(&app.loading_message),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Loading ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(loading, area);
}

/// Render error dialog
pub fn render_error(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 10, frame.area());
    frame.render_widget(Clear, area);

    let error = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " Error: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(app.error_message.clone()),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter]", Style::default().fg(Color::Yellow)),
            Span::raw(" Continue  "),
            Span::styled("[q]", Style::default().fg(Color::Yellow)),
            Span::raw(" Quit"),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Error ")
            .border_style(Style::default().fg(Color::Red)),
    );

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
