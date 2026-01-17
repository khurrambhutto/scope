//! Details view - shows full package information

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Render the details popup
pub fn render(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    if let Some(pkg) = app.selected_package() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(10),    // Details
                Constraint::Length(3),  // Footer
            ])
            .split(area);

        // Package name as title
        let title = Paragraph::new(Line::from(vec![
            Span::styled(
                &pkg.name,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("({})", pkg.source),
                Style::default().fg(pkg.source.color()),
            ),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));

        frame.render_widget(title, chunks[0]);

        // Package details
        let size_human = pkg.size_human();
        let mut details_lines = vec![
            Line::from(vec![
                Span::styled("Version:     ", Style::default().fg(Color::Yellow)),
                Span::raw(&pkg.version),
            ]),
            Line::from(vec![
                Span::styled("Size:        ", Style::default().fg(Color::Yellow)),
                Span::styled(size_human, Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled("Type:        ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    pkg.app_type.to_string(),
                    Style::default().fg(match pkg.app_type {
                        crate::package::AppType::GUI => Color::Green,
                        crate::package::AppType::CLI => Color::Blue,
                        crate::package::AppType::Unknown => Color::DarkGray,
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Source:      ", Style::default().fg(Color::Yellow)),
                Span::styled(pkg.source.to_string(), Style::default().fg(pkg.source.color())),
            ]),
        ];

        // Add install path if available
        if let Some(ref path) = pkg.install_path {
            details_lines.push(Line::from(vec![
                Span::styled("Path:        ", Style::default().fg(Color::Yellow)),
                Span::raw(path),
            ]));
        }

        // Add update info
        details_lines.push(Line::from(""));
        match pkg.has_update {
            Some(true) => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:      ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("Available ({})", pkg.update_version.as_deref().unwrap_or("?")),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            Some(false) => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:      ", Style::default().fg(Color::Yellow)),
                    Span::raw("Up to date"),
                ]));
            }
            None => {
                details_lines.push(Line::from(vec![
                    Span::styled("Update:      ", Style::default().fg(Color::Yellow)),
                    Span::styled("Not checked", Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        // Add description
        details_lines.push(Line::from(""));
        details_lines.push(Line::from(vec![
            Span::styled("Description:", Style::default().fg(Color::Yellow)),
        ]));
        
        // Word-wrap description
        let desc = if pkg.description.is_empty() {
            "No description available".to_string()
        } else {
            pkg.description.clone()
        };
        
        details_lines.push(Line::from(vec![
            Span::raw(desc),
        ]));

        let details = Paragraph::new(details_lines)
            .wrap(Wrap { trim: true })
            .scroll((app.details_scroll, 0));

        frame.render_widget(details, chunks[1]);

        // Footer with actions
        let footer_text = if pkg.has_update == Some(true) {
            " [Esc] Back | [d] Uninstall | [u] Update "
        } else {
            " [Esc] Back | [d] Uninstall "
        };

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));

        frame.render_widget(footer, chunks[2]);

        // Outer border
        let border = Block::default()
            .borders(Borders::ALL)
            .title(" Package Details ")
            .border_style(Style::default().fg(Color::Cyan));
        
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
