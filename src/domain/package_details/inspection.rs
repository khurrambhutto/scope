//! Shared inspection limits, dispatch, and streamed read-only results.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::{apt, flatpak, local, targets, tools, DetailEffect, PackageDetails};
use crate::domain::{
    listing,
    package::{InstalledPackage, PackageSource},
    snap_details,
};

pub struct InspectionUpdate {
    pub details: PackageDetails,
    pub complete: bool,
}

pub(super) type Publish = Arc<dyn Fn(InspectionUpdate) + Send + Sync>;

pub(crate) async fn inspection_permit() -> Option<OwnedSemaphorePermit> {
    static WORKERS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let workers = WORKERS.get_or_init(|| Arc::new(Semaphore::new(2)));
    tokio::time::timeout(Duration::from_secs(1), workers.clone().acquire_owned())
        .await
        .ok()?
        .ok()
}

fn normalize(mut details: PackageDetails, removable: bool) -> PackageDetails {
    if !removable {
        for item in &mut details.items {
            if item.effect == DetailEffect::Removed {
                item.effect = DetailEffect::Informational;
            }
        }
    }
    details
}

pub async fn inspect_progressive(
    pkg: InstalledPackage,
    cancel: Arc<AtomicBool>,
    publish: impl Fn(InspectionUpdate) + Send + Sync + 'static,
) -> PackageDetails {
    let removable = listing::can_uninstall(&pkg);
    let publish: Publish = Arc::new(move |update| {
        publish(InspectionUpdate {
            details: normalize(update.details, removable),
            complete: update.complete,
        })
    });
    if cancel.load(Ordering::Relaxed) {
        return PackageDetails::unavailable("Inspection cancelled");
    }
    if pkg.source == PackageSource::Snap {
        let details = snap_details::inspect_progressive(&pkg.package_id, cancel, move |update| {
            publish(InspectionUpdate {
                details: PackageDetails::from(&update.details),
                complete: update.complete,
            });
        })
        .await;
        return normalize(PackageDetails::from(&details), removable);
    }
    let Some(permit) = inspection_permit().await else {
        return PackageDetails::unavailable("Inspection is busy. Reopen details to try again.");
    };
    if cancel.load(Ordering::Relaxed) {
        return PackageDetails::unavailable("Inspection cancelled");
    }
    let discovery = match pkg.source {
        PackageSource::Uv
        | PackageSource::Pipx
        | PackageSource::Cargo
        | PackageSource::Npm
        | PackageSource::Bun => tokio::task::spawn_blocking(move || tools::discover(&pkg))
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("Tool files could not be read"))),
        PackageSource::Apt => apt::discover(&pkg).await,
        PackageSource::Flatpak => flatpak::discover(&pkg).await,
        PackageSource::AppImage | PackageSource::Desktop => {
            tokio::task::spawn_blocking(move || local::discover(&pkg))
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Local files could not be read")))
        }
        PackageSource::Steam => {
            let mut discovery = targets::Discovery::default();
            discovery.unknown(
                "Game files",
                "Steam game footprints are not inspected by Scope",
            );
            discovery.unknown("User data", "Steam manages game data and saves");
            Ok(discovery)
        }
        PackageSource::JetBrainsToolbox => {
            let mut discovery = targets::Discovery::default();
            discovery.unknown(
                "IDE files",
                "JetBrains Toolbox manages the IDE installation",
            );
            discovery.unknown(
                "User data",
                "IDE settings and projects are not inspected by Scope",
            );
            Ok(discovery)
        }
        PackageSource::Snap => unreachable!("Snap inspection dispatched above"),
    };
    let discovery = match discovery {
        Ok(discovery) => discovery,
        Err(error) => return PackageDetails::unavailable(error.to_string()),
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        targets::measure(discovery, &cancel, &publish)
    })
    .await
    .unwrap_or_else(|_| PackageDetails::unavailable("Application sizes could not be measured"));
    normalize(result, removable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package_details::{DetailItem, Measurement};

    #[test]
    fn protected_footprints_never_promise_removal() {
        let details = PackageDetails {
            items: vec![DetailItem {
                label: "Application".into(),
                location: None,
                size: Measurement {
                    status: Default::default(),
                    bytes: Some(123),
                    complete: true,
                    note: None,
                },
                effect: DetailEffect::Removed,
            }],
            warnings: vec![],
        };
        let details = normalize(details, false);
        assert_eq!(details.items[0].effect, DetailEffect::Informational);
        assert_eq!(details.removal_size().bytes, None);
    }

    #[tokio::test]
    async fn steam_details_are_informational_and_do_not_claim_removal() {
        let pkg = InstalledPackage::new(PackageSource::Steam, "570");
        let details = inspect_progressive(pkg, Arc::new(AtomicBool::new(false)), |_| {}).await;
        assert_eq!(details.items.len(), 2);
        assert!(details
            .items
            .iter()
            .all(|item| item.effect == DetailEffect::Informational && item.size.bytes.is_none()));
        assert_eq!(details.removal_size().bytes, None);
    }

    #[tokio::test]
    async fn toolbox_details_are_informational_and_do_not_claim_removal() {
        let pkg = InstalledPackage::new(PackageSource::JetBrainsToolbox, "/toolbox/apps/pycharm");
        let details = inspect_progressive(pkg, Arc::new(AtomicBool::new(false)), |_| {}).await;
        assert_eq!(details.items.len(), 2);
        assert_eq!(details.items[0].label, "IDE files");
        assert_eq!(details.items[1].label, "User data");
        assert!(details
            .items
            .iter()
            .all(|item| item.effect == DetailEffect::Informational && item.size.bytes.is_none()));
        assert_eq!(details.removal_size().bytes, None);
    }

    #[tokio::test]
    #[ignore = "requires a current Scope cache with installed apps from every source; read-only"]
    async fn live_inspection_reads_every_installed_source() {
        let scan = crate::backend::read_cache().expect("Run Scope once to populate the cache");
        for source in [
            PackageSource::Apt,
            PackageSource::Snap,
            PackageSource::Flatpak,
            PackageSource::AppImage,
            PackageSource::Desktop,
        ] {
            let pkg = scan
                .packages
                .iter()
                .find(|pkg| pkg.source == source && listing::is_listable(pkg))
                .expect("Install a visible app from each source");
            let updates = Arc::new(std::sync::Mutex::new(0));
            let received = updates.clone();
            let details =
                inspect_progressive(pkg.clone(), Arc::new(AtomicBool::new(false)), move |_| {
                    *received.lock().unwrap() += 1;
                })
                .await;
            assert!(
                !details.items.is_empty(),
                "{source:?}: {:?}",
                details.warnings
            );
            assert!(
                details.items.iter().any(|item| item.size.bytes.is_some()),
                "{source:?}: no readable sizes"
            );
            assert!(
                *updates.lock().unwrap() > 0,
                "{source:?}: no progressive updates"
            );
        }
    }
}
