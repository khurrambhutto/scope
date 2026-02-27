//! Details view - shows full package information

use crate::app::App;
use crate::theme::get_theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Render the details popup (full-screen version - deprecated)
#[allow(dead_code)]
pub fn render(frame: &mut Frame, app: &App) {
    let theme = get_theme();
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    if let Some(pkg) = app.selected_package() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(10),   // Details
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Package name as title
        let title = Paragraph::new(Line::from(vec![
            Span::styled(&pkg.name, theme.primary_bold()),
            Span::raw(" "),
            Span::styled(format!("({})", pkg.source), theme.primary_style()),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style()),
        )
        .style(theme.base_style());

        frame.render_widget(title, chunks[0]);

        // Package details
        let size_human = pkg.size_human();
        let mut details_lines = vec![
            Line::from(vec![
                Span::styled("Version:     ", theme.label_style()),
                Span::raw(&pkg.version),
            ]),
            Line::from(vec![
                Span::styled("Size:        ", theme.label_style()),
                Span::styled(size_human, theme.primary_style()),
            ]),
            Line::from(vec![
                Span::styled("Type:        ", theme.label_style()),
                Span::styled(pkg.app_type.to_string(), theme.primary_style()),
            ]),
            Line::from(vec![
                Span::styled("Source:      ", theme.label_style()),
                Span::styled(pkg.source.to_string(), theme.primary_style()),
            ]),
        ];

        // Add install path if available
        if let Some(ref path) = pkg.install_path {
            details_lines.push(Line::from(vec![
                Span::styled("Path:        ", theme.label_style()),
                Span::raw(path),
            ]));
        }

        // Add update info
        details_lines.push(Line::from(""));
        match pkg.has_update {
            Some(true) => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:      ", theme.label_style()),
                    Span::styled(
                        format!(
                            "Available ({})",
                            pkg.update_version.as_deref().unwrap_or("?")
                        ),
                        theme.success_style().add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            Some(false) => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:      ", theme.label_style()),
                    Span::raw("Up to date"),
                ]));
            }
            None => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:      ", theme.label_style()),
                    Span::styled("Not checked", theme.muted_style()),
                ]));
            }
        }

        // Add description
        details_lines.push(Line::from(""));
        details_lines.push(Line::from(vec![Span::styled(
            "Description:",
            theme.label_style(),
        )]));

        // Word-wrap description
        let desc = if pkg.description.is_empty() {
            "No description available".to_string()
        } else {
            pkg.description.clone()
        };

        details_lines.push(Line::from(vec![Span::raw(desc)]));

        let details = Paragraph::new(details_lines)
            .wrap(Wrap { trim: true })
            .scroll((app.details_scroll, 0))
            .style(theme.base_style());

        frame.render_widget(details, chunks[1]);

        // Footer with actions
        let footer_text = if pkg.has_update == Some(true) {
            " [Esc] Back | [d] Uninstall | [u] Update "
        } else {
            " [Esc] Back | [d] Uninstall "
        };

        let footer = Paragraph::new(footer_text)
            .style(theme.muted_style())
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.border_style()),
            );

        frame.render_widget(footer, chunks[2]);

        // Outer border
        let border = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Package Details ")
            .title_style(theme.title_style())
            .border_style(theme.border_style())
            .style(theme.base_style());

        frame.render_widget(border, area);
    }
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
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

/// Render the details view within a specific area
pub fn render_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();
    frame.render_widget(Clear, area);

    // Fill background
    let bg_block = Block::default().style(theme.base_style());
    frame.render_widget(bg_block, area);

    if let Some(pkg) = app.selected_package() {
        let available_w = area.width.saturating_sub(2);
        let available_h = area.height.saturating_sub(2);
        if available_w < 20 || available_h < 10 {
            return;
        }

        let card_w = available_w.min(88);
        let card_h = if available_h > 14 {
            available_h.min(20)
        } else {
            available_h
        };
        let card_area = Rect {
            x: area.x + (area.width.saturating_sub(card_w)) / 2,
            y: area.y + (area.height.saturating_sub(card_h)) / 2,
            width: card_w,
            height: card_h,
        };

        let card = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" App Details ")
            .title_style(theme.title_style())
            .border_style(theme.border_style())
            .style(theme.base_style());
        frame.render_widget(card, card_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(8), // Info rows + borders
                Constraint::Min(0),    // Spacer
                Constraint::Length(3), // Actions
            ])
            .split(card_area);

        let source_badge = Span::styled(
            format!(" {} ", pkg.source.to_string().to_uppercase()),
            Style::default()
                .fg(theme.background)
                .bg(theme.source_color(&pkg.source))
                .add_modifier(Modifier::BOLD),
        );
        let header = Paragraph::new(Line::from(vec![
            Span::styled(&pkg.name, theme.primary_bold()),
            Span::raw("  "),
            source_badge,
        ]))
        .style(theme.base_style());
        frame.render_widget(header, chunks[0]);

        let type_label = match pkg.app_type {
            crate::package::AppType::GUI => "GUI",
            crate::package::AppType::CLI => "CLI",
            crate::package::AppType::Unknown => "Unknown",
        };
        let size_human = pkg.size_human();
        let path_value = pkg.install_path.as_deref().unwrap_or("Not available");
        let info_label_width = 16usize;
        let label_cell = |label: &str| {
            Span::styled(
                format!("{:<width$}", label, width = info_label_width),
                theme.label_style(),
            )
        };

        let description = if pkg.description.trim().is_empty() {
            "Not available"
        } else {
            pkg.description.trim()
        };

        let mut info_lines = vec![
            Line::from(vec![
                label_cell("Description:"),
                Span::styled(description, theme.primary_style()),
            ]),
            Line::from(vec![
                label_cell("Version:"),
                Span::styled(&pkg.version, theme.primary_style()),
            ]),
            Line::from(vec![
                label_cell("Installed Size:"),
                Span::styled(size_human, theme.primary_style()),
            ]),
            Line::from(vec![
                label_cell("Type:"),
                Span::styled(
                    type_label,
                    Style::default().fg(theme.app_type_color(&pkg.app_type)),
                ),
            ]),
            Line::from(vec![
                label_cell("Path:"),
                Span::styled(path_value, theme.muted_style()),
            ]),
        ];

        match pkg.has_update {
            Some(true) => {
                info_lines.push(Line::from(vec![
                    label_cell("Update:"),
                    Span::styled(
                        format!(
                            "Available ({})",
                            pkg.update_version.as_deref().unwrap_or("?")
                        ),
                        theme.warning_style().add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            Some(false) => {
                info_lines.push(Line::from(vec![
                    label_cell("Update:"),
                    Span::styled("Up to date", theme.success_style()),
                ]));
            }
            None => {
                info_lines.push(Line::from(vec![
                    label_cell("Update:"),
                    Span::styled("Not checked", theme.muted_style()),
                ]));
            }
        }

        let details = Paragraph::new(info_lines)
            .wrap(Wrap { trim: true })
            .scroll((app.details_scroll, 0))
            .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Info ")
                .title_style(theme.title_style())
                .border_style(theme.border_style()),
        );
        frame.render_widget(details, chunks[1]);

        let action_line = if pkg.has_update == Some(true) {
            Line::from(vec![
                Span::styled("[Esc]", theme.primary_style()),
                Span::styled(" Back  |  ", theme.muted_style()),
                Span::styled("[u]", theme.success_style().add_modifier(Modifier::BOLD)),
                Span::styled(" Update  |  ", theme.muted_style()),
                Span::styled("[d]", theme.error_style().add_modifier(Modifier::BOLD)),
                Span::styled(" Uninstall", theme.muted_style()),
            ])
        } else {
            Line::from(vec![
                Span::styled("[Esc]", theme.primary_style()),
                Span::styled(" Back  |  ", theme.muted_style()),
                Span::styled("[d]", theme.error_style().add_modifier(Modifier::BOLD)),
                Span::styled(" Uninstall", theme.muted_style()),
            ])
        };
        let footer = Paragraph::new(action_line)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.border_style()),
            );
        frame.render_widget(footer, chunks[3]);
    }
}
