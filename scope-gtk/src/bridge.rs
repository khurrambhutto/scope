//! Bridges between the async backend world (tokio) and the GTK main loop.
//!
//! Backend futures run on a shared multi-thread tokio runtime; their results
//! are delivered back onto the GLib main context through a bounded channel, so
//! UI callbacks can touch widgets safely.

use std::sync::Arc;
use std::sync::OnceLock;

use gtk::glib;

/// Timestamped debug tracing, enabled with `SCOPE_GTK_DEBUG=1`.
pub fn debug_log(message: &str) {
    if std::env::var_os("SCOPE_GTK_DEBUG").is_some() {
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        let elapsed = START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64();
        eprintln!("[scope {elapsed:>8.3}s] {message}");
    }
}

use crate::backend::operations::{
    now_ms, Operation, OperationPlan, OperationResult, PlanStore,
};
use crate::backend::package::{InstallScope, InstalledPackage, PackageSource};
use crate::backend::scanner::{scan_all, ScanAvailability};

/// Run `future` on the tokio runtime, then run `callback` on the GTK main
/// loop with its output. Callbacks may touch widgets freely.
pub fn spawn<F, T, C>(future: F, callback: C)
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
    C: FnOnce(T) + 'static,
{
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        debug_log("initializing tokio runtime");
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to start tokio runtime")
    });

    let (tx, rx) = async_channel::bounded::<T>(1);
    runtime.spawn(async move {
        let output = future.await;
        debug_log("tokio task finished, sending result to main loop");
        let _ = tx.send(output).await;
    });
    debug_log("future spawned on tokio runtime, callback queued on main loop");

    glib::MainContext::default().spawn_local(async move {
        if let Ok(value) = rx.recv().await {
            debug_log("callback dispatched on main loop");
            callback(value);
        }
    });
}

/// Snapshot of the latest full scan, cached exactly like the Tauri backend's
/// `ScanCache` so previews resolve against backend-known packages only.
#[derive(Clone)]
pub struct CachedScan {
    pub packages: Vec<InstalledPackage>,
    pub availability: ScanAvailability,
    pub scanned_at_ms: u64,
}

impl Default for CachedScan {
    fn default() -> Self {
        Self {
            packages: Vec::new(),
            availability: ScanAvailability::default(),
            scanned_at_ms: 0,
        }
    }
}

/// Backend service used by the GTK UI.
///
/// This mirrors the Tauri command layer (`src-tauri/src/commands/`) one-to-one:
/// preview issues a plan into the [`PlanStore`], and apply only accepts a plan
/// id the backend itself issued, revalidating against a fresh scan first. The
/// frontend never picks arbitrary package ids or commands.
#[derive(Default, Clone)]
pub struct Backend {
    cache: Arc<tokio::sync::Mutex<Option<CachedScan>>>,
    plans: PlanStore,
}

impl Backend {
    /// Full parallel scan across all sources; caches and returns the result.
    pub async fn scan(&self) -> CachedScan {
        debug_log("backend scan started");
        let (packages, availability) = scan_all().await;
        debug_log(&format!("backend scan finished: {} packages", packages.len()));
        let snapshot = CachedScan {
            packages: packages.clone(),
            availability,
            scanned_at_ms: now_ms(),
        };
        *self.cache.lock().await = Some(snapshot.clone());
        snapshot
    }

    /// Look up one package by backend key in the cached scan.
    pub async fn find(&self, key: &str) -> Option<InstalledPackage> {
        let guard = self.cache.lock().await;
        let cached = guard.as_ref()?;
        cached.packages.iter().find(|p| p.key == key).cloned()
    }

    /// Build (and store) an uninstall preview plan. Protected packages return
    /// an unissued plan so their id can never be applied.
    pub async fn preview_uninstall(&self, key: &str) -> Result<OperationPlan, String> {
        let pkg = self
            .find(key)
            .await
            .ok_or_else(|| format!("Package not found in current scan: {key}"))?;
        let plan = crate::backend::operations::uninstall::preview(&pkg);
        if plan.protected {
            return Ok(plan);
        }
        self.plans.issue(plan.clone()).await;
        Ok(plan)
    }

    /// Build (and store) an update preview plan; requires a known update.
    pub async fn preview_update(&self, key: &str) -> Result<OperationPlan, String> {
        let pkg = self
            .find(key)
            .await
            .ok_or_else(|| format!("Package not found in current scan: {key}"))?;
        if !pkg.has_update {
            let name = pkg.display_name.unwrap_or_else(|| pkg.name.clone());
            return Err(format!("'{name}' has no updates available."));
        }
        let plan = crate::backend::operations::update::preview(&pkg);
        if plan.protected {
            return Ok(plan);
        }
        self.plans.issue(plan.clone()).await;
        Ok(plan)
    }

    /// Apply an uninstall plan by id: fresh scan + revalidate, then execute.
    pub async fn apply_uninstall(&self, plan_id: &str) -> Result<OperationResult, String> {
        let plan = self
            .plans
            .take(plan_id)
            .await
            .ok_or_else(|| "Stale or unknown uninstall plan. Please preview again.".to_string())?;
        let (packages, _) = scan_all().await;
        crate::backend::operations::uninstall::revalidate(&plan, &packages)
            .await
            .map_err(|e| e.to_string())?;
        let result = crate::backend::operations::uninstall::apply(&plan).await;
        Ok(result)
    }

    /// Apply an update plan by id: fresh scan + revalidate, then execute.
    pub async fn apply_update(&self, plan_id: &str) -> Result<OperationResult, String> {
        let plan = self
            .plans
            .take(plan_id)
            .await
            .ok_or_else(|| "Stale or unknown update plan. Please preview again.".to_string())?;
        let (packages, _) = scan_all().await;
        crate::backend::operations::update::revalidate(&plan, &packages)
            .await
            .map_err(|e| e.to_string())?;
        let result = crate::backend::operations::update::apply(&plan).await;
        Ok(result)
    }

    /// What this plan would do (for task titles).
    pub fn operation_label(op: Operation) -> &'static str {
        match op {
            Operation::Uninstall => "Uninstall",
            Operation::Update => "Update",
        }
    }
}

/// Decode the `scope-icon://localhost/<escaped-path>` URL produced by the
/// backend's enrichment step back into the local file it is allowed to serve.
///
/// The GTK app performs the same whitelist check the Tauri protocol handler
/// does (`icons::is_registered_path`) before touching the filesystem.
pub fn icon_path_from_url(url: &str) -> Option<std::path::PathBuf> {
    // `icon_url` produces `scope-icon://localhost/<absolute-path>`; the leading
    // `/` after the host is part of the absolute path itself.
    let escaped = url.strip_prefix("scope-icon://localhost")?;
    Some(percent_decode(escaped).into())
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Package id safe for display. Manual packages pack
/// `<binary>\x1f<desktop-path>` into `package_id`; show only the primary part.
pub fn display_id(pkg: &InstalledPackage) -> String {
    match pkg.source {
        PackageSource::Manual => crate::backend::scanner::split_manual_id(&pkg.package_id)
            .0
            .to_string(),
        _ => pkg.package_id.clone(),
    }
}

/// Human label for the install scope, if the source has one.
#[allow(dead_code)]
pub fn scope_label(pkg: &InstalledPackage) -> Option<&'static str> {
    match pkg.install_scope {
        Some(InstallScope::User) => Some("User"),
        Some(InstallScope::System) => Some("System-wide"),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_icon_urls() {
        let path = icon_path_from_url("scope-icon://localhost/usr/share/icons/hicolor/48x48/apps/a%20b.png")
            .expect("url decodes");
        assert_eq!(path.to_str().unwrap(), "/usr/share/icons/hicolor/48x48/apps/a b.png");
        assert!(icon_path_from_url("https://example.com/x.png").is_none());
    }

    #[test]
    fn display_id_unpacks_manual_keys() {
        let pkg = InstalledPackage::new(
            PackageSource::Manual,
            "/opt/MyApp/bin/myapp\u{1f}/home/u/.local/share/applications/myapp.desktop",
        );
        assert_eq!(display_id(&pkg), "/opt/MyApp/bin/myapp");
    }
}

#[cfg(test)]
mod delivery_tests {
    /// Headless check that `bridge::spawn` delivers a tokio-computed value back
    /// onto the default GLib main context (no GTK/display required).
    #[test]
    fn bridge_delivers_on_main_context() {
        let (done_tx, done_rx) = std::sync::mpsc::channel::<u64>();

        super::spawn(
            async {
                // Simulate a real backend call: async + a worker thread hop.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                tokio::task::spawn_blocking(|| 42u64)
                    .await
                    .expect("blocking task")
            },
            move |value| {
                done_tx.send(value).expect("receiver alive");
            },
        );

        let ctx = gtk::glib::MainContext::default();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if let Ok(value) = done_rx.try_recv() {
                assert_eq!(value, 42);
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "callback never delivered within 10s"
            );
            ctx.iteration(true);
        }
    }
}
