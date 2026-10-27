//! Inventory publication and concurrent update discovery.
use super::*;
use futures::FutureExt;
use std::collections::HashMap;
use tokio::task::JoinSet;

#[derive(Clone)]
struct ScanOutcome {
    source: PackageSource,
    available: bool,
    packages: Vec<InstalledPackage>,
    error: Option<String>,
}

pub(super) async fn scan_sources(
    scanners: Vec<Box<dyn Scanner>>,
    desktop: tokio::task::JoinHandle<DesktopIndex>,
    on_inventory: impl FnOnce((Vec<InstalledPackage>, ScanAvailability)),
) -> (Vec<InstalledPackage>, ScanAvailability) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut join = JoinSet::new();
    for scanner in scanners {
        let tx = tx.clone();
        join.spawn(async move {
            let source = scanner.source();
            let available = scanner.is_available().await;
            let result = if available {
                scanner.scan().await
            } else {
                Ok(ScanReport::default())
            };
            let succeeded = result.is_ok() && available;
            let report = result.unwrap_or_else(|error| ScanReport {
                packages: Vec::new(),
                warning: Some(scan_error_message(&error)),
            });
            let inventory = ScanOutcome {
                source,
                available,
                packages: report.packages,
                error: report.warning,
            };
            let _ = tx.send(inventory.clone());
            // Close the inventory channel before waiting on network services.
            drop(tx);
            let mut report = ScanReport {
                packages: inventory.packages,
                warning: inventory.error,
            };
            if succeeded {
                if let Err(payload) = std::panic::AssertUnwindSafe(async {
                    scanner.check_updates(&mut report).await;
                })
                .catch_unwind()
                .await
                {
                    let message = payload
                        .downcast_ref::<&str>()
                        .copied()
                        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                        .unwrap_or("unknown panic");
                    let warning = format!("Update check panicked: {message}");
                    for package in &mut report.packages {
                        package.has_update = false;
                        package.update_version = None;
                        package.update_note = Some(warning.clone());
                        if let Some(installation) = package.appimage_installation.as_mut() {
                            installation.update = None;
                        }
                    }
                    report.warning = Some(match report.warning.take() {
                        Some(inventory_warning) => format!("{inventory_warning}; {warning}"),
                        None => warning,
                    });
                }
            }
            ScanOutcome {
                source,
                available,
                packages: report.packages,
                error: report.warning,
            }
        });
    }
    drop(tx);
    let mut inventories = Vec::new();
    while let Some(inventory) = rx.recv().await {
        inventories.push(inventory);
    }
    let desktop = std::sync::Arc::new(desktop.await.unwrap_or_else(|_| DesktopIndex::empty()));
    let (mut packages, mut availability) = merge(inventories, desktop).await;
    for package in &mut packages {
        if matches!(
            package.source,
            PackageSource::Apt | PackageSource::Snap | PackageSource::Flatpak
        ) || crate::domain::tool_managers::is_tool_source(package.source)
            || package.appimage_installation.is_some()
        {
            package
                .update_note
                .get_or_insert_with(|| "Checking for updates...".into());
        }
    }
    on_inventory((packages.clone(), availability.clone()));

    let package_indexes: HashMap<String, usize> = packages
        .iter()
        .enumerate()
        .map(|(index, package)| (package.key.clone(), index))
        .collect();
    while let Some(result) = join.join_next().await {
        let Ok(outcome) = result else {
            continue;
        };
        set_source_availability(&mut availability, &outcome);
        for checked in outcome.packages {
            let Some(&index) = package_indexes.get(&checked.key) else {
                continue;
            };
            let package = &mut packages[index];
            package.has_update = checked.has_update;
            package.update_version = checked.update_version;
            package.update_note = checked.update_note;
            if let (Some(package_installation), Some(checked_installation)) = (
                &mut package.appimage_installation,
                checked.appimage_installation,
            ) {
                package_installation.update = checked_installation.update;
            }
        }
    }
    (packages, availability)
}

async fn merge(
    outcomes: Vec<ScanOutcome>,
    desktop: std::sync::Arc<DesktopIndex>,
) -> (Vec<InstalledPackage>, ScanAvailability) {
    let mut merged: Vec<InstalledPackage> = Vec::new();
    let mut availability = ScanAvailability::default();
    for outcome in outcomes {
        set_source_availability(&mut availability, &outcome);
        merged.extend(outcome.packages);
    }

    // Enrich + classify + resolve icons, then sort apps-first, by display
    // name. Icon resolution touches the filesystem (theme lookups), so the
    // whole merge pass runs on a blocking thread to keep the async runtime
    // responsive. `DesktopIndex` and `InstalledPackage` are both `Send`.
    let (merged, availability) = tokio::task::spawn_blocking(move || {
        for pkg in merged.iter_mut() {
            enrich(pkg, &desktop);
        }
        let unmanaged_apps = desktop.unmanaged_user_apps(&merged);
        merged.extend(unmanaged_apps);
        // `sort_by_cached_key` computes the lowercase display name once per
        // package instead of once per comparison.
        merged.sort_by_cached_key(|p| {
            (
                kind_rank(p.app_kind),
                p.display_name.as_deref().unwrap_or(&p.name).to_lowercase(),
            )
        });
        (merged, availability)
    })
    .await
    .unwrap_or_else(|_| (Vec::new(), ScanAvailability::default()));

    (merged, availability)
}

fn set_source_availability(availability: &mut ScanAvailability, outcome: &ScanOutcome) {
    match outcome.source {
        PackageSource::Apt => {
            availability.apt = outcome.available;
            availability.apt_error = outcome.error.clone();
        }
        PackageSource::Snap => {
            availability.snap = outcome.available;
            availability.snap_error = outcome.error.clone();
        }
        PackageSource::Flatpak => {
            availability.flatpak = outcome.available;
            availability.flatpak_error = outcome.error.clone();
        }
        PackageSource::AppImage => {
            availability.appimage = outcome.available;
            availability.appimage_dirs = appimage::search_directories();
        }
        PackageSource::Steam => {
            availability.steam = outcome.available;
            availability.steam_error = outcome.error.clone();
        }
        PackageSource::JetBrainsToolbox => {
            availability.jetbrains_toolbox = outcome.available;
            availability.jetbrains_toolbox_error = outcome.error.clone();
        }
        PackageSource::Uv => {
            availability.uv = outcome.available;
            availability.uv_error = outcome.error.clone();
        }
        PackageSource::Pipx => {
            availability.pipx = outcome.available;
            availability.pipx_error = outcome.error.clone();
        }
        PackageSource::Cargo => {
            availability.cargo = outcome.available;
            availability.cargo_error = outcome.error.clone();
        }
        PackageSource::Npm => {
            availability.npm = outcome.available;
            availability.npm_error = outcome.error.clone();
        }
        PackageSource::Bun => {
            availability.bun = outcome.available;
            availability.bun_error = outcome.error.clone();
        }
        PackageSource::Desktop => {
            // Unmanaged desktop apps are synthesized from visible launchers
            // below, not reported by a package-manager scanner.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::{oneshot, Notify};

    struct GatedUpdates(Arc<Notify>);

    impl Scanner for GatedUpdates {
        fn source(&self) -> PackageSource {
            PackageSource::Cargo
        }
        fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(async { true })
        }
        fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
            Box::pin(async {
                let mut package = InstalledPackage::new(PackageSource::Cargo, "fixture");
                package.version = "1.0.0".into();
                Ok(ScanReport::ok(vec![package]))
            })
        }
        fn check_updates<'a>(
            &'a self,
            report: &'a mut ScanReport,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                self.0.notified().await;
                report.packages[0].has_update = true;
                report.packages[0].update_version = Some("2.0.0".into());
                report.warning = Some("fixture warning".into());
            })
        }
    }

    #[tokio::test]
    async fn publishes_inventory_before_update_services_finish() {
        let release = Arc::new(Notify::new());
        let (tx, rx) = oneshot::channel();
        let scan = scan_sources(
            vec![Box::new(GatedUpdates(release.clone()))],
            tokio::task::spawn_blocking(DesktopIndex::empty),
            |snapshot| {
                let _ = tx.send(snapshot);
            },
        );
        let controller = async {
            let (packages, availability) =
                tokio::time::timeout(std::time::Duration::from_secs(2), rx)
                    .await
                    .expect("installed list must not wait for update queries")
                    .unwrap();
            let fixture = packages.iter().find(|p| p.package_id == "fixture").unwrap();
            assert_eq!(fixture.version, "1.0.0");
            assert!(!fixture.has_update);
            assert_eq!(
                fixture.update_note.as_deref(),
                Some("Checking for updates...")
            );
            assert!(availability.cargo);
            release.notify_one();
        };
        let ((packages, availability), ()) = tokio::join!(scan, controller);
        let fixture = packages.iter().find(|p| p.package_id == "fixture").unwrap();
        assert_eq!(fixture.update_version.as_deref(), Some("2.0.0"));
        assert!(fixture.has_update);
        assert!(fixture.update_note.is_none());
        assert_eq!(availability.cargo_error.as_deref(), Some("fixture warning"));
    }

    struct FailedSource {
        source: PackageSource,
        available: bool,
    }
    impl Scanner for FailedSource {
        fn source(&self) -> PackageSource {
            self.source
        }
        fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(async move { self.available })
        }
        fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
            Box::pin(async { anyhow::bail!("inventory unavailable") })
        }
        fn check_updates<'a>(
            &'a self,
            _: &'a mut ScanReport,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            panic!("failed or absent sources must not run update checks")
        }
    }

    #[tokio::test]
    async fn failed_and_absent_sources_keep_their_availability_in_both_stages() {
        let (_, final_state) = scan_sources(
            vec![
                Box::new(FailedSource {
                    source: PackageSource::Apt,
                    available: true,
                }),
                Box::new(FailedSource {
                    source: PackageSource::Snap,
                    available: false,
                }),
                Box::new(FailedSource {
                    source: PackageSource::JetBrainsToolbox,
                    available: true,
                }),
            ],
            tokio::task::spawn_blocking(DesktopIndex::empty),
            |(packages, state)| {
                assert!(packages.is_empty());
                assert!(state.apt);
                assert_eq!(state.apt_error.as_deref(), Some("inventory unavailable"));
                assert!(!state.snap);
                assert!(state.snap_error.is_none());
                assert!(state.jetbrains_toolbox);
                assert_eq!(
                    state.jetbrains_toolbox_error.as_deref(),
                    Some("inventory unavailable")
                );
            },
        )
        .await;
        assert!(final_state.apt);
        assert_eq!(
            final_state.apt_error.as_deref(),
            Some("inventory unavailable")
        );
        assert!(!final_state.snap);
        assert!(final_state.snap_error.is_none());
        assert!(final_state.jetbrains_toolbox);
        assert_eq!(
            final_state.jetbrains_toolbox_error.as_deref(),
            Some("inventory unavailable")
        );
    }

    struct PanickingUpdates {
        before_poll: bool,
    }

    impl Scanner for PanickingUpdates {
        fn source(&self) -> PackageSource {
            PackageSource::Cargo
        }
        fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(async { true })
        }
        fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
            Box::pin(async {
                let mut package = InstalledPackage::new(PackageSource::Cargo, "fixture");
                package.name = "Fixture Tool".into();
                package.version = "1.0.0".into();
                Ok(ScanReport {
                    packages: vec![package],
                    warning: Some("inventory partially unavailable".into()),
                })
            })
        }
        fn check_updates<'a>(
            &'a self,
            report: &'a mut ScanReport,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            if self.before_poll {
                panic!("update service failed");
            }
            Box::pin(async move {
                report.packages[0].has_update = true;
                report.packages[0].update_version = Some("2.0.0".into());
                panic!("update service failed");
            })
        }
    }

    #[tokio::test]
    async fn panicked_update_check_retains_inventory_and_reports_unavailable_state() {
        for before_poll in [false, true] {
            let mut published_count = 0;
            let (packages, availability) = scan_sources(
                vec![Box::new(PanickingUpdates { before_poll })],
                tokio::task::spawn_blocking(DesktopIndex::empty),
                |(packages, availability)| {
                    assert!(availability.cargo);
                    assert_eq!(
                        availability.cargo_error.as_deref(),
                        Some("inventory partially unavailable")
                    );
                    assert_eq!(packages.len(), 1);
                    published_count = packages.len();
                },
            )
            .await;

            assert_eq!(published_count, 1);
            assert!(availability.cargo);
            assert_eq!(packages.len(), 1);
            assert_eq!(packages[0].package_id, "fixture");
            assert_eq!(packages[0].version, "1.0.0");
            assert!(!packages[0].has_update);
            assert!(packages[0].update_version.is_none());
            assert_eq!(
                packages[0].update_note.as_deref(),
                Some("Update check panicked: update service failed")
            );
            assert_eq!(
                availability.cargo_error.as_deref(),
                Some(
                    "inventory partially unavailable; Update check panicked: update service failed"
                )
            );
        }
    }

    struct AppImageUpdates;

    impl Scanner for AppImageUpdates {
        fn source(&self) -> PackageSource {
            PackageSource::AppImage
        }
        fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(async { true })
        }
        fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
            Box::pin(async {
                let mut package = InstalledPackage::new(
                    PackageSource::AppImage,
                    "/home/test/AppImages/tool.AppImage",
                );
                package.name = "Tool".into();
                package.appimage_installation =
                    Some(Box::new(crate::domain::appimage::AppImageInstallation {
                        path: "/home/test/AppImages/tool.AppImage".into(),
                        launcher: "/home/test/.local/share/applications/tool.desktop".into(),
                        launchers: vec!["/home/test/.local/share/applications/tool.desktop".into()],
                        icons: vec!["/home/test/AppImages/.icons/tool.png".into()],
                        size: 42,
                        modified_ns: 7,
                        update_info: Some("zsync|https://example.com/tool.zsync".into()),
                        update: None,
                    }));
                Ok(ScanReport::ok(vec![package]))
            })
        }
        fn check_updates<'a>(
            &'a self,
            report: &'a mut ScanReport,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            Box::pin(async move {
                let package = &mut report.packages[0];
                package.has_update = true;
                package.update_version = Some("2.0.0".into());
                package.appimage_installation.as_mut().unwrap().update =
                    Some(crate::domain::appimage::updates::AppImageUpdate {
                        download_url: "https://example.com/tool.AppImage".into(),
                        digest: crate::domain::appimage::updates::Digest::Sha256("abc".into()),
                        release: Some("v2.0.0".into()),
                        source: "Fixture".into(),
                    });
            })
        }
    }

    #[tokio::test]
    async fn final_update_results_preserve_appimage_installation_metadata() {
        let (packages, _) = scan_sources(
            vec![Box::new(AppImageUpdates)],
            tokio::task::spawn_blocking(DesktopIndex::empty),
            |_| {},
        )
        .await;

        let package = packages.iter().find(|p| p.name == "Tool").unwrap();
        let installation = package.appimage_installation.as_ref().unwrap();
        assert_eq!(installation.size, 42);
        assert_eq!(installation.launchers.len(), 1);
        assert_eq!(installation.icons.len(), 1);
        assert_eq!(
            installation
                .update
                .as_ref()
                .and_then(|update| update.release.as_deref()),
            Some("v2.0.0")
        );
        assert!(package.has_update);
        assert_eq!(package.update_version.as_deref(), Some("2.0.0"));
    }
}
