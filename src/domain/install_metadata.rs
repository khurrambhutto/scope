//! Optional app information for local-file install previews.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallMetadata {
    pub description: Option<String>,
    pub homepage: Option<String>,
    /// Preview assets live only as long as their issued plan and dialog.
    #[serde(skip)]
    pub icon: Option<Arc<PreviewIcon>>,
}

#[derive(Debug)]
pub struct PreviewIcon {
    path: PathBuf,
    _directory: tempfile::TempDir,
}

impl PartialEq for PreviewIcon {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for PreviewIcon {}

impl PreviewIcon {
    pub fn url(&self) -> String {
        crate::domain::icons::icon_url(&self.path)
    }
}

impl InstallMetadata {
    pub fn new(description: Option<&str>, homepage: Option<&str>) -> Self {
        let description = description
            .map(|text| {
                text.lines()
                    .map(str::trim)
                    .filter(|line| *line != "(none)")
                    .map(|line| if line == "." { "" } else { line })
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n')
                    .take(4096)
                    .collect::<String>()
            })
            .filter(|text| !text.is_empty());
        let homepage = homepage.and_then(|value| {
            let url = url::Url::parse(value.trim()).ok()?;
            (matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none())
            .then(|| url.to_string())
        });
        Self {
            description,
            homepage,
            icon: None,
        }
    }

    pub fn with_icon(mut self, bytes: &[u8], extension: &str) -> Self {
        if !matches!(extension, "png" | "svg") || bytes.len() > 4 * 1024 * 1024 {
            return self;
        }
        if let Ok(directory) = tempfile::Builder::new()
            .prefix("scope-install-icon-")
            .tempdir()
        {
            let path = directory.path().join(format!("icon.{extension}"));
            if std::fs::write(&path, bytes).is_ok() {
                self.icon = Some(Arc::new(PreviewIcon {
                    path,
                    _directory: directory,
                }));
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_descriptions_keep_paragraphs_without_debian_indentation() {
        let metadata =
            InstallMetadata::new(Some("Disk usage viewer\n .\n Find large files."), None);
        assert_eq!(
            metadata.description.as_deref(),
            Some("Disk usage viewer\n\nFind large files.")
        );
    }

    #[test]
    fn homepage_links_allow_only_web_urls_without_credentials() {
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.com",
            "steam://install/123",
            "not a url",
        ] {
            assert!(
                InstallMetadata::new(None, Some(url)).homepage.is_none(),
                "{url}"
            );
        }
        assert_eq!(
            InstallMetadata::new(None, Some("https://example.com/app"))
                .homepage
                .as_deref(),
            Some("https://example.com/app")
        );
    }

    #[test]
    fn preview_icons_stay_available_until_the_last_plan_copy_is_dropped() {
        let metadata = InstallMetadata::default().with_icon(b"\x89PNG\r\n\x1a\n", "png");
        let path = metadata.icon.as_ref().unwrap().path.clone();
        let copy = metadata.clone();
        drop(metadata);
        assert!(path.is_file());
        drop(copy);
        assert!(!path.exists());
    }
}
