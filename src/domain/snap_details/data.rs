//! Known Snap data locations. Alternative layouts need explicit coverage labels.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::{measurement, DetailItem, Measurement, RemovalEffect};

pub(super) fn items(home: Option<&Path>, name: &str, cancel: &AtomicBool) -> Vec<DetailItem> {
    let mut items = Vec::new();
    if let Some(home) = home {
        let standard = home.join("snap").join(name);
        let hidden = home.join(".snap/data").join(name);
        let hidden_exists = match hidden.symlink_metadata() {
            Ok(_) => true,
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        };
        // Hidden data is an experimental snapd layout. The active per-snap
        // migration policy is not exposed by our unauthenticated metadata read.
        if hidden_exists {
            if standard.symlink_metadata().is_ok() {
                items.push(item(
                    "Your default data directory",
                    &standard,
                    RemovalEffect::Unverified,
                    cancel,
                ));
            }
            let mut hidden_item = item(
                "Your hidden data directory",
                &hidden,
                RemovalEffect::Unverified,
                cancel,
            );
            let measured_note = hidden_item.size.note.take().unwrap_or_default();
            hidden_item.size.note = Some(format!(
                "Alternative Snap layout; removal coverage could not be verified. {measured_note}"
            ));
            items.push(hidden_item);
        } else {
            items.push(item(
                "Your managed data",
                &standard,
                RemovalEffect::RemovedBySnap,
                cancel,
            ));
        }
    } else {
        items.push(DetailItem {
            label: "Your managed data".into(),
            location: None,
            size: Measurement::unknown("Home directory unavailable"),
            effect: RemovalEffect::Unverified,
        });
    }
    items.push(item(
        "System managed data",
        &Path::new("/var/snap").join(name),
        RemovalEffect::RemovedBySnap,
        cancel,
    ));
    items
}

fn item(label: &str, path: &Path, effect: RemovalEffect, cancel: &AtomicBool) -> DetailItem {
    DetailItem {
        label: label.into(),
        location: Some(path.to_string_lossy().into_owned()),
        size: measurement::measure(path, cancel),
        effect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternative_layout_is_measured_without_promising_removal() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".snap/data/code")).unwrap();
        let items = items(Some(home.path()), "code", &AtomicBool::new(false));
        assert_eq!(items[0].effect, RemovalEffect::Unverified);
        assert!(items[0].size.bytes.is_some());
        assert_eq!(items[0].label, "Your hidden data directory");
    }
}
