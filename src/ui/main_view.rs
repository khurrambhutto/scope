//! Main view - package list display

use crate::app::App;
use crate::app::SourceTab;
use crate::theme::get_theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
    Frame,
};

/// Render the main package list view (full-screen version - deprecated)
#[allow(dead_code)]
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
}

/// Render header with source tabs and stats
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let theme = get_theme();

    // Split header into tabs and stats
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(45), // Tabs
            Constraint::Min(10),    // Stats
        ])
        .split(area);

    // Render source tabs
    render_source_tabs(frame, header_chunks[0], app);

    // Render stats on the right
    let (total, apt, snap, flatpak, appimage) = app.get_stats();

    let mut spans = vec![Span::styled(
        format!(" {} pkgs", total),
        theme.primary_bold(),
    )];

    // Show counts by source (compact)
    spans.push(Span::styled(
        format!(" A:{} S:{} F:{} I:{}", apt, snap, flatpak, appimage),
        theme.muted_style(),
    ));

    // Show scanning status
    if app.is_scanning() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(app.get_scan_status(), theme.success_style()));
    }

    let stats = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style()),
        )
        .style(theme.base_style());

    frame.render_widget(stats, header_chunks[1]);
}

/// Render source filter tabs
fn render_source_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let theme = get_theme();

    let tab_titles: Vec<&str> = vec![
        SourceTab::All.label(),
        SourceTab::Apt.label(),
        SourceTab::Snap.label(),
        SourceTab::Flatpak.label(),
        SourceTab::AppImage.label(),
    ];

    let selected_index = match app.source_tab {
        SourceTab::All => 0,
        SourceTab::Apt => 1,
        SourceTab::Snap => 2,
        SourceTab::Flatpak => 3,
        SourceTab::AppImage => 4,
    };

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style()),
        )
        .select(selected_index)
        .style(theme.muted_style())
        .highlight_style(theme.primary_bold())
        .divider("|");

    frame.render_widget(tabs, area);
}

/// Render the package table
fn render_table(frame: &mut Frame, area: Rect, app: &App) {
    let theme = get_theme();

    let header_cells = ["", "Name", "Source", "Type", "Size"]
        .iter()
        .map(|h| Cell::from(*h).style(theme.header_style()));

    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .filtered_packages
        .iter()
        .enumerate()
        .map(|(i, &pkg_idx)| {
            let pkg = &app.packages[pkg_idx];
            let is_selected = i == app.selected;

            let style = if is_selected {
                theme.selection_style()
            } else {
                theme.base_style()
            };

            let selector = if is_selected { ">" } else { " " };

            Row::new(vec![
                Cell::from(selector),
                Cell::from(pkg.name.clone()).style(Style::default().fg(theme.secondary())),
                Cell::from(pkg.source.to_string())
                    .style(Style::default().fg(theme.source_color(&pkg.source))),
                Cell::from(pkg.app_type.to_string())
                    .style(Style::default().fg(theme.app_type_color(&pkg.app_type))),
                Cell::from(pkg.size_human()).style(theme.primary_style()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Percentage(45),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(
                    " Packages ({}/{}) - Sort: {} - Filter: {} ",
                    app.filtered_packages.len(),
                    app.packages.len(),
                    app.sort_criteria.label(),
                    app.app_type_filter.label()
                ))
                .title_style(theme.title_style())
                .border_style(theme.border_style()),
        )
        .style(theme.base_style())
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    state.select(Some(app.selected));
    frame.render_stateful_widget(table, area, &mut state);
}

/// Render the footer with search input and help text
fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let theme = get_theme();

    // Split footer into search box (left) and help (right)
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(30), // Search box
            Constraint::Min(20),    // Help text
        ])
        .split(area);

    // Search box on the left - minimal placeholder style
    let search_text = if app.search_query.is_empty() {
        " Search...".to_string()
    } else {
        format!(" {}_", app.search_query)
    };

    let search_style = if !app.search_query.is_empty() {
        theme.primary_style()
    } else {
        theme.muted_style()
    };

    let search_box = Paragraph::new(search_text).style(search_style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if !app.search_query.is_empty() {
                theme.border_focused_style()
            } else {
                theme.muted_style()
            }),
    );

    frame.render_widget(search_box, footer_chunks[0]);

    // Help text on the right
    let help_text = " [Tab] Source | [d] Del | [s] Sort | [q] Quit ";

    let footer = Paragraph::new(help_text).style(theme.muted_style()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.muted_style()),
    );

    frame.render_widget(footer, footer_chunks[1]);
}

/// Render update selection view (full-screen version - deprecated)
#[allow(dead_code)]
pub fn render_update_select(frame: &mut Frame, app: &App) {
    let theme = get_theme();

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
    .style(theme.warning_style().add_modifier(Modifier::BOLD))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.warning_style()),
    );

    frame.render_widget(header, chunks[0]);

    // Table of updateable packages
    let header_cells = ["", "Sel", "Name", "Source", "Current", "New Version"]
        .iter()
        .map(|h| Cell::from(*h).style(theme.header_style()));

    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .update_selection
        .iter()
        .enumerate()
        .map(|(i, &pkg_idx)| {
            let pkg = &app.packages[pkg_idx];
            let is_selected = i == app.selected;

            let style = if is_selected {
                theme.selection_style()
            } else {
                theme.base_style()
            };

            let selector = if is_selected { ">" } else { " " };
            let check = if pkg.selected { "[x]" } else { "[ ]" };

            Row::new(vec![
                Cell::from(selector),
                Cell::from(check).style(if pkg.selected {
                    theme.success_style()
                } else {
                    Style::default()
                }),
                Cell::from(pkg.name.clone()).style(Style::default().fg(theme.secondary())),
                Cell::from(pkg.source.to_string())
                    .style(Style::default().fg(theme.source_color(&pkg.source))),
                Cell::from(pkg.version.clone()),
                Cell::from(pkg.update_version.clone().unwrap_or_default())
                    .style(theme.success_style()),
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Updates Available ")
                .title_style(theme.title_style())
                .border_style(theme.border_style()),
        )
        .style(theme.base_style());

    frame.render_widget(table, chunks[1]);

    // Footer
    let footer = Paragraph::new(" [Space] Toggle | [a] Select All | [n] Select None | [Enter] Update Selected | [Esc] Cancel ")
        .style(theme.muted_style())
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(theme.muted_style()));

    frame.render_widget(footer, chunks[2]);
}

/// Render main view within a specific area (for floating window)
pub fn render_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();

    // Fill background
    let bg_block = Block::default().style(theme.base_style());
    frame.render_widget(bg_block, area);

    // Split area into header, table, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, chunks[0], app);
    render_table(frame, chunks[1], app);
    render_footer(frame, chunks[2], app);
}

/// Render update selection view within a specific area
pub fn render_update_select_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    let update_count = app.get_update_count();
    let selected_count = app.packages.iter().filter(|p| p.selected).count();

    let header = Paragraph::new(format!(
        " Select packages ({} / {})",
        selected_count, update_count
    ))
    .style(theme.warning_style().add_modifier(Modifier::BOLD))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.warning_style()),
    );

    frame.render_widget(header, chunks[0]);

    // Table
    let header_cells = ["", "Sel", "Name", "Source", "Current", "New"]
        .iter()
        .map(|h| Cell::from(*h).style(theme.header_style()));

    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = app
        .update_selection
        .iter()
        .enumerate()
        .map(|(i, &pkg_idx)| {
            let pkg = &app.packages[pkg_idx];
            let is_selected = i == app.selected;

            let style = if is_selected {
                theme.selection_style()
            } else {
                theme.base_style()
            };

            let selector = if is_selected { ">" } else { " " };
            let check = if pkg.selected { "[x]" } else { "[ ]" };

            Row::new(vec![
                Cell::from(selector),
                Cell::from(check).style(if pkg.selected {
                    theme.success_style()
                } else {
                    Style::default()
                }),
                Cell::from(pkg.name.clone()).style(Style::default().fg(theme.secondary())),
                Cell::from(pkg.source.to_string())
                    .style(Style::default().fg(theme.source_color(&pkg.source))),
                Cell::from(pkg.version.clone()),
                Cell::from(pkg.update_version.clone().unwrap_or_default())
                    .style(theme.success_style()),
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style()),
        )
        .style(theme.base_style());

    frame.render_widget(table, chunks[1]);

    // Footer
    let footer =
        Paragraph::new(" [Space] Toggle | [a] All | [n] None | [Enter] Update | [Esc] Cancel ")
            .style(theme.muted_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.muted_style()),
            );

    frame.render_widget(footer, chunks[2]);
}
