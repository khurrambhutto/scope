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
    let sections = [
        SidebarSection::Apps,
        SidebarSection::Update,
        SidebarSection::Install,
        SidebarSection::Clean,
    ];

    let mut lines: Vec<Line> = Vec::new();

    // Calculate vertical centering (account for spacing between items)
    let section_count = sections.len() as u16;
    let total_lines = section_count * 2; // Each item + spacing
    let top_padding = inner_area.height.saturating_sub(total_lines + 2) / 3;
    
    // Add top padding for centering
    for _ in 0..top_padding {
        lines.push(Line::from(""));
    }

    for (i, section) in sections.iter().enumerate() {
        let is_selected = *section == app.sidebar_section;

        let style = if is_selected {
            theme.sidebar_selected_style()
        } else {
            theme.sidebar_style()
        };

        // Icon based on selection
        let icon = if is_selected { ">" } else { " " };

        // Create the menu item line (left aligned with small indent)
        let label = format!("  {} {}", icon, section.label());
        
        let line = Line::from(vec![Span::styled(
            label,
            style.add_modifier(if is_selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        )]);

        lines.push(line);
        
        // Add spacing between items (except after last item)
        if i < sections.len() - 1 {
            lines.push(Line::from(""));
        }
    }

    // Add hint at bottom for navigation (only when focused)
    if app.sidebar_focused {
        // Add spacing before hint
        let remaining_height = inner_area.height.saturating_sub(lines.len() as u16 + 2);
        for _ in 0..remaining_height {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled("   [↑/↓] Nav", theme.muted_style())));
        lines.push(Line::from(Span::styled(
            "   [Enter] Select",
            theme.muted_style(),
        )));
    }

    let sidebar_content = Paragraph::new(lines).style(theme.base_style());

    frame.render_widget(sidebar_content, inner_area);
}
