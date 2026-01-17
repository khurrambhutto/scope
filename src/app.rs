//! Application state management

use crate::package::{sort_packages, AppTypeFilter, Package, SortCriteria};
use crate::scanner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Main,
    Details,
    Search,
    Confirm,
    UpdateSelect,
    Loading,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Uninstall,
    Update,
}

pub struct App {
    /// All packages from all sources
    pub packages: Vec<Package>,
    /// Filtered packages (based on search/filter)
    pub filtered_packages: Vec<usize>, // indices into packages
    /// Currently selected index in filtered list
    pub selected: usize,
    /// Current view
    pub view: View,
    /// Search query
    pub search_query: String,
    /// Sort criteria
    pub sort_criteria: SortCriteria,
    /// App type filter
    pub app_type_filter: AppTypeFilter,
    /// Confirmation action pending
    pub confirm_action: Option<ConfirmAction>,
    /// Loading message
    pub loading_message: String,
    /// Error message
    pub error_message: String,
    /// Whether we're checking for updates
    pub checking_updates: bool,
    /// Packages selected for batch update
    pub update_selection: Vec<usize>,
    /// Scroll offset for details view
    pub details_scroll: u16,
    /// Application should quit
    pub should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            packages: Vec::new(),
            filtered_packages: Vec::new(),
            selected: 0,
            view: View::Loading,
            search_query: String::new(),
            sort_criteria: SortCriteria::default(), // Size descending
            app_type_filter: AppTypeFilter::default(),
            confirm_action: None,
            loading_message: "Scanning installed packages...".to_string(),
            error_message: String::new(),
            checking_updates: false,
            update_selection: Vec::new(),
            details_scroll: 0,
            should_quit: false,
        }
    }

    /// Scan all package managers and load packages
    pub async fn load_packages(&mut self) -> anyhow::Result<()> {
        self.view = View::Loading;
        self.loading_message = "Scanning installed packages...".to_string();

        match scanner::scan_all().await {
            Ok(packages) => {
                self.packages = packages;
                self.sort_packages();
                self.apply_filters();
                self.view = View::Main;
                Ok(())
            }
            Err(e) => {
                self.error_message = format!("Failed to scan packages: {}", e);
                self.view = View::Error;
                Err(e)
            }
        }
    }

    /// Check for updates on all packages
    pub async fn check_updates(&mut self) -> anyhow::Result<()> {
        self.checking_updates = true;
        self.loading_message = "Checking for updates...".to_string();
        let prev_view = self.view;
        self.view = View::Loading;

        let result = scanner::check_all_updates(&mut self.packages).await;
        
        self.checking_updates = false;
        self.view = prev_view;
        self.apply_filters();
        
        result
    }

    /// Sort packages based on current criteria
    pub fn sort_packages(&mut self) {
        sort_packages(&mut self.packages, self.sort_criteria);
    }

    /// Apply search and filter to get filtered_packages
    pub fn apply_filters(&mut self) {
        self.filtered_packages = self
            .packages
            .iter()
            .enumerate()
            .filter(|(_, pkg)| {
                // Apply search filter
                let matches_search = self.search_query.is_empty() 
                    || pkg.matches_search(&self.search_query);
                
                // Apply app type filter
                let matches_type = self.app_type_filter.matches(pkg.app_type);
                
                matches_search && matches_type
            })
            .map(|(i, _)| i)
            .collect();

        // Reset selection if out of bounds
        if self.selected >= self.filtered_packages.len() {
            self.selected = self.filtered_packages.len().saturating_sub(1);
        }
    }

    /// Get currently selected package (if any)
    pub fn selected_package(&self) -> Option<&Package> {
        self.filtered_packages
            .get(self.selected)
            .and_then(|&idx| self.packages.get(idx))
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected < self.filtered_packages.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Move selection to top
    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    /// Move selection to bottom
    pub fn select_last(&mut self) {
        self.selected = self.filtered_packages.len().saturating_sub(1);
    }

    /// Page up
    pub fn page_up(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size);
    }

    /// Page down
    pub fn page_down(&mut self, page_size: usize) {
        self.selected = (self.selected + page_size)
            .min(self.filtered_packages.len().saturating_sub(1));
    }

    /// Toggle sort criteria
    pub fn toggle_sort(&mut self) {
        self.sort_criteria = self.sort_criteria.next();
        self.sort_packages();
        self.apply_filters();
    }

    /// Toggle app type filter
    pub fn toggle_filter(&mut self) {
        self.app_type_filter = self.app_type_filter.next();
        self.apply_filters();
    }

    /// Enter search mode
    pub fn enter_search(&mut self) {
        self.view = View::Search;
    }

    /// Exit search mode
    pub fn exit_search(&mut self) {
        self.view = View::Main;
        self.apply_filters();
    }

    /// Clear search
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.apply_filters();
    }

    /// Show details for selected package
    pub fn show_details(&mut self) {
        if self.selected_package().is_some() {
            self.details_scroll = 0;
            self.view = View::Details;
        }
    }

    /// Hide details
    pub fn hide_details(&mut self) {
        self.view = View::Main;
    }

    /// Request uninstall confirmation
    pub fn request_uninstall(&mut self) {
        if self.selected_package().is_some() {
            self.confirm_action = Some(ConfirmAction::Uninstall);
            self.view = View::Confirm;
        }
    }

    /// Request update for selected package
    pub fn request_update(&mut self) {
        if let Some(pkg) = self.selected_package() {
            if pkg.has_update == Some(true) {
                self.confirm_action = Some(ConfirmAction::Update);
                self.view = View::Confirm;
            }
        }
    }

    /// Cancel confirmation
    pub fn cancel_confirm(&mut self) {
        self.confirm_action = None;
        self.view = View::Main;
    }

    /// Show update selection view
    pub fn show_update_selection(&mut self) {
        // Collect indices of packages with updates
        self.update_selection = self
            .packages
            .iter()
            .enumerate()
            .filter(|(_, pkg)| pkg.has_update == Some(true))
            .map(|(i, _)| i)
            .collect();

        if !self.update_selection.is_empty() {
            // Mark all as selected by default
            for &idx in &self.update_selection {
                self.packages[idx].selected = true;
            }
            self.view = View::UpdateSelect;
        }
    }

    /// Get total count stats
    pub fn get_stats(&self) -> (usize, usize, usize, usize, usize) {
        let mut apt = 0;
        let mut snap = 0;
        let mut flatpak = 0;
        let mut appimage = 0;
        
        for pkg in &self.packages {
            match pkg.source {
                crate::package::PackageSource::Apt | crate::package::PackageSource::DebFile => apt += 1,
                crate::package::PackageSource::Snap => snap += 1,
                crate::package::PackageSource::Flatpak => flatpak += 1,
                crate::package::PackageSource::AppImage => appimage += 1,
            }
        }
        
        (self.packages.len(), apt, snap, flatpak, appimage)
    }

    /// Get count of packages with updates
    pub fn get_update_count(&self) -> usize {
        self.packages.iter().filter(|p| p.has_update == Some(true)).count()
    }
}
