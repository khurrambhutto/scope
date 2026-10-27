//! Dismiss scan warnings without changing source availability or package details.

use std::collections::HashMap;

use crate::domain::scanner::ScanAvailability;

#[derive(Default)]
pub(super) struct ScanNotices {
    dismissed: HashMap<&'static str, String>,
}

impl ScanNotices {
    pub(super) fn reset(&mut self) {
        self.dismissed.clear();
    }

    pub(super) fn dismiss(&mut self, source: &'static str, message: String) {
        self.dismissed.insert(source, message);
    }

    pub(super) fn visible<'a>(
        &self,
        availability: &'a ScanAvailability,
    ) -> Vec<(&'static str, &'a str)> {
        [
            ("APT", availability.apt_error.as_deref()),
            ("Snap", availability.snap_error.as_deref()),
            ("Flatpak", availability.flatpak_error.as_deref()),
            ("Steam", availability.steam_error.as_deref()),
            ("uv", availability.uv_error.as_deref()),
            ("pipx", availability.pipx_error.as_deref()),
            ("Cargo", availability.cargo_error.as_deref()),
            ("npm", availability.npm_error.as_deref()),
            ("Bun", availability.bun_error.as_deref()),
        ]
        .into_iter()
        .filter_map(|(source, message)| {
            let message = message?;
            (self.dismissed.get(source).map(String::as_str) != Some(message))
                .then_some((source, message))
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_source_warning_can_be_dismissed_independently() {
        let availability = ScanAvailability {
            apt_error: Some("apt failed".into()),
            snap_error: Some("snap failed".into()),
            flatpak_error: Some("flatpak failed".into()),
            steam_error: Some("steam failed".into()),
            uv_error: Some("uv failed".into()),
            pipx_error: Some("pipx failed".into()),
            cargo_error: Some("cargo failed".into()),
            npm_error: Some("npm failed".into()),
            bun_error: Some("bun failed".into()),
            ..Default::default()
        };
        let mut notices = ScanNotices::default();
        let warnings = notices.visible(&availability);
        assert_eq!(warnings.len(), 9);
        for (index, (source, message)) in warnings.into_iter().enumerate() {
            notices.dismiss(source, message.to_string());
            let remaining = notices.visible(&availability);
            assert_eq!(remaining.len(), 8 - index);
            assert!(!remaining.iter().any(|(label, _)| *label == source));
        }
        assert_eq!(availability.snap_error.as_deref(), Some("snap failed"));
    }

    #[test]
    fn dismissed_warning_stays_hidden_when_scan_results_are_republished() {
        let availability = ScanAvailability {
            snap_error: Some("Store timed out".into()),
            ..Default::default()
        };
        let mut notices = ScanNotices::default();
        notices.dismiss("Snap", "Store timed out".into());
        assert!(notices.visible(&availability.clone()).is_empty());
    }

    #[test]
    fn changed_warning_is_visible_even_after_dismissal() {
        let mut notices = ScanNotices::default();
        notices.dismiss("Snap", "Store timed out".into());
        let availability = ScanAvailability {
            snap_error: Some("Could not run snap".into()),
            ..Default::default()
        };
        assert_eq!(
            notices.visible(&availability),
            vec![("Snap", "Could not run snap")]
        );
    }

    #[test]
    fn rescan_allows_the_same_warning_to_appear_again() {
        let mut notices = ScanNotices::default();
        notices.dismiss("npm", "Registry unavailable".into());
        notices.reset();
        let availability = ScanAvailability {
            npm_error: Some("Registry unavailable".into()),
            ..Default::default()
        };
        assert_eq!(
            notices.visible(&availability),
            vec![("npm", "Registry unavailable")]
        );
    }
}
