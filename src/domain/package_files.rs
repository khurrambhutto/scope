//! Read-only access to the installed-file lists maintained by dpkg.

use std::fs;
use std::path::{Path, PathBuf};

/// Files owned by an installed package, from dpkg's local metadata.
pub fn list(package: &str) -> Vec<PathBuf> {
    let info_dir = Path::new("/var/lib/dpkg/info");
    candidate_names(package)
        .into_iter()
        .find_map(|name| fs::read_to_string(info_dir.join(format!("{name}.list"))).ok())
        .map(|files| files.lines().map(PathBuf::from).collect())
        .unwrap_or_default()
}

fn candidate_names(package: &str) -> Vec<String> {
    let mut names = vec![package.to_string()];
    if let Some(unqualified) = package
        .strip_suffix(":amd64")
        .or_else(|| package.strip_suffix(":i386"))
    {
        names.push(unqualified.to_string());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn architecture_qualified_names_also_try_the_unqualified_file() {
        assert_eq!(
            candidate_names("libfoo:amd64"),
            vec!["libfoo:amd64".to_string(), "libfoo".to_string()]
        );
        assert_eq!(
            candidate_names("libfoo:i386"),
            vec!["libfoo:i386".to_string(), "libfoo".to_string()]
        );
        assert_eq!(candidate_names("firefox"), vec!["firefox".to_string()]);
    }
}
