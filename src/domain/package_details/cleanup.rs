//! Project the reviewed data discovery into read-only footprint rows.
use std::path::Path;

use super::{DetailEffect, DetailItem, Measurement, PackageDetails};
use crate::domain::app_data::{CleanupEffect, CleanupPlan};

pub(super) fn merge(details: &mut PackageDetails, plan: &CleanupPlan) {
    if !plan.items.is_empty() {
        for item in &mut details.items {
            if item.label == "User data" && item.location.is_none() {
                item.label = "Other user data".into();
                item.size = Measurement::descriptive(
                    super::MeasurementStatus::Unattributed,
                    "Other locations are unknown",
                );
            }
        }
    }
    for item in &plan.items {
        if item.effect == CleanupEffect::Included {
            continue;
        }
        let location = item.location_label();
        let existing = details
            .items
            .iter()
            .position(|row| row.location.as_deref() == Some(&location));
        if existing.is_some_and(|index| details.items[index].effect == DetailEffect::Removed) {
            continue;
        }
        // Never count a selectable child again when the manager already removes
        // its parent, or introduce overlapping data estimates.
        if !item.path.as_os_str().is_empty()
            && details.items.iter().any(|row| {
                row.location.as_ref().is_some_and(|path| {
                    Path::new(path) != item.path
                        && item.path.starts_with(path)
                        && matches!(row.effect, DetailEffect::Removed | DetailEffect::Review)
                })
            })
        {
            continue;
        }
        let row = DetailItem {
            label: item.label.clone(),
            location: Some(location),
            effect: if item.selectable() {
                DetailEffect::Review
            } else {
                DetailEffect::Kept
            },
            size: Measurement {
                status: if item.path.as_os_str().is_empty() {
                    super::MeasurementStatus::NotMeasured
                } else {
                    Default::default()
                },
                bytes: item.bytes,
                complete: item.bytes.is_some(),
                note: (!item.selectable()).then(|| item.reason.clone()),
            },
        };
        if let Some(index) = existing {
            details.items[index] = row;
        } else {
            details.items.push(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::app_data::CleanupItem;
    fn data(path: &str) -> CleanupItem {
        serde_json::from_value(serde_json::json!({
            "id": path, "label": "User data", "path": path,
            "bytes": 200, "effect": "Trash", "default_selected": false,
            "reason": "Association", "action": "Trash", "identity": null
        }))
        .unwrap()
    }
    fn native(path: &str, effect: DetailEffect) -> DetailItem {
        DetailItem {
            label: "User data".into(),
            location: Some(path.into()),
            effect,
            size: Measurement {
                status: Default::default(),
                bytes: Some(100),
                complete: true,
                note: None,
            },
        }
    }
    #[test]
    fn flatpak_profile_becomes_one_review_row_without_inflating_removal() {
        let mut details = PackageDetails {
            items: vec![native(
                "/home/u/.var/app/org.example.App",
                DetailEffect::Kept,
            )],
            warnings: vec![],
        };
        let mut plan = CleanupPlan::default();
        plan.items.push(data("/home/u/.var/app/org.example.App"));
        merge(&mut details, &plan);
        assert_eq!(details.items.len(), 1);
        assert_eq!(details.items[0].effect, DetailEffect::Review);
        assert_eq!(details.removal_size().bytes, None);
    }
    #[test]
    fn native_parent_and_duplicate_paths_never_double_count_data() {
        let mut details = PackageDetails {
            items: vec![native("/home/u/snap/app", DetailEffect::Removed)],
            warnings: vec![],
        };
        let mut plan = CleanupPlan::default();
        plan.items.extend([
            data("/home/u/snap/app/cache"),
            data("/home/u/.config/App"),
            data("/home/u/.config/App"),
        ]);
        merge(&mut details, &plan);
        assert_eq!(details.items.len(), 2);
        assert_eq!(details.removal_size().bytes, Some(100));
        assert_eq!(details.review_size().bytes, Some(200));
    }

    #[test]
    fn permission_records_do_not_make_a_complete_storage_estimate_partial() {
        let mut details = PackageDetails::default();
        let mut plan = CleanupPlan::default();
        let mut permissions = data("");
        permissions.label = "Saved Flatpak permissions".into();
        permissions.bytes = None;
        permissions.effect = CleanupEffect::Native;
        plan.items
            .extend([data("/home/u/.var/app/org.example.App"), permissions]);
        merge(&mut details, &plan);
        assert_eq!(details.review_size().bytes, Some(200));
        assert!(details.review_size().complete);
    }
}
