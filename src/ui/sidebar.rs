//! Sidebar component for scope TUI
//!
//! Displays navigation items for different sections of the app.

use crate::app::{App, SidebarSection};
use crate::theme::get_theme;
use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

/// Render the sidebar
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();

    // Create sidebar block with border
    let sidebar_block = Block::default()
        .borders(Borders::RIGHT)
        .border_type(BorderType::Rounded)
        .border_style(if app.sidebar_focused {
            theme.border_focused_style()
        } else {
            theme.border_style()
        })
        .style(theme.base_style());

    frame.render_widget(sidebar_block, area);

    // Create inner area for content
    let inner_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Sidebar sections
    let sections = [SidebarSection::Apps, SidebarSection::Updates, SidebarSection::Clean];

    let mut lines: Vec<Line> = Vec::new();

    // Add some top padding
    lines.push(Line::from(""));

    for section in sections.iter() {
        let is_selected = *section == app.sidebar_section;

        let style = if is_selected {
            theme.sidebar_selected_style()
        } else {
            theme.sidebar_style()
        };

        // Icon based on section
        let icon = if is_selected { ">" } else { " " };

        // Create the menu item line with optional badge for Updates
        let line = if *section == SidebarSection::Updates {
            let update_count = app.get_update_count();
            if update_count > 0 {
                Line::from(vec![
                    Span::styled(
                        format!(" {} {} ", icon, section.label()),
                        style.add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                    Span::styled(
                        format!("[{}]", update_count),
                        theme.warning_style().add_modifier(Modifier::BOLD),
                    ),
                ])
            } else {
                Line::from(vec![Span::styled(
                    format!(" {} {} ", icon, section.label()),
                    style.add_modifier(if is_selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                )])
            }
        } else {
            Line::from(vec![Span::styled(
                format!(" {} {} ", icon, section.label()),
                style.add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            )])
        };

        lines.push(line);
    }

    // Add hint at bottom for navigation
    if app.sidebar_focused {
        // Add spacing before hint
        let remaining_height = inner_area.height.saturating_sub(lines.len() as u16 + 2);
        for _ in 0..remaining_height {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(" [j/k] Nav", theme.muted_style())));
        lines.push(Line::from(Span::styled(
            " [Enter] Select",
            theme.muted_style(),
        )));
    }

    let sidebar_content = Paragraph::new(lines).style(theme.base_style());

    frame.render_widget(sidebar_content, inner_area);
}
