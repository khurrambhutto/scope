use crate::domain::package::InstalledPackage;

#[derive(Default)]
pub(super) struct PackageListModel {
    pub entries: Vec<InstalledPackage>,
    keys: Vec<String>,
    next_scan: u64,
    active_scan: u64,
}

impl PackageListModel {
    pub fn replace(&mut self, entries: Vec<InstalledPackage>) -> bool {
        let keys = entries.iter().map(|package| package.key.clone()).collect();
        let keys_changed = keys != self.keys;
        self.entries = entries;
        self.keys = keys;
        keys_changed
    }

    pub fn begin_scan(&mut self) -> u64 {
        self.next_scan += 1;
        self.active_scan = self.next_scan;
        self.active_scan
    }

    pub fn accepts_scan(&self, request: u64) -> bool {
        request == self.active_scan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::PackageSource;

    #[test]
    fn replaces_package_data_even_when_keys_are_unchanged() {
        let mut model = PackageListModel::default();
        let mut old = InstalledPackage::new(PackageSource::Apt, "gimp");
        old.version = "1".into();
        assert!(model.replace(vec![old]));
        let mut new = InstalledPackage::new(PackageSource::Apt, "gimp");
        new.version = "2".into();
        assert!(!model.replace(vec![new]));
        assert_eq!(model.entries[0].version, "2");
    }

    #[test]
    fn rejects_a_stale_scan_completion() {
        let mut model = PackageListModel::default();
        let first = model.begin_scan();
        let second = model.begin_scan();
        assert!(!model.accepts_scan(first));
        assert!(model.accepts_scan(second));
    }
}
