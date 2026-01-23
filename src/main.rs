//! Scope - Linux Package Manager TUI
//!
//! A terminal user interface for managing packages across multiple package managers
//! including APT, Snap, Flatpak, and AppImages.

mod app;
mod package;
mod scanner;
pub mod theme;
mod ui;

use anyhow::Result;
use app::{App, ConfirmAction, SidebarSection, View};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};
use std::io::{self, Write};
use std::time::Duration;

// Configuration for the floating window
const WINDOW_WIDTH: u16 = 100;
const WINDOW_HEIGHT: u16 = 35;

#[tokio::main]
async fn main() -> Result<()> {
    // Resize terminal window to our preferred size
    // Using ANSI escape sequence: ESC[8;height;widtht
    print!("\x1b[8;{};{}t", WINDOW_HEIGHT, WINDOW_WIDTH);
    io::stdout().flush()?;
    
    // Small delay to let terminal resize
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    // Setup terminal with alternate screen
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Run the app
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

/// Calculate the window area in top-left corner
fn calculate_window_area(terminal_size: Rect) -> Rect {
    // Position in top-left corner
    let x = 0;
    let y = 0;

    Rect::new(
        x,
        y,
        WINDOW_WIDTH.min(terminal_size.width),
        WINDOW_HEIGHT.min(terminal_size.height),
    )
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    // Start streaming scan
    let mut scan_rx = scanner::scan_all_streaming();

    loop {
        // Draw UI within window area
        terminal.draw(|f| {
            let window_area = calculate_window_area(f.area());
            ui::render_in_area(f, app, window_area);
        })?;

        // Check for scan results (non-blocking)
        while let Ok(msg) = scan_rx.try_recv() {
            match msg {
                scanner::ScanMessage::Packages(packages) => {
                    app.add_packages(packages);
                }
                scanner::ScanMessage::Started(source) => {
                    app.scanner_started(source);
                }
                scanner::ScanMessage::Completed(source) => {
                    app.scanner_completed(source);
                }
                scanner::ScanMessage::Done => {
                    app.scanning_done();
                }
            }
        }

        // Handle events with timeout for animation
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match app.view {
                    View::Main => handle_main_input(app, key.code, key.modifiers).await?,
                    View::Details => handle_details_input(app, key.code).await?,
                    View::Confirm => handle_confirm_input(app, key.code, terminal).await?,
                    View::UpdateSelect => {
                        handle_update_select_input(app, key.code, terminal).await?
                    }
                    View::Loading => {
                        // Allow quitting during loading
                        if key.code == KeyCode::Char('q') {
                            app.should_quit = true;
                        }
                    }
                    View::Error => handle_error_input(app, key.code),
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_main_input(app: &mut App, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
    // Handle sidebar navigation when sidebar is focused
    if app.sidebar_focused {
        match key {
            KeyCode::Esc | KeyCode::Right | KeyCode::Char('l') => {
                // Exit sidebar focus, go to main content
                app.sidebar_focused = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.prev_sidebar_section();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.next_sidebar_section();
            }
            KeyCode::Enter => {
                // Select current section and exit sidebar focus
                let section = app.sidebar_section;
                app.sidebar_focused = false;
                
                // Handle section-specific actions
                match section {
                    SidebarSection::Delete => {
                        // Delete section - just shows the main package view
                    }
                    SidebarSection::Update => {
                        // Check for updates first, then show selection
                        app.check_updates().await?;
                        if app.get_update_count() > 0 {
                            app.show_update_selection();
                        }
                    }
                    SidebarSection::Install | SidebarSection::Clean => {
                        // Install and Clean - placeholder for future features
                    }
                }
            }
            KeyCode::Char('q') => {
                app.should_quit = true;
            }
            _ => {}
        }
        return Ok(());
    }

    // Normal main view input handling
    match key {
        KeyCode::Esc => {
            // Esc clears search if there's a query, otherwise quits
            if !app.search_query.is_empty() {
                app.clear_search();
            } else {
                app.should_quit = true;
            }
        }
        KeyCode::Char('q') if app.search_query.is_empty() => {
            // Only quit if not searching
            app.should_quit = true;
        }
        // Ctrl+b or Left arrow to focus sidebar
        KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.sidebar_focused = true;
        }
        KeyCode::Left | KeyCode::Char('h') if app.search_query.is_empty() => {
            app.sidebar_focused = true;
        }
        KeyCode::Up | KeyCode::Char('k') if app.search_query.is_empty() => {
            app.select_previous();
        }
        KeyCode::Up => {
            app.select_previous();
        }
        KeyCode::Down | KeyCode::Char('j') if app.search_query.is_empty() => {
            app.select_next();
        }
        KeyCode::Down => {
            app.select_next();
        }
        KeyCode::Home => {
            app.select_first();
        }
        KeyCode::Char('g') if app.search_query.is_empty() => {
            app.select_first();
        }
        KeyCode::End => {
            app.select_last();
        }
        KeyCode::Char('G') if app.search_query.is_empty() => {
            app.select_last();
        }
        KeyCode::PageUp => {
            app.page_up(10);
        }
        KeyCode::PageDown => {
            app.page_down(10);
        }
        KeyCode::Enter => {
            app.show_details();
        }
        KeyCode::Tab => {
            // Switch source tabs
            if modifiers.contains(KeyModifiers::SHIFT) {
                app.prev_tab();
            } else {
                app.next_tab();
            }
        }
        KeyCode::BackTab => {
            // Shift+Tab goes to previous tab
            app.prev_tab();
        }
        KeyCode::Char('f') if app.search_query.is_empty() => {
            app.toggle_filter();
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_search();
        }
        KeyCode::Char('r') if app.search_query.is_empty() => {
            // Refresh/rescan
            app.load_packages().await?;
        }
        KeyCode::Backspace => {
            // Delete last character from search
            app.search_backspace();
        }
        KeyCode::Char(c) => {
            // Always-on search: type to filter
            app.search_input(c);
        }
        _ => {}
    }
    Ok(())
}

async fn handle_details_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.hide_details();
        }
        KeyCode::Char('d') => {
            app.request_uninstall();
        }
        KeyCode::Char('u') => {
            app.request_update();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.details_scroll = app.details_scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.details_scroll = app.details_scroll.saturating_add(1);
        }
        _ => {}
    }
    Ok(())
}

async fn handle_confirm_input(
    app: &mut App,
    key: KeyCode,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            match app.confirm_action {
                Some(ConfirmAction::Uninstall) => {
                    // Extract needed data before borrowing
                    let pkg_info = app
                        .selected_package()
                        .map(|pkg| (pkg.name.clone(), pkg.source, pkg.install_path.clone(), app.selected));

                    if let Some((name, source, install_path, selected_idx)) = pkg_info {
                        let scanner = scanner::get_scanner(source);
                        app.loading_message = format!("Uninstalling {}...", name);
                        app.view = View::Loading;

                        // Create a temporary package for uninstall
                        let mut temp_pkg = crate::package::Package::new(name.clone(), source);
                        temp_pkg.install_path = install_path;

                        // Leave alternate screen for pkexec to show its UI
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

                        // Perform uninstall
                        let result = scanner.uninstall(&temp_pkg).await;

                        // Re-enter alternate screen and restore raw mode
                        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                        enable_raw_mode()?;
                        terminal.clear()?;

                        if let Err(e) = result {
                            app.error_message = format!("Uninstall failed: {}", e);
                            app.view = View::Error;
                        } else {
                            // Remove from package list
                            if let Some(&idx) = app.filtered_packages.get(selected_idx) {
                                app.packages.remove(idx);
                                // Clear search to show all packages
                                app.clear_search();
                            }
                            app.view = View::Main;
                        }
                    }
                }
                Some(ConfirmAction::Update) => {
                    // Extract needed data before borrowing
                    let pkg_info = app
                        .selected_package()
                        .map(|pkg| (pkg.name.clone(), pkg.source, pkg.install_path.clone()));

                    if let Some((name, source, install_path)) = pkg_info {
                        let scanner = scanner::get_scanner(source);
                        app.loading_message = format!("Updating {}...", name);
                        app.view = View::Loading;

                        // Create a temporary package for update
                        let mut temp_pkg = crate::package::Package::new(name.clone(), source);
                        temp_pkg.install_path = install_path;

                        // Leave alternate screen for pkexec to show its UI
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

                        // Perform update
                        let result = scanner.update(&temp_pkg).await;

                        // Re-enter alternate screen and restore raw mode
                        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                        enable_raw_mode()?;
                        terminal.clear()?;

                        if let Err(e) = result {
                            app.error_message = format!("Update failed: {}", e);
                            app.view = View::Error;
                        } else {
                            // Refresh package info
                            app.load_packages().await?;
                        }
                    }
                }
                None => {}
            }
            app.confirm_action = None;
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.cancel_confirm();
        }
        _ => {}
    }
    Ok(())
}

async fn handle_update_select_input(
    app: &mut App,
    key: KeyCode,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => {
            // Clear selections and return to main
            for pkg in &mut app.packages {
                pkg.selected = false;
            }
            app.view = View::Main;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected > 0 {
                app.selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected < app.update_selection.len().saturating_sub(1) {
                app.selected += 1;
            }
        }
        KeyCode::Char(' ') => {
            // Toggle selection
            if let Some(&idx) = app.update_selection.get(app.selected) {
                app.packages[idx].selected = !app.packages[idx].selected;
            }
        }
        KeyCode::Char('a') => {
            // Select all
            for &idx in &app.update_selection {
                app.packages[idx].selected = true;
            }
        }
        KeyCode::Char('n') => {
            // Select none
            for &idx in &app.update_selection {
                app.packages[idx].selected = false;
            }
        }
        KeyCode::Enter => {
            // Perform updates on selected packages
            let selected_indices: Vec<usize> = app
                .update_selection
                .iter()
                .filter(|&&idx| app.packages[idx].selected)
                .copied()
                .collect();

            // Leave alternate screen for pkexec to show its UI
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

            for idx in selected_indices {
                let pkg = &app.packages[idx];
                let scanner = scanner::get_scanner(pkg.source);
                if let Err(e) = scanner.update(pkg).await {
                    // Store error but continue with other updates
                    app.error_message = format!("Failed to update {}: {}", pkg.name, e);
                }
            }

            // Re-enter alternate screen and restore raw mode
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            enable_raw_mode()?;
            terminal.clear()?;

            // Refresh after updates
            app.load_packages().await?;
        }
        _ => {}
    }
    Ok(())
}

fn handle_error_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter => {
            app.error_message.clear();
            app.view = View::Main;
        }
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        _ => {}
    }
}
