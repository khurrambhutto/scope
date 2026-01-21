//! Details view - shows full package information

use crate::app::App;
use crate::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
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
        // Calculate content height to center it
        let content_lines = 6; // title + details lines
        let footer_height = 2;
        let total_content = content_lines + footer_height;
        let available_height = area.height.saturating_sub(2); // margin
        let top_padding = available_height.saturating_sub(total_content as u16) / 3;
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(top_padding.max(1)),      // Top padding
                Constraint::Length(2),                       // Title
                Constraint::Length(content_lines as u16),    // Content
                Constraint::Length(1),                       // Small gap
                Constraint::Length(1),                       // Footer
                Constraint::Min(0),                          // Bottom space
            ])
            .split(area);

        // Package name as title (centered, no underline)
        let title = Paragraph::new(Line::from(vec![
            Span::styled(&pkg.name, theme.primary_bold()),
            Span::raw(" "),
            Span::styled(format!("({})", pkg.source), theme.primary_style()),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(theme.base_style());

        frame.render_widget(title, chunks[1]);

        // Package details (centered)
        let size_human = pkg.size_human();
        let mut details_lines = vec![
            Line::from(vec![
                Span::styled("Version:  ", theme.label_style()),
                Span::raw(&pkg.version),
            ]),
            Line::from(vec![
                Span::styled("Size:     ", theme.label_style()),
                Span::styled(size_human, theme.primary_style()),
            ]),
            Line::from(vec![
                Span::styled("Type:     ", theme.label_style()),
                Span::styled(pkg.app_type.to_string(), theme.primary_style()),
            ]),
        ];

        if let Some(ref path) = pkg.install_path {
            details_lines.push(Line::from(vec![
                Span::styled("Path:     ", theme.label_style()),
                Span::raw(&path[..path.len().min(40)]),
            ]));
        }

        match pkg.has_update {
            Some(true) => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:   ", theme.label_style()),
                    Span::styled(
                        format!("Available ({})", pkg.update_version.as_deref().unwrap_or("?")),
                        theme.success_style(),
                    ),
                ]));
            }
            Some(false) => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:   ", theme.label_style()),
                    Span::raw("Up to date"),
                ]));
            }
            None => {}
        }

        let details = Paragraph::new(details_lines)
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(theme.base_style());

        frame.render_widget(details, chunks[2]);

        // Footer (centered, no border)
        let footer_text = if pkg.has_update == Some(true) {
            "[Esc] Back  |  [d] Uninstall  |  [u] Update"
        } else {
            "[Esc] Back  |  [d] Uninstall"
        };

        let footer = Paragraph::new(footer_text)
            .alignment(ratatui::layout::Alignment::Center)
            .style(theme.muted_style());

        frame.render_widget(footer, chunks[4]);
    }
}
