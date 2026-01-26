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

/// Render header with source tabs
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    // Render source tabs (full width)
    render_source_tabs(frame, area, app);
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
                    " Packages ({}/{}) ",
                    app.filtered_packages.len(),
                    app.packages.len(),
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

    // Help text on the right - minimal, clean
    let help_text = " [Enter] Details | [Tab] Source | [q] Quit ";

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

/// Render update by source selection view (full-screen in content area)
pub fn render_update_by_source_in_area(frame: &mut Frame, app: &App, area: Rect) {
    use crate::package::PackageSource;
    
    let theme = get_theme();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
        ])
        .split(area);

    // Header
    let header_text = if app.updates_checked {
        format!(" Update Packages ({} available)", app.get_total_update_count())
    } else {
        " Update Packages".to_string()
    };
    
    let header = Paragraph::new(header_text)
        .style(theme.primary_bold())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.primary_style()),
        );

    frame.render_widget(header, chunks[0]);

    // Source list - build the lines for centered display
    let sources = [
        (PackageSource::Apt, "APT"),
        (PackageSource::Snap, "Snap"),
        (PackageSource::Flatpak, "Flatpak"),
    ];

    let mut lines: Vec<Line> = Vec::new();

    for (i, (source, label)) in sources.iter().enumerate() {
        let is_selected = app.selected_update_source == i;
        let count = app
            .update_source_counts
            .as_ref()
            .and_then(|c| c.get(source))
            .copied()
            .unwrap_or(0);

        let count_str = if app.updates_checked {
            format!("({})", count)
        } else {
            "(?)".to_string()
        };

        let style = if is_selected {
            theme.selection_style()
        } else {
            theme.base_style()
        };

        let selector = if is_selected { ">" } else { " " };

        lines.push(Line::styled(
            format!("   {} {:<12} {:>6}", selector, label, count_str),
            style,
        ));
    }

    // Separator
    lines.push(Line::from("     ─────────────────"));

    // All option
    let is_all_selected = app.selected_update_source == 3;
    let total_count = app.get_total_update_count();
    let total_str = if app.updates_checked {
        format!("({})", total_count)
    } else {
        "(?)".to_string()
    };

    let all_style = if is_all_selected {
        theme.selection_style()
    } else {
        theme.base_style()
    };
    let all_selector = if is_all_selected { ">" } else { " " };

    lines.push(Line::styled(
        format!("   {} {:<12} {:>6}", all_selector, "All", total_str),
        all_style,
    ));

    // Calculate centered area for the source list
    let content_height = 5u16; // 3 sources + 1 separator + 1 all
    let content_width = 30u16;
    
    let content_area = chunks[1];
    let vertical_padding = content_area.height.saturating_sub(content_height + 4) / 2; // +4 for instructions
    let horizontal_padding = content_area.width.saturating_sub(content_width) / 2;
    
    let centered_area = Rect {
        x: content_area.x + horizontal_padding,
        y: content_area.y + vertical_padding,
        width: content_width.min(content_area.width),
        height: content_height.min(content_area.height),
    };

    // Render background block for the full content area
    let bg_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Select Source ")
        .title_style(theme.title_style())
        .border_style(theme.border_style())
        .style(theme.base_style());
    
    frame.render_widget(bg_block, chunks[1]);

    // Render the centered source list
    let content = Paragraph::new(lines)
        .style(theme.base_style())
        .alignment(ratatui::layout::Alignment::Left);

    frame.render_widget(content, centered_area);

    // Instructions at bottom-right inside the content area
    let instructions = vec![
        Line::from(vec![
            Span::styled("[c]", theme.primary_style()),
            Span::styled(" Check for updates", theme.muted_style()),
        ]),
        Line::from(vec![
            Span::styled("[Enter]", theme.primary_style()),
            Span::styled(" Update", theme.muted_style()),
        ]),
        Line::from(vec![
            Span::styled("[↑↓]", theme.primary_style()),
            Span::styled(" Navigate", theme.muted_style()),
        ]),
        Line::from(vec![
            Span::styled("[Esc]", theme.primary_style()),
            Span::styled(" Back", theme.muted_style()),
        ]),
    ];

    let instructions_width = 22u16;
    let instructions_height = 4u16;
    
    let instructions_area = Rect {
        x: content_area.x + content_area.width.saturating_sub(instructions_width + 3),
        y: content_area.y + content_area.height.saturating_sub(instructions_height + 2),
        width: instructions_width,
        height: instructions_height,
    };

    let instructions_widget = Paragraph::new(instructions)
        .style(theme.base_style())
        .alignment(ratatui::layout::Alignment::Left);

    frame.render_widget(instructions_widget, instructions_area);
}

/// Render update progress view (full-screen in content area)
pub fn render_update_progress_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let progress = &app.update_progress;
    let source_name = progress
        .source
        .map(|s| s.to_string())
        .unwrap_or_else(|| "All".to_string());

    // Header
    let header = Paragraph::new(format!(" Updating {} Packages ", source_name))
        .style(theme.warning_style().add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.warning_style()),
        );

    frame.render_widget(header, chunks[0]);

    // Progress content
    let filled = if progress.total > 0 {
        (progress.current * 30) / progress.total
    } else {
        0
    };
    let empty = 30 - filled;
    let progress_bar = format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(empty)
    );

    let progress_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Current: ", theme.label_style()),
            Span::styled(&progress.current_package, theme.primary_bold()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Progress: ", theme.label_style()),
            Span::styled(progress_bar, theme.primary_style()),
            Span::raw(format!(" {}/{}", progress.current, progress.total)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Completed: ", theme.label_style()),
            Span::styled(format!("{}", progress.success_count), theme.success_style()),
        ]),
        if !progress.errors.is_empty() {
            Line::from(vec![
                Span::styled("  Failed: ", theme.label_style()),
                Span::styled(format!("{}", progress.errors.len()), theme.error_style()),
            ])
        } else {
            Line::from("")
        },
    ];

    let content = Paragraph::new(progress_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style()),
        )
        .style(theme.base_style());

    frame.render_widget(content, chunks[1]);

    // Footer
    let footer = Paragraph::new(" [Esc] Cancel ")
        .style(theme.muted_style())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.muted_style()),
        );

    frame.render_widget(footer, chunks[2]);
}

/// Render cancel confirmation view (full-screen in content area)
pub fn render_cancel_confirm_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let progress = &app.update_progress;

    // Header
    let header = Paragraph::new(" Cancel Update? ")
        .style(theme.warning_style().add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.warning_style()),
        );

    frame.render_widget(header, chunks[0]);

    // Content
    let content_text = vec![
        Line::from(""),
        Line::from("  Stop updating packages?"),
        Line::from(""),
        Line::from(format!(
            "  {}/{} packages completed.",
            progress.success_count, progress.total
        )),
        Line::from(""),
    ];

    let content = Paragraph::new(content_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style()),
        )
        .style(theme.base_style());

    frame.render_widget(content, chunks[1]);

    // Footer
    let footer = Paragraph::new(" [y] Yes, stop | [n] No, continue ")
        .style(theme.muted_style())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.muted_style()),
        );

    frame.render_widget(footer, chunks[2]);
}

/// Render update summary view (full-screen in content area)
pub fn render_update_summary_in_area(frame: &mut Frame, app: &App, area: Rect) {
    let theme = get_theme();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let progress = &app.update_progress;
    let has_errors = !progress.errors.is_empty();
    let skipped = progress.total.saturating_sub(progress.success_count + progress.errors.len());

    // Header
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

    let header = Paragraph::new(title)
        .style(border_style.add_modifier(Modifier::BOLD))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style),
        );

    frame.render_widget(header, chunks[0]);

    // Content
    let mut content_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ✓ Updated: ", theme.success_style()),
            Span::raw(format!("{} packages", progress.success_count)),
        ]),
    ];

    if has_errors {
        content_lines.push(Line::from(vec![
            Span::styled("  ✗ Failed:  ", theme.error_style()),
            Span::raw(format!("{} packages", progress.errors.len())),
        ]));
    }

    if progress.cancelled && skipped > 0 {
        content_lines.push(Line::from(vec![
            Span::styled("  ○ Skipped: ", theme.muted_style()),
            Span::raw(format!("{} packages", skipped)),
        ]));
    }

    if has_errors {
        content_lines.push(Line::from(""));
        content_lines.push(Line::from(Span::styled("  Errors:", theme.error_style())));
        for (name, err) in progress.errors.iter().take(5) {
            let error_msg = if err.len() > 40 {
                format!("{}...", &err[..37])
            } else {
                err.clone()
            };
            content_lines.push(Line::from(Span::styled(
                format!("    - {}: {}", name, error_msg),
                theme.muted_style(),
            )));
        }
        if progress.errors.len() > 5 {
            content_lines.push(Line::from(Span::styled(
                format!("    ... and {} more", progress.errors.len() - 5),
                theme.muted_style(),
            )));
        }
    }

    let content = Paragraph::new(content_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.border_style()),
        )
        .style(theme.base_style());

    frame.render_widget(content, chunks[1]);

    // Footer
    let footer = Paragraph::new(" [Enter] Continue | [q] Quit ")
        .style(theme.muted_style())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.muted_style()),
        );

    frame.render_widget(footer, chunks[2]);
}

