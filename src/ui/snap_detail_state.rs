//! Request identity and short-lived cache for lazy Snap inspection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::domain::package::InstalledPackage;
use crate::domain::snap_details::SnapDetails;

const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(super) struct SnapDetailState {
    generation: u64,
    cancel: Option<Arc<AtomicBool>>,
    cache: HashMap<(String, String), (Instant, SnapDetails)>,
}

pub(super) struct DetailRequest {
    pub generation: u64,
    pub cancel: Arc<AtomicBool>,
}

impl SnapDetailState {
    pub fn cancel(&mut self) {
        self.generation += 1;
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn clear(&mut self) {
        self.cancel();
        self.cache.clear();
    }

    pub fn begin(&mut self, pkg: &InstalledPackage) -> Option<DetailRequest> {
        self.cancel();
        self.cache.retain(|_, (created, details)| {
            created.elapsed() < CACHE_TTL && !details.items.is_empty()
        });
        if self.get(pkg).is_some() {
            return None;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        Some(DetailRequest {
            generation: self.generation,
            cancel,
        })
    }

    pub fn accept(
        &mut self,
        generation: u64,
        pkg: &InstalledPackage,
        details: SnapDetails,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        if self.cache.len() >= 32 {
            self.cache.clear();
        }
        self.cache.insert(
            (pkg.key.clone(), pkg.version.clone()),
            (Instant::now(), details),
        );
        self.cancel = None;
        true
    }

    pub fn get(&self, pkg: &InstalledPackage) -> Option<&SnapDetails> {
        // Cache expiry is checked on opening; keep an already displayed result stable.
        self.cache
            .get(&(pkg.key.clone(), pkg.version.clone()))
            .map(|(_, details)| details)
    }
}

impl Drop for SnapDetailState {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::PackageSource;

    fn details() -> SnapDetails {
        SnapDetails {
            summary: None,
            items: vec![crate::domain::snap_details::DetailItem {
                label: "Revision 1".into(),
                location: None,
                size: crate::domain::snap_details::Measurement {
                    bytes: Some(1),
                    complete: true,
                    note: None,
                },
                effect: crate::domain::snap_details::RemovalEffect::RemovedBySnap,
            }],
            warnings: vec![],
        }
    }

    #[test]
    fn late_results_cannot_replace_new_selection() {
        let mut state = SnapDetailState::default();
        let first = InstalledPackage::new(PackageSource::Snap, "code");
        let second = InstalledPackage::new(PackageSource::Snap, "firefox");
        let old = state.begin(&first).unwrap();
        let new = state.begin(&second).unwrap();
        assert!(old.cancel.load(Ordering::Relaxed));
        assert!(!state.accept(old.generation, &first, details()));
        assert!(state.accept(new.generation, &second, details()));
        assert!(state.get(&first).is_none());
    }

    #[test]
    fn scan_invalidation_rejects_in_flight_results_and_clears_cache() {
        let mut state = SnapDetailState::default();
        let pkg = InstalledPackage::new(PackageSource::Snap, "code");
        let request = state.begin(&pkg).unwrap();
        state.clear();
        assert!(!state.accept(request.generation, &pkg, details()));
        let request = state.begin(&pkg).unwrap();
        assert!(state.accept(request.generation, &pkg, details()));
        assert!(state.begin(&pkg).is_none());
        state.clear();
        assert!(state.get(&pkg).is_none());
    }
}
