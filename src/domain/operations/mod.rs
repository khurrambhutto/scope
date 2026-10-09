//! Update/uninstall preview and apply flows.
//!
//! Every destructive action is preview-first: the frontend asks for a plan, the
//! backend validates it against the live system and returns an [`OperationPlan`]
//! the user confirms. The apply step only trusts a plan id that the backend
//! itself issued (stored in [`PlanStore`]) and revalidates the system state
//! before executing — so a stale or tampered plan is rejected.

pub mod apt_transaction;
pub mod probe;
pub mod uninstall;
pub mod update;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::domain::package::InstalledPackage;
use crate::domain::package::{InstallScope, PackageSource};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum OperationTarget {
    Apt { package_id: String },
    Snap { package_id: String },
    Flatpak { app_id: String, scope: InstallScope },
    AppImage { path: PathBuf },
    Desktop { desktop_file: PathBuf },
    /// Steam discovery is read-only; every operation phase rejects this target.
    Steam { app_id: String },
}

impl OperationTarget {
    pub fn from_package(package: &InstalledPackage) -> anyhow::Result<Self> {
        match package.source {
            PackageSource::Apt => Ok(Self::Apt {
                package_id: package.package_id.clone(),
            }),
            PackageSource::Snap => Ok(Self::Snap {
                package_id: package.package_id.clone(),
            }),
            PackageSource::Flatpak => Ok(Self::Flatpak {
                app_id: package.package_id.clone(),
                scope: package.install_scope.ok_or_else(|| {
                    anyhow::anyhow!("Flatpak installation scope is missing. Rescan and try again.")
                })?,
            }),
            PackageSource::AppImage => Ok(Self::AppImage {
                path: PathBuf::from(&package.package_id),
            }),
            PackageSource::Desktop => Ok(Self::Desktop {
                desktop_file: PathBuf::from(&package.package_id),
            }),
            PackageSource::Steam => Ok(Self::Steam {
                app_id: package.package_id.clone(),
            }),
        }
    }

    pub fn source(&self) -> PackageSource {
        match self {
            Self::Apt { .. } => PackageSource::Apt,
            Self::Snap { .. } => PackageSource::Snap,
            Self::Flatpak { .. } => PackageSource::Flatpak,
            Self::AppImage { .. } => PackageSource::AppImage,
            Self::Desktop { .. } => PackageSource::Desktop,
            Self::Steam { .. } => PackageSource::Steam,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Apt { package_id } | Self::Snap { package_id } => package_id,
            Self::Flatpak { app_id, .. } | Self::Steam { app_id } => app_id,
            Self::AppImage { path } => path.to_str().unwrap_or(""),
            Self::Desktop { desktop_file } => desktop_file.to_str().unwrap_or(""),
        }
    }

    pub fn install_scope(&self) -> Option<InstallScope> {
        match self {
            Self::Flatpak { scope, .. } => Some(*scope),
            _ => None,
        }
    }
}

/// What kind of operation a plan describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Uninstall,
    Update,
}

/// How privilege escalation is handled. Scope never touches passwords — `pkexec`
/// hands auth to Polkit, which shows the native system password dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// No elevation needed (user-installed flatpaks, AppImage trash).
    None,
    /// Runs via `pkexec`, which triggers a Polkit password popup.
    Pkexec,
}

/// One human-readable step in a plan, with a safe command summary for the
/// confirmation view. The command summary is display-only and never re-parsed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub description: String,
    pub command_summary: String,
}

/// A backend-validated, user-confirmable operation plan.
///
/// The frontend receives this for confirmation and sends only `plan_id` back to
/// apply. The full plan contents live server-side in [`PlanStore`], so the
/// frontend cannot tamper with what gets executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationPlan {
    pub plan_id: String,
    pub operation: Operation,
    pub target: OperationTarget,
    pub display_name: String,
    pub current_version: String,
    pub target_version: String,
    pub requires_auth: bool,
    pub auth_method: AuthMethod,
    pub protected: bool,
    pub protection_reason: Option<String>,
    pub steps: Vec<PlanStep>,
    pub created_at_ms: u64,
    pub apt_transaction_fingerprint: Option<String>,
    /// Informational Snap breakdown captured at preview; never executed as paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_details: Option<Box<crate::domain::snap_details::SnapDetails>>,
}

/// Outcome of applying a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub logs: String,
    pub exit_code: Option<i32>,
}

/// Progress stage of a running apply flow, streamed to the UI so the wait
/// before the Polkit password dialog is never a silent gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationStage {
    /// Checking that the package still matches the plan and is safe to act on.
    Verifying,
    /// The package-manager command is running (possibly awaiting a Polkit
    /// password dialog).
    Executing,
}

/// Payload of [`OPERATION_STATUS_EVENT`], emitted while an apply command runs.
/// Tauri-only transport type; kept so the domain stays in sync — GPUI streams
/// progress via `backend::OpMsg` instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct OperationStatus {
    pub plan_id: String,
    pub stage: OperationStage,
}

/// One line of live command output, emitted while an apply command runs so the
/// UI can show real progress instead of a bare spinner.
/// Tauri-only transport type; see `OperationStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct OperationLog {
    pub plan_id: String,
    pub line: String,
}

/// Event name used to stream [`OperationStatus`] to the frontend (Tauri only).
#[allow(dead_code)]
pub const OPERATION_STATUS_EVENT: &str = "operation-status";

/// Event name used to stream live command output ([`OperationLog`]) (Tauri only).
#[allow(dead_code)]
pub const OPERATION_LOG_EVENT: &str = "operation-log";

/// In-memory store of issued plans, keyed by id. Plans expire after
/// [`PLAN_TTL`] so a user who walks away cannot later apply a stale plan that
/// no longer reflects the system.
#[derive(Default, Clone)]
pub struct PlanStore {
    inner: Arc<std::sync::Mutex<HashMap<String, StoredPlan>>>,
}

struct StoredPlan {
    plan: OperationPlan,
    created: Instant,
}

/// Plans are valid for 5 minutes after preview.
pub const PLAN_TTL: Duration = Duration::from_secs(5 * 60);

impl PlanStore {
    /// Register a freshly previewed plan. Expired entries are swept here so the
    /// store cannot grow without bound.
    pub fn issue(&self, plan: OperationPlan) {
        let id = plan.plan_id.clone();
        let mut guard = self.lock();
        guard.insert(
            id,
            StoredPlan {
                plan,
                created: Instant::now(),
            },
        );
        // Opportunistic cleanup of expired entries.
        guard.retain(|_, v| v.created.elapsed() < PLAN_TTL);
    }

    /// Take (and remove) a non-expired plan. Returns `None` if missing/expired,
    /// which the apply command treats as a stale-plan rejection.
    pub fn take(&self, plan_id: &str) -> Option<OperationPlan> {
        self.take_at(plan_id, Instant::now())
    }

    /// Clock-injectable form of [`take`](Self::take) so expiry is testable
    /// without sleeping for [`PLAN_TTL`].
    fn take_at(&self, plan_id: &str, now: Instant) -> Option<OperationPlan> {
        let mut guard = self.lock();
        let valid = guard
            .get(plan_id)
            .is_some_and(|v| now.duration_since(v.created) < PLAN_TTL);
        if valid {
            guard.remove(plan_id).map(|v| v.plan)
        } else {
            // Drop any expired entry with this id too.
            guard.remove(plan_id);
            None
        }
    }

    /// Lock the store, recovering from poisoning rather than panicking. The
    /// critical section is synchronous, so a plain `std` mutex is correct here.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, StoredPlan>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Generate a unique-enough plan id (timestamp + counter). Not a security
/// primitive — apply revalidates against the real system regardless.
pub fn new_plan_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    format!("plan-{ms}-{n}")
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(id: &str) -> OperationPlan {
        OperationPlan {
            plan_id: id.into(),
            operation: Operation::Uninstall,
            target: OperationTarget::Apt {
                package_id: "gimp".into(),
            },
            display_name: "GIMP".into(),
            current_version: "2.10".into(),
            target_version: String::new(),
            requires_auth: true,
            auth_method: AuthMethod::Pkexec,
            protected: false,
            protection_reason: None,
            steps: vec![],
            created_at_ms: 0,
            apt_transaction_fingerprint: None,
            snap_details: None,
        }
    }

    #[test]
    fn issue_then_take_returns_the_plan() {
        let store = PlanStore::default();
        store.issue(plan("plan-1"));
        assert_eq!(
            store.take("plan-1").map(|p| p.target.id().to_string()),
            Some("gimp".into())
        );
    }

    #[test]
    fn take_is_single_use() {
        let store = PlanStore::default();
        store.issue(plan("plan-1"));
        let _ = store.take("plan-1");
        assert!(store.take("plan-1").is_none());
    }

    #[test]
    fn take_returns_none_for_unknown_id() {
        let store = PlanStore::default();
        assert!(store.take("never-issued").is_none());
    }

    #[test]
    fn take_rejects_a_plan_older_than_the_ttl() {
        let store = PlanStore::default();
        store.issue(plan("plan-1"));
        let after_ttl = Instant::now() + PLAN_TTL + Duration::from_secs(1);
        assert!(store.take_at("plan-1", after_ttl).is_none());
    }

    #[test]
    fn expired_plan_is_removed_so_a_later_take_cannot_revive_it() {
        let store = PlanStore::default();
        store.issue(plan("plan-1"));
        let after_ttl = Instant::now() + PLAN_TTL + Duration::from_secs(1);
        let _ = store.take_at("plan-1", after_ttl);
        assert!(store.take("plan-1").is_none());
    }

    #[test]
    fn issuing_again_replaces_the_previous_plan() {
        let store = PlanStore::default();
        store.issue(plan("plan-1"));
        let mut replacement = plan("plan-1");
        replacement.target = OperationTarget::Apt {
            package_id: "vlc".into(),
        };
        store.issue(replacement);
        assert_eq!(
            store.take("plan-1").map(|p| p.target.id().to_string()),
            Some("vlc".into())
        );
    }

    #[test]
    fn new_plan_ids_are_unique() {
        assert_ne!(new_plan_id(), new_plan_id());
    }

    #[test]
    fn steam_targets_preserve_the_app_id_for_read_only_rejection() {
        let package = InstalledPackage::new(PackageSource::Steam, "570");
        let target = OperationTarget::from_package(&package).unwrap();

        assert_eq!(target.source(), PackageSource::Steam);
        assert_eq!(target.id(), "570");
    }
}
