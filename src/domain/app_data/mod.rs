//! Reviewed, source-neutral personal-data cleanup. The stored plan is the authority.
mod catalog;
mod claims;
mod discovery;
mod identity;
mod native;
pub(crate) mod processes;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::domain::package::InstalledPackage;
use identity::{FileIdentity, Roots};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupEffect {
    Included,
    Trash,
    Native,
    Kept,
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum CleanupAction {
    #[default]
    Trash,
    FlatpakPermissions {
        app_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupItem {
    #[serde(default)]
    pub action: CleanupAction,
    pub id: String,
    pub label: String,
    pub path: PathBuf,
    pub bytes: Option<u64>,
    pub effect: CleanupEffect,
    pub default_selected: bool,
    pub reason: String,
    identity: Option<FileIdentity>,
}

impl CleanupItem {
    pub fn selectable(&self) -> bool {
        matches!(self.effect, CleanupEffect::Trash | CleanupEffect::Native)
    }
    pub fn location_label(&self) -> String {
        if self.path.as_os_str().is_empty() {
            self.label.clone()
        } else {
            self.path.display().to_string()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanupPlan {
    pub items: Vec<CleanupItem>,
    pub warnings: Vec<String>,
    package: Option<Box<InstalledPackage>>,
    roots: Option<Roots>,
    #[serde(default)]
    executables: Vec<PathBuf>,
    #[serde(default)]
    hidden_writers: Vec<String>,
}

impl CleanupPlan {
    pub fn default_selection(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|i| i.selectable() && i.default_selected)
            .map(|i| i.id.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupStatus {
    Included,
    Trashed,
    Removed,
    Kept,
    Shared,
    Changed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupOutcome {
    pub label: String,
    pub path: PathBuf,
    pub status: CleanupStatus,
    pub message: String,
}

pub fn validate_selection(plan: &CleanupPlan, selected: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    for id in selected {
        if !seen.insert(id) {
            bail!("Duplicate cleanup selection. Preview again.");
        }
        if !plan.items.iter().any(|i| {
            &i.id == id
                && i.selectable()
                && match i.action {
                    CleanupAction::Trash => i.identity.is_some(),
                    CleanupAction::FlatpakPermissions { ref app_id } => {
                        plan.package.as_ref().is_some_and(|pkg| {
                            pkg.source == crate::domain::package::PackageSource::Flatpak
                                && pkg.package_id == *app_id
                                && pkg.install_scope.is_some()
                        })
                    }
                }
        }) {
            bail!("Unknown or unavailable cleanup selection. Preview again.");
        }
    }
    for (index, id) in selected.iter().enumerate() {
        let Some(item) = plan.items.iter().find(|i| &i.id == id) else {
            bail!("Cleanup selection is unavailable");
        };
        if selected[index + 1..]
            .iter()
            .filter_map(|id| plan.items.iter().find(|i| &i.id == id))
            .any(|other| {
                !item.path.as_os_str().is_empty()
                    && !other.path.as_os_str().is_empty()
                    && (item.path.starts_with(&other.path) || other.path.starts_with(&item.path))
            })
        {
            bail!("Overlapping cleanup selections. Preview again.");
        }
    }
    Ok(())
}

pub async fn preview(pkg: &InstalledPackage) -> CleanupPlan {
    let roots = match Roots::current() {
        Ok(roots) => roots,
        Err(error) => {
            return CleanupPlan {
                warnings: vec![format!("App data cannot be inspected: {error}")],
                ..Default::default()
            }
        }
    };
    let claims = claims::collect(pkg, &roots).await;
    let Some(permit) = crate::domain::package_details::inspection_permit().await else {
        return CleanupPlan {
            warnings: vec!["App-data inspection is busy. Preview again to review cleanup.".into()],
            ..Default::default()
        };
    };
    let package = pkg.clone();
    let fallback = package.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut plan = discovery::build(&package, &roots, &claims);
        if plan.items.iter().any(CleanupItem::selectable) || package.extracted_installation.is_some() {
            let inspected = processes::assess(&package, &plan.executables, &roots);
            if inspected.running {
                plan.warnings.push(if package.extracted_installation.is_some() {
                    "Close the extracted app before uninstalling."
                } else {
                    "Close the app before uninstalling with app data selected."
                }.into());
            }
            if let Some(error) = inspected.error {
                for item in &mut plan.items {
                    if item.selectable() {
                        item.effect = CleanupEffect::Unknown;
                        item.default_selected = false;
                        item.reason = error.clone();
                    }
                }
                plan.warnings.push(error);
            } else if !inspected.hidden.is_empty() {
                plan.warnings.push(format!("{} protected background process(es) could not be inspected. Data choices start unchecked. Select them only after closing the app; background writers may recreate data.", inspected.hidden.len()));
                if package.extracted_installation.is_some() {
                    plan.warnings.push("Confirming Uninstall accepts these displayed process-inspection gaps for the extracted app files. A known running app or a new gap still stops removal.".into());
                }
                for item in &mut plan.items {
                    item.default_selected = false;
                }
                plan.hidden_writers = inspected.hidden;
            }
        }
        plan
    })
    .await
    .unwrap_or_else(|_| CleanupPlan {
        package: Some(Box::new(fallback)),
        warnings: vec!["App-data discovery stopped; coverage is unknown.".into()],
        ..Default::default()
    })
}

/// Refuse a selected profile cleanup before uninstalling its running application.
pub fn preflight(plan: &CleanupPlan, selected: &[String]) -> Result<()> {
    validate_selection(plan, selected)?;
    if selected.is_empty()
        && !plan
            .package
            .as_ref()
            .is_some_and(|package| package.extracted_installation.is_some())
    {
        return Ok(());
    }
    let package = plan
        .package
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Cleanup context is missing"))?;
    let roots = plan
        .roots
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Cleanup roots are missing"))?;
    if &Roots::current()? != roots {
        bail!("HOME or XDG roots changed; preview again");
    }
    if let Some(reason) =
        processes::check_current(package, &plan.executables, roots, &plan.hidden_writers)
    {
        bail!("{reason}");
    }
    Ok(())
}

pub(crate) fn extracted_process_review<'a>(
    plan: &'a CleanupPlan,
    installation: &crate::domain::extracted_app::ExtractedInstallation,
) -> Result<&'a [String]> {
    if plan
        .package
        .as_ref()
        .and_then(|package| package.extracted_installation.as_deref())
        != Some(installation)
    {
        bail!("Extracted installation does not match its reviewed uninstall plan");
    }
    if plan.roots.as_ref() != Some(&Roots::current()?) {
        bail!("HOME or XDG roots changed; preview again");
    }
    Ok(&plan.hidden_writers)
}

/// Call only after the exact installation has been verified removed.
pub async fn apply(
    plan: &CleanupPlan,
    selected: &[String],
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> Vec<CleanupOutcome> {
    if let Err(error) = validate_selection(plan, selected) {
        return plan
            .items
            .iter()
            .map(|i| outcome(i, CleanupStatus::Unknown, error.to_string()))
            .collect();
    }
    if selected.is_empty() {
        return plan.items.iter().map(unselected).collect();
    }
    let Some(package) = &plan.package else {
        return Vec::new();
    };
    let current = Roots::current();
    let Some(roots) = plan
        .roots
        .as_ref()
        .filter(|old| current.as_ref().is_ok_and(|now| now == *old))
    else {
        return plan
            .items
            .iter()
            .map(|i| {
                outcome(
                    i,
                    CleanupStatus::Unknown,
                    "HOME or XDG roots changed; data was retained.".into(),
                )
            })
            .collect();
    };
    let mut claims = claims::collect(package, roots).await;
    claims.writer =
        processes::check_current(package, &plan.executables, roots, &plan.hidden_writers);
    apply_with(
        plan,
        selected,
        &claims,
        |path| {
            let expected = plan
                .items
                .iter()
                .find(|item| item.path == path)
                .and_then(|item| item.identity.clone());
            let roots = roots.clone();
            async move {
                let expected = expected
                    .ok_or_else(|| anyhow::anyhow!("Reviewed filesystem identity is missing"))?;
                crate::domain::system::trash::trash_checked(&path, move |candidate| {
                    if identity::verify(candidate, &roots)? != expected {
                        bail!("The reviewed app-data identity changed");
                    }
                    Ok(())
                })
                .await
            }
        },
        on_line,
    )
    .await
}

async fn apply_with<F, Fut>(
    plan: &CleanupPlan,
    selected: &[String],
    claims: &claims::Claims,
    trash: F,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> Vec<CleanupOutcome>
where
    F: Fn(PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let mut results: Vec<CleanupOutcome> = Vec::new();
    let (Some(package), Some(roots)) = (&plan.package, &plan.roots) else {
        return results;
    };
    for item in &plan.items {
        let result = if !selected.contains(&item.id) {
            if item.effect == CleanupEffect::Included
                && results.iter().any(|parent| {
                    parent.status == CleanupStatus::Trashed
                        && !parent.path.as_os_str().is_empty()
                        && item.path.starts_with(&parent.path)
                })
            {
                outcome(
                    item,
                    CleanupStatus::Included,
                    "Moved with its selected parent folder.".into(),
                )
            } else {
                unselected(item)
            }
        } else if !claims.complete {
            outcome(
                item,
                CleanupStatus::Unknown,
                "App-data ownership could not be verified; retained.".into(),
            )
        } else if let Some(reason) = claims.shared_reason(package, &item.path) {
            outcome(item, CleanupStatus::Shared, reason)
        } else if let Some(reason) = &claims.writer {
            outcome(item, CleanupStatus::Unknown, reason.clone())
        } else if let CleanupAction::FlatpakPermissions { app_id } = &item.action {
            match native::reset_permissions(app_id).await {
                Ok(()) => outcome(
                    item,
                    CleanupStatus::Removed,
                    "Saved Flatpak permissions removed.".into(),
                ),
                Err(error) => outcome(
                    item,
                    CleanupStatus::Failed,
                    format!("Permission cleanup failed: {error}"),
                ),
            }
        } else if item
            .identity
            .as_ref()
            .is_none_or(|old| identity::verify(&item.path, roots).as_ref().ok() != Some(old))
        {
            outcome(
                item,
                CleanupStatus::Changed,
                "The reviewed path or its ownership changed; retained.".into(),
            )
        } else {
            match trash(item.path.clone()).await {
                Ok(()) if matches!(std::fs::symlink_metadata(&item.path), Err(e) if e.kind() == std::io::ErrorKind::NotFound) => {
                    outcome(item, CleanupStatus::Trashed, "Moved to Trash.".into())
                }
                Ok(()) => outcome(
                    item,
                    CleanupStatus::Failed,
                    "The path remains or was recreated after Trash.".into(),
                ),
                Err(error) => outcome(
                    item,
                    CleanupStatus::Failed,
                    format!("Could not move to Trash: {error}"),
                ),
            }
        };
        on_line(&format!(
            "[scope] {}: {}",
            result.path.display(),
            result.message
        ));
        results.push(result);
    }
    results
}

fn unselected(item: &CleanupItem) -> CleanupOutcome {
    match item.effect {
        CleanupEffect::Included => outcome(
            item,
            CleanupStatus::Kept,
            "The parent folder was not removed.".into(),
        ),
        CleanupEffect::Unknown => outcome(item, CleanupStatus::Unknown, item.reason.clone()),
        CleanupEffect::Kept => outcome(item, CleanupStatus::Shared, item.reason.clone()),
        CleanupEffect::Trash | CleanupEffect::Native => {
            outcome(item, CleanupStatus::Kept, "Kept by choice.".into())
        }
    }
}

fn outcome(item: &CleanupItem, status: CleanupStatus, message: String) -> CleanupOutcome {
    CleanupOutcome {
        label: item.label.clone(),
        path: item.path.clone(),
        status,
        message,
    }
}
