//! Update/uninstall preview and apply flows.
//!
//! Every destructive action is preview-first: the frontend asks for a plan, the
//! backend validates it against the live system and returns an [`OperationPlan`]
//! the user confirms. The apply step only trusts a plan id that the backend
//! itself issued (stored in [`PlanStore`]) and revalidates the system state
//! before executing — so a stale or tampered plan is rejected.

pub mod uninstall;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::package::{InstallScope, PackageSource};

/// What kind of operation a plan describes. `Update` is reserved for a later
/// phase and not yet wired to the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Uninstall,
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
    pub source: PackageSource,
    pub package_id: String,
    pub install_scope: Option<InstallScope>,
    pub display_name: String,
    pub current_version: String,
    pub requires_auth: bool,
    pub auth_method: AuthMethod,
    pub protected: bool,
    pub protection_reason: Option<String>,
    pub steps: Vec<PlanStep>,
    pub created_at_ms: u64,
}

/// Outcome of applying a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub logs: String,
    pub exit_code: Option<i32>,
}

/// In-memory store of issued plans, keyed by id. Plans expire after
/// [`PLAN_TTL`] so a user who walks away cannot later apply a stale plan that
/// no longer reflects the system.
#[derive(Default, Clone)]
pub struct PlanStore {
    inner: Arc<tokio::sync::Mutex<HashMap<String, StoredPlan>>>,
}

struct StoredPlan {
    plan: OperationPlan,
    created: Instant,
}

/// Plans are valid for 5 minutes after preview.
pub const PLAN_TTL: Duration = Duration::from_secs(5 * 60);

impl PlanStore {
    pub async fn issue(&self, plan: OperationPlan) {
        let id = plan.plan_id.clone();
        let mut guard = self.inner.lock().await;
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
    pub async fn take(&self, plan_id: &str) -> Option<OperationPlan> {
        let mut guard = self.inner.lock().await;
        let exists = guard
            .get(plan_id)
            .map(|v| v.created.elapsed() < PLAN_TTL)
            .unwrap_or(false);
        if exists {
            guard.remove(plan_id).map(|v| v.plan)
        } else {
            // Drop any expired entry with this id too.
            guard.remove(plan_id);
            None
        }
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
