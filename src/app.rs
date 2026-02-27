//! Application state management

use crate::package::{sort_packages, AppTypeFilter, Package, PackageSource, SortCriteria};
use crate::scanner;
use std::collections::{HashMap, HashSet};

/// Progress tracking for batch updates
#[derive(Debug, Clone, Default)]
pub struct UpdateProgress {
    /// Source being updated (None = All)
    pub source: Option<PackageSource>,
    /// Current package index (0-based)
    pub current: usize,
    /// Total packages to update
    pub total: usize,
    /// Name of current package being updated
    pub current_package: String,
    /// Number of successful updates
    pub success_count: usize,
    /// List of errors (package_name, error_message)
    pub errors: Vec<(String, String)>,
    /// Whether update was cancelled
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Main,
    Details,
    Confirm,
    UpdateSelect,
    UpdateBySource,
    UpdateProgress,
    UpdateSummary,
    CancelConfirm,
    Loading,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Uninstall,
    Update,
}

/// Sidebar sections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarSection {
    #[default]
    Apps,
    Update,
    Install,
    Clean,
}

impl SidebarSection {
    pub fn next(self) -> Self {
        match self {
            SidebarSection::Apps => SidebarSection::Update,
            SidebarSection::Update => SidebarSection::Install,
            SidebarSection::Install => SidebarSection::Clean,
            SidebarSection::Clean => SidebarSection::Apps,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            SidebarSection::Apps => SidebarSection::Clean,
            SidebarSection::Update => SidebarSection::Apps,
            SidebarSection::Install => SidebarSection::Update,
            SidebarSection::Clean => SidebarSection::Install,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SidebarSection::Apps => "Apps",
            SidebarSection::Update => "Update",
            SidebarSection::Install => "Install",
            SidebarSection::Clean => "Clean",
        }
    }
}

/// Source filter tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceTab {
    #[default]
    All,
    Apt,
    Snap,
    Flatpak,
    AppImage,
}

impl SourceTab {
    pub fn next(self) -> Self {
        match self {
            SourceTab::All => SourceTab::Apt,
            SourceTab::Apt => SourceTab::Snap,
            SourceTab::Snap => SourceTab::Flatpak,
            SourceTab::Flatpak => SourceTab::AppImage,
            SourceTab::AppImage => SourceTab::All,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            SourceTab::All => SourceTab::AppImage,
            SourceTab::Apt => SourceTab::All,
            SourceTab::Snap => SourceTab::Apt,
            SourceTab::Flatpak => SourceTab::Snap,
            SourceTab::AppImage => SourceTab::Flatpak,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SourceTab::All => "All",
            SourceTab::Apt => "APT",
            SourceTab::Snap => "Snap",
            SourceTab::Flatpak => "Flatpak",
            SourceTab::AppImage => "AppImage",
        }
    }

    pub fn matches(&self, source: PackageSource) -> bool {
        match self {
            SourceTab::All => true,
            SourceTab::Apt => matches!(source, PackageSource::Apt | PackageSource::DebFile),
            SourceTab::Snap => source == PackageSource::Snap,
            SourceTab::Flatpak => source == PackageSource::Flatpak,
            SourceTab::AppImage => source == PackageSource::AppImage,
        }
    }
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
    /// Search query (always active)
    pub search_query: String,
    /// Sort criteria
    pub sort_criteria: SortCriteria,
    /// App type filter
    pub app_type_filter: AppTypeFilter,
    /// Source tab filter
    pub source_tab: SourceTab,
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
    /// Scanning status - which scanners are currently running
    pub scanning_sources: HashSet<PackageSource>,
    /// Whether initial scan is complete
    pub scan_complete: bool,
    /// Current sidebar section
    pub sidebar_section: SidebarSection,
    /// Whether sidebar is focused (for navigation)
    pub sidebar_focused: bool,
    /// Selected source in update-by-source view (0=APT, 1=Snap, 2=Flatpak, 3=All)
    pub selected_update_source: usize,
    /// Update counts per source (None = not checked yet)
    pub update_source_counts: Option<HashMap<PackageSource, usize>>,
    /// Current update progress
    pub update_progress: UpdateProgress,
    /// Whether updates have been checked
    pub updates_checked: bool,
    /// Toast message to display (slides in from right)
    pub toast_message: Option<String>,
    /// When the toast should disappear (timestamp in milliseconds)
    pub toast_expires_at: Option<u128>,
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
            view: View::Main, // Start with Main view, show packages as they load
            search_query: String::new(),
            sort_criteria: SortCriteria::default(), // Size descending
            app_type_filter: AppTypeFilter::default(),
            source_tab: SourceTab::default(),
            confirm_action: None,
            loading_message: "Scanning...".to_string(),
            error_message: String::new(),
            checking_updates: false,
            update_selection: Vec::new(),
            details_scroll: 0,
            should_quit: false,
            scanning_sources: HashSet::new(),
            scan_complete: false,
            sidebar_section: SidebarSection::default(),
            sidebar_focused: false,
            selected_update_source: 0,
            update_source_counts: None,
            update_progress: UpdateProgress::default(),
            updates_checked: false,
            toast_message: None,
            toast_expires_at: None,
        }
    }

    /// Show a toast notification that auto-dismisses after 3 seconds
    pub fn show_toast(&mut self, message: String) {
        self.toast_message = Some(message);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        self.toast_expires_at = Some(now + 3000); // 3 seconds
    }

    /// Check if toast should be dismissed
    pub fn check_toast_expiry(&mut self) {
        if let Some(expires_at) = self.toast_expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            if now >= expires_at {
                self.toast_message = None;
                self.toast_expires_at = None;
            }
        }
    }

    /// Show update by source selection view
    pub fn show_update_by_source(&mut self) {
        self.selected_update_source = 0;
        self.view = View::UpdateBySource;
    }

    /// Get update counts by source (APT, Snap, Flatpak)
    pub fn calculate_update_counts(&mut self) {
        let mut counts = HashMap::new();
        counts.insert(PackageSource::Apt, 0);
        counts.insert(PackageSource::Snap, 0);
        counts.insert(PackageSource::Flatpak, 0);

        for pkg in &self.packages {
            if pkg.has_update == Some(true) {
                match pkg.source {
                    PackageSource::Apt | PackageSource::DebFile => {
                        *counts.get_mut(&PackageSource::Apt).unwrap() += 1;
                    }
                    PackageSource::Snap => {
                        *counts.get_mut(&PackageSource::Snap).unwrap() += 1;
                    }
                    PackageSource::Flatpak => {
                        *counts.get_mut(&PackageSource::Flatpak).unwrap() += 1;
                    }
                    PackageSource::AppImage => {} // AppImages don't have central updates
                }
            }
        }

        self.update_source_counts = Some(counts);
        self.updates_checked = true;
    }

    /// Get total update count across all sources
    pub fn get_total_update_count(&self) -> usize {
        self.update_source_counts
            .as_ref()
            .map(|counts| counts.values().sum())
            .unwrap_or(0)
    }

    /// Get packages with updates for a specific source (None = all sources)
    pub fn get_packages_to_update(&self, source: Option<PackageSource>) -> Vec<usize> {
        self.packages
            .iter()
            .enumerate()
            .filter(|(_, pkg)| {
                if pkg.has_update != Some(true) {
                    return false;
                }
                match source {
                    None => true, // All sources
                    Some(s) => match s {
                        PackageSource::Apt => {
                            matches!(pkg.source, PackageSource::Apt | PackageSource::DebFile)
                        }
                        _ => pkg.source == s,
                    },
                }
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Reset update progress
    pub fn reset_update_progress(&mut self) {
        self.update_progress = UpdateProgress::default();
    }

    /// Add packages from a scanner (used during streaming load)
    pub fn add_packages(&mut self, mut new_packages: Vec<Package>) {
        self.packages.append(&mut new_packages);
        self.sort_packages();
        self.apply_filters();
    }

    /// Mark a scanner as started
    pub fn scanner_started(&mut self, source: PackageSource) {
        self.scanning_sources.insert(source);
    }

    /// Mark a scanner as completed
    pub fn scanner_completed(&mut self, source: PackageSource) {
        self.scanning_sources.remove(&source);
    }

    /// Mark all scanning as done
    pub fn scanning_done(&mut self) {
        self.scan_complete = true;
        self.scanning_sources.clear();
    }

    /// Check if we're still scanning
    pub fn is_scanning(&self) -> bool {
        !self.scan_complete || !self.scanning_sources.is_empty()
    }

    /// Get scanning status message
    pub fn get_scan_status(&self) -> String {
        if self.scan_complete {
            String::new()
        } else if self.scanning_sources.is_empty() {
            "Starting scan...".to_string()
        } else {
            let sources: Vec<String> = self
                .scanning_sources
                .iter()
                .map(|s| s.to_string())
                .collect();
            format!("Scanning: {}", sources.join(", "))
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
                // Apply source tab filter
                let matches_source = self.source_tab.matches(pkg.source);

                // Apply search filter
                let matches_search =
                    self.search_query.is_empty() || pkg.matches_search(&self.search_query);

                // Apply app type filter
                let matches_type = self.app_type_filter.matches(pkg.app_type);

                matches_source && matches_search && matches_type
            })
            .map(|(i, _)| i)
            .collect();

        // Reset selection if out of bounds
        if self.selected >= self.filtered_packages.len() {
            self.selected = self.filtered_packages.len().saturating_sub(1);
        }
    }

    /// Switch to next source tab
    pub fn next_tab(&mut self) {
        self.source_tab = self.source_tab.next();
        self.apply_filters();
    }

    /// Switch to previous source tab
    pub fn prev_tab(&mut self) {
        self.source_tab = self.source_tab.prev();
        self.apply_filters();
    }

    /// Handle character input for search
    pub fn search_input(&mut self, c: char) {
        self.search_query.push(c);
        self.apply_filters();
    }

    /// Handle backspace for search
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.apply_filters();
    }

    /// Clear search
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.apply_filters();
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
        self.selected =
            (self.selected + page_size).min(self.filtered_packages.len().saturating_sub(1));
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
                crate::package::PackageSource::Apt | crate::package::PackageSource::DebFile => {
                    apt += 1
                }
                crate::package::PackageSource::Snap => snap += 1,
                crate::package::PackageSource::Flatpak => flatpak += 1,
                crate::package::PackageSource::AppImage => appimage += 1,
            }
        }

        (self.packages.len(), apt, snap, flatpak, appimage)
    }

    /// Get count of packages with updates
    pub fn get_update_count(&self) -> usize {
        self.packages
            .iter()
            .filter(|p| p.has_update == Some(true))
            .count()
    }

    /// Toggle sidebar focus
    pub fn toggle_sidebar_focus(&mut self) {
        self.sidebar_focused = !self.sidebar_focused;
    }

    /// Move to next sidebar section
    pub fn next_sidebar_section(&mut self) {
        self.sidebar_section = self.sidebar_section.next();
    }

    /// Move to previous sidebar section
    pub fn prev_sidebar_section(&mut self) {
        self.sidebar_section = self.sidebar_section.prev();
    }
}
