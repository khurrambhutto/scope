use crate::domain::package::InstalledPackage;

const CHECKING: &str = "Checking for updates...";

pub(super) fn checked(package: &mut InstalledPackage) {
    if package.update_note.as_deref() == Some(CHECKING) {
        package.update_note = None;
    }
}

pub(super) fn unavailable(
    package: &mut InstalledPackage,
    warning: &mut Option<String>,
    message: &str,
) {
    package.update_note = Some(message.to_string());
    append_warning(warning, message);
}

pub(super) fn append_warning(warning: &mut Option<String>, message: &str) {
    match warning {
        Some(existing) if !existing.contains(message) => {
            existing.push_str("; ");
            existing.push_str(message);
        }
        Some(_) => {}
        None => *warning = Some(message.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::{InstalledPackage, PackageSource};
    use crate::domain::scanner::ScanReport;

    fn pending_package() -> InstalledPackage {
        let mut package = InstalledPackage::new(PackageSource::Apt, "fixture");
        package.update_note = Some(CHECKING.into());
        package
    }

    #[test]
    fn failed_update_check_marks_row_unknown_and_adds_a_source_warning() {
        let mut report = ScanReport::ok(vec![pending_package()]);

        unavailable(
            &mut report.packages[0],
            &mut report.warning,
            "APT update check failed: command timed out",
        );

        assert_eq!(
            report.packages[0].update_note.as_deref(),
            Some("APT update check failed: command timed out")
        );
        assert_eq!(
            report.warning.as_deref(),
            Some("APT update check failed: command timed out")
        );
        assert!(!report.packages[0].has_update);
    }

    #[test]
    fn successful_check_with_no_updates_clears_the_pending_note() {
        let mut package = pending_package();

        checked(&mut package);

        assert!(package.update_note.is_none());
        assert!(!package.has_update);
    }

    #[test]
    fn update_warning_preserves_an_existing_inventory_warning() {
        let mut report = ScanReport {
            packages: vec![pending_package()],
            warning: Some("Flatpak system scan failed".into()),
        };

        unavailable(
            &mut report.packages[0],
            &mut report.warning,
            "Flatpak user update check failed",
        );

        assert_eq!(
            report.warning.as_deref(),
            Some("Flatpak system scan failed; Flatpak user update check failed")
        );
    }
}
