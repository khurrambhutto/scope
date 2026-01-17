//! Scope - Linux Package Manager TUI
//!
//! A terminal user interface for managing packages across multiple package managers
//! including APT, Snap, Flatpak, and AppImages.

mod app;
mod package;
mod scanner;
mod ui;

use app::{App, ConfirmAction, View};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Run initial scan in background
    let scan_result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = scan_result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    // Initial render
    terminal.draw(|f| ui::render(f, app))?;

    // Load packages
    app.load_packages().await?;

    loop {
        // Draw UI
        terminal.draw(|f| ui::render(f, app))?;

        // Handle events with timeout for animation
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app.view {
                    View::Main => handle_main_input(app, key.code, key.modifiers).await?,
                    View::Search => handle_search_input(app, key.code, key.modifiers),
                    View::Details => handle_details_input(app, key.code).await?,
                    View::Confirm => handle_confirm_input(app, key.code).await?,
                    View::UpdateSelect => handle_update_select_input(app, key.code).await?,
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
    match key {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_previous();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next();
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.select_first();
        }
        KeyCode::End | KeyCode::Char('G') => {
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
        KeyCode::Char('/') => {
            app.enter_search();
        }
        KeyCode::Char('s') => {
            app.toggle_sort();
        }
        KeyCode::Char('f') => {
            app.toggle_filter();
        }
        KeyCode::Char('d') => {
            app.request_uninstall();
        }
        KeyCode::Char('c') => {
            // Check for updates
            app.check_updates().await?;
        }
        KeyCode::Char('U') => {
            // Show update selection if there are updates
            if app.get_update_count() > 0 {
                app.show_update_selection();
            }
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_search();
        }
        KeyCode::Char('r') => {
            // Refresh/rescan
            app.load_packages().await?;
        }
        _ => {}
    }
    Ok(())
}

fn handle_search_input(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Esc => {
            app.exit_search();
        }
        KeyCode::Enter => {
            app.exit_search();
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.apply_filters();
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_search();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.apply_filters();
        }
        _ => {}
    }
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

async fn handle_confirm_input(app: &mut App, key: KeyCode) -> Result<()> {
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            match app.confirm_action {
                Some(ConfirmAction::Uninstall) => {
                    // Extract needed data before borrowing
                    let pkg_info = app.selected_package().map(|pkg| {
                        (pkg.name.clone(), pkg.source, app.selected)
                    });
                    
                    if let Some((name, source, selected_idx)) = pkg_info {
                        let scanner = scanner::get_scanner(source);
                        app.loading_message = format!("Uninstalling {}...", name);
                        app.view = View::Loading;
                        
                        // Create a temporary package for uninstall
                        let temp_pkg = crate::package::Package::new(name.clone(), source);
                        
                        // Perform uninstall
                        if let Err(e) = scanner.uninstall(&temp_pkg).await {
                            app.error_message = format!("Uninstall failed: {}", e);
                            app.view = View::Error;
                        } else {
                            // Remove from package list
                            if let Some(&idx) = app.filtered_packages.get(selected_idx) {
                                app.packages.remove(idx);
                                app.apply_filters();
                            }
                            app.view = View::Main;
                        }
                    }
                }
                Some(ConfirmAction::Update) => {
                    // Extract needed data before borrowing
                    let pkg_info = app.selected_package().map(|pkg| {
                        (pkg.name.clone(), pkg.source, pkg.install_path.clone())
                    });
                    
                    if let Some((name, source, install_path)) = pkg_info {
                        let scanner = scanner::get_scanner(source);
                        app.loading_message = format!("Updating {}...", name);
                        app.view = View::Loading;
                        
                        // Create a temporary package for update
                        let mut temp_pkg = crate::package::Package::new(name.clone(), source);
                        temp_pkg.install_path = install_path;
                        
                        if let Err(e) = scanner.update(&temp_pkg).await {
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

async fn handle_update_select_input(app: &mut App, key: KeyCode) -> Result<()> {
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

            for idx in selected_indices {
                let pkg = &app.packages[idx];
                app.loading_message = format!("Updating {}...", pkg.name);
                
                let scanner = scanner::get_scanner(pkg.source);
                if let Err(e) = scanner.update(pkg).await {
                    app.error_message = format!("Failed to update {}: {}", pkg.name, e);
                    // Continue with other updates
                }
            }

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
