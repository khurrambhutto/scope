//! Main view - package list display

use crate::app::{App, View};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

/// Render the main package list view
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Table
            Constraint::Length(3), // Footer/Help
        ])
        .split(frame.area());

    render_header(frame, chunks[0], app);
    render_table(frame, chunks[1], app);
    render_footer(frame, chunks[2], app);

    // Render search overlay if in search mode
    if app.view == View::Search {
        render_search_overlay(frame, app);
    }
}

/// Render header with title and stats
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let (total, apt, snap, flatpak, appimage) = app.get_stats();
    let update_count = app.get_update_count();

    let title = format!(
        " SCOPE - Linux Package Manager | {} packages (APT: {} | Snap: {} | Flatpak: {} | AppImage: {})",
        total, apt, snap, flatpak, appimage
    );

    let mut spans = vec![Span::styled(
        title,
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )];

    if update_count > 0 {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            format!("{} updates available", update_count),
            Style::default().fg(Color::Yellow),
        ));
    }

    let header = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));

    frame.render_widget(header, area);
}

/// Render the package table
fn render_table(frame: &mut Frame, area: Rect, app: &App) {
    let header_cells = ["", "Name", "Source", "Type", "Version", "Size", "Upd"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));

    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .filtered_packages
        .iter()
        .enumerate()
        .map(|(i, &pkg_idx)| {
            let pkg = &app.packages[pkg_idx];
            let is_selected = i == app.selected;

            let style = if is_selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let selector = if is_selected { ">" } else { " " };
            let update_indicator = match pkg.has_update {
                Some(true) => "Y",
                Some(false) => " ",
                None => "?",
            };

            Row::new(vec![
                Cell::from(selector),
                Cell::from(pkg.name.clone()).style(Style::default().fg(Color::White)),
                Cell::from(pkg.source.to_string()).style(Style::default().fg(pkg.source.color())),
                Cell::from(pkg.app_type.to_string()).style(Style::default().fg(
                    match pkg.app_type {
                        crate::package::AppType::GUI => Color::Green,
                        crate::package::AppType::CLI => Color::Blue,
                        crate::package::AppType::Unknown => Color::DarkGray,
                    }
                )),
                Cell::from(pkg.version.clone()),
                Cell::from(pkg.size_human()).style(Style::default().fg(Color::Magenta)),
                Cell::from(update_indicator).style(
                    if pkg.has_update == Some(true) {
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    }
                ),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Percentage(30),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Percentage(20),
        Constraint::Length(12),
        Constraint::Length(4),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Packages ({}/{}) - Sort: {} - Filter: {} ",
                    app.filtered_packages.len(),
                    app.packages.len(),
                    app.sort_criteria.label(),
                    app.app_type_filter.label()
                ))
                .border_style(Style::default().fg(Color::White)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    state.select(Some(app.selected));
    frame.render_stateful_widget(table, area, &mut state);
}

/// Render the footer with help text
fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help_text = if app.view == View::Search {
        " Type to search | Enter: Apply | Esc: Cancel | Ctrl+U: Clear "
    } else {
        " [Enter] Details | [/] Search | [d] Uninstall | [c] Check Updates | [U] Update All | [s] Sort | [f] Filter | [q] Quit "
    };

    let search_info = if !app.search_query.is_empty() {
        format!(" | Search: \"{}\"", app.search_query)
    } else {
        String::new()
    };

    let footer = Paragraph::new(format!("{}{}", help_text, search_info))
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));

    frame.render_widget(footer, area);
}

/// Render search input overlay
fn render_search_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 3, frame.area());
    
    frame.render_widget(Clear, area);
    
    let search_input = Paragraph::new(format!(" {}_", app.search_query))
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search ")
                .border_style(Style::default().fg(Color::Yellow)),
        );

    frame.render_widget(search_input, area);
}

/// Render update selection view
pub fn render_update_select(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // Header
    let update_count = app.get_update_count();
    let selected_count = app.packages.iter().filter(|p| p.selected).count();
    
    let header = Paragraph::new(format!(
        " Select packages to update ({} selected / {} available)",
        selected_count, update_count
    ))
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow)));
    
    frame.render_widget(header, chunks[0]);

    // Table of updateable packages
    let header_cells = ["", "Sel", "Name", "Source", "Current", "New Version"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));

    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .update_selection
        .iter()
        .enumerate()
        .map(|(i, &pkg_idx)| {
            let pkg = &app.packages[pkg_idx];
            let is_selected = i == app.selected;

            let style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            let selector = if is_selected { ">" } else { " " };
            let check = if pkg.selected { "[x]" } else { "[ ]" };

            Row::new(vec![
                Cell::from(selector),
                Cell::from(check).style(
                    if pkg.selected {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default()
                    }
                ),
                Cell::from(pkg.name.clone()),
                Cell::from(pkg.source.to_string()).style(Style::default().fg(pkg.source.color())),
                Cell::from(pkg.version.clone()),
                Cell::from(pkg.update_version.clone().unwrap_or_default())
                    .style(Style::default().fg(Color::Green)),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(4),
        Constraint::Percentage(30),
        Constraint::Length(10),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Updates Available "));

    frame.render_widget(table, chunks[1]);

    // Footer
    let footer = Paragraph::new(" [Space] Toggle | [a] Select All | [n] Select None | [Enter] Update Selected | [Esc] Cancel ")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    
    frame.render_widget(footer, chunks[2]);
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height.min(100)) / 2),
            Constraint::Length(height),
            Constraint::Percentage((100 - height.min(100)) / 2),
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
