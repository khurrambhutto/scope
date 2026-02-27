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
            Span::styled("[Esc]", theme.primary_style()),
            Span::raw(" Back"),
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
            Span::styled("[Esc]", theme.primary_style()),
            Span::raw(" Back"),
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

/// Render update by source selection dialog
pub fn render_update_by_source_in_area(frame: &mut Frame, app: &App, area: Rect) {
    use crate::package::PackageSource;
    
    let theme = get_theme();
    let dialog_area = centered_rect_in_area(60, 14, area);
    frame.render_widget(Clear, dialog_area);

    let sources = [
        (PackageSource::Apt, "APT"),
        (PackageSource::Snap, "Snap"),
        (PackageSource::Flatpak, "Flatpak"),
    ];

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (i, (source, label)) in sources.iter().enumerate() {
        let is_selected = app.selected_update_source == i;
        let count = app
            .update_source_counts
            .as_ref()
            .and_then(|c| c.get(source))
            .copied()
            .unwrap_or(0);

        let count_str = if app.updates_checked {
            format!(" ({})", count)
        } else {
            " (?)".to_string()
        };

        let prefix = if is_selected { " > " } else { "   " };
        let style = if is_selected {
            theme.selection_style()
        } else {
            theme.base_style()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{}{:<10}", prefix, label), style),
            Span::styled(count_str, theme.muted_style()),
        ]));
    }

    // Separator
    lines.push(Line::from("   ─────────────────────"));

    // All option at bottom
    let is_all_selected = app.selected_update_source == 3;
    let total_count = app.get_total_update_count();
    let total_str = if app.updates_checked {
        format!(" ({})", total_count)
    } else {
        " (?)".to_string()
    };
    let all_prefix = if is_all_selected { " > " } else { "   " };
    let all_style = if is_all_selected {
        theme.selection_style()
    } else {
        theme.base_style()
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{}{:<10}", all_prefix, "All"), all_style),
        Span::styled(total_str, theme.muted_style()),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[c]", theme.primary_style()),
        Span::raw(" Check  "),
        Span::styled("[Enter]", theme.primary_style()),
        Span::raw(" Update"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("[Esc]", theme.muted_style()),
        Span::raw(" Back"),
    ]));

    let dialog = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Update Packages ")
                .title_style(theme.title_style())
                .border_style(theme.primary_style()),
        )
        .style(theme.base_style());

    frame.render_widget(dialog, dialog_area);
}

/// Render update progress dialog
pub fn render_update_progress_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();
    let dialog_area = centered_rect_in_area(60, 8, area);
    frame.render_widget(Clear, dialog_area);

    let progress = &app.update_progress;
    let source_name = progress
        .source
        .map(|s| s.to_string())
        .unwrap_or_else(|| "All".to_string());

    // Progress bar
    let filled = if progress.total > 0 {
        (progress.current * 20) / progress.total
    } else {
        0
    };
    let empty = 20 - filled;
    let progress_bar = format!(
        "[{}{}] {}/{}",
        "█".repeat(filled),
        "░".repeat(empty),
        progress.current,
        progress.total
    );

    let spinner_frames = ['|', '/', '-', '\\'];
    let spinner_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        / 100) as usize
        % spinner_frames.len();

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                format!(" {} ", spinner_frames[spinner_idx]),
                theme.primary_bold(),
            ),
            Span::raw(&progress.current_package),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  {}", progress_bar),
            theme.primary_style(),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Esc]", theme.muted_style()),
            Span::styled(" Cancel", theme.muted_style()),
        ]),
    ];

    let dialog = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(" Updating {} ", source_name))
                .title_style(theme.title_style())
                .border_style(theme.warning_style()),
        )
        .style(theme.base_style());

    frame.render_widget(dialog, dialog_area);
}

/// Render cancel confirmation dialog
pub fn render_cancel_confirm_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();
    let dialog_area = centered_rect_in_area(55, 8, area);
    frame.render_widget(Clear, dialog_area);

    let progress = &app.update_progress;

    let lines = vec![
        Line::from(""),
        Line::from("  Stop updating packages?"),
        Line::from(format!(
            "  {}/{} packages completed.",
            progress.success_count, progress.total
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [y]", theme.error_style().add_modifier(Modifier::BOLD)),
            Span::raw(" Yes, stop  "),
            Span::styled("[n]", theme.success_style().add_modifier(Modifier::BOLD)),
            Span::raw(" Continue"),
        ]),
    ];

    let dialog = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Cancel Update? ")
                .title_style(theme.title_style())
                .border_style(theme.warning_style()),
        )
        .style(theme.base_style());

    frame.render_widget(dialog, dialog_area);
}

/// Render update summary dialog
pub fn render_update_summary_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();
    
    let progress = &app.update_progress;
    let has_errors = !progress.errors.is_empty();
    let skipped = progress.total.saturating_sub(progress.success_count + progress.errors.len());
    
    // Calculate height based on errors
    let error_lines = progress.errors.len().min(3); // Show max 3 errors
    let height = 9 + error_lines as u16;
    
    let dialog_area = centered_rect_in_area(65, height, area);
    frame.render_widget(Clear, dialog_area);

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ✓ ", theme.success_style()),
            Span::raw(format!("Updated: {} packages", progress.success_count)),
        ]),
    ];

    if has_errors {
        lines.push(Line::from(vec![
            Span::styled("  ✗ ", theme.error_style()),
            Span::raw(format!("Failed:  {} packages", progress.errors.len())),
        ]));
    }

    if progress.cancelled && skipped > 0 {
        lines.push(Line::from(vec![
            Span::styled("  ○ ", theme.muted_style()),
            Span::raw(format!("Skipped: {} packages", skipped)),
        ]));
    }

    if has_errors {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  Errors:", theme.error_style())));
        for (name, err) in progress.errors.iter().take(3) {
            let error_msg = if err.len() > 30 {
                format!("{}...", &err[..27])
            } else {
                err.clone()
            };
            lines.push(Line::from(Span::styled(
                format!("    - {}: {}", name, error_msg),
                theme.muted_style(),
            )));
        }
        if progress.errors.len() > 3 {
            lines.push(Line::from(Span::styled(
                format!("    ... and {} more", progress.errors.len() - 3),
                theme.muted_style(),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  [Enter]", theme.primary_style()),
        Span::raw(" Continue"),
    ]));

    let title = if progress.cancelled {
        " Update Cancelled "
    } else {
        " Update Complete "
    };

    let border_style = if has_errors {
        theme.warning_style()
    } else {
        theme.success_style()
    };

    let dialog = Paragraph::new(lines)
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
