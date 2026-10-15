//! Inventory publication and concurrent update discovery.
use super::*;
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
                scanner.check_updates(&mut report).await;
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
    let (mut packages, availability) = merge(inventories, desktop.clone()).await;
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
    on_inventory((packages, availability));

    let mut outcomes = Vec::new();
    while let Some(result) = join.join_next().await {
        if let Ok(outcome) = result {
            outcomes.push(outcome);
        }
    }
    merge(outcomes, desktop).await
}

async fn merge(
    outcomes: Vec<ScanOutcome>,
    desktop: std::sync::Arc<DesktopIndex>,
) -> (Vec<InstalledPackage>, ScanAvailability) {
    let mut merged: Vec<InstalledPackage> = Vec::new();
    let mut availability = ScanAvailability::default();
    for outcome in outcomes {
        match outcome.source {
            PackageSource::Apt => {
                availability.apt = outcome.available;
                availability.apt_error = outcome.error;
            }
            PackageSource::Snap => {
                availability.snap = outcome.available;
                availability.snap_error = outcome.error;
            }
            PackageSource::Flatpak => {
                availability.flatpak = outcome.available;
                availability.flatpak_error = outcome.error;
            }
            PackageSource::AppImage => {
                availability.appimage = outcome.available;
                availability.appimage_dirs = appimage::search_directories();
            }
            PackageSource::Steam => {
                availability.steam = outcome.available;
                availability.steam_error = outcome.error;
            }
            PackageSource::Uv => {
                availability.uv = outcome.available;
                availability.uv_error = outcome.error;
            }
            PackageSource::Pipx => {
                availability.pipx = outcome.available;
                availability.pipx_error = outcome.error;
            }
            PackageSource::Cargo => {
                availability.cargo = outcome.available;
                availability.cargo_error = outcome.error;
            }
            PackageSource::Npm => {
                availability.npm = outcome.available;
                availability.npm_error = outcome.error;
            }
            PackageSource::Bun => {
                availability.bun = outcome.available;
                availability.bun_error = outcome.error;
            }
            PackageSource::Desktop => {
                // Unmanaged desktop apps are synthesized from visible launchers
                // below, not reported by a package-manager scanner.
            }
        }
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
            ],
            tokio::task::spawn_blocking(DesktopIndex::empty),
            |(packages, state)| {
                assert!(packages.is_empty());
                assert!(state.apt);
                assert_eq!(state.apt_error.as_deref(), Some("inventory unavailable"));
                assert!(!state.snap);
                assert!(state.snap_error.is_none());
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
    }
}
