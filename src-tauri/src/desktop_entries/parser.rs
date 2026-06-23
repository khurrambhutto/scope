//! Minimal `.desktop` (INI-ish) parser for the fields Scope needs.

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A parsed, visible GUI app entry.
#[derive(Debug, Clone, Serialize)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub terminal: bool,
    /// `NoDisplay=true` entries are skipped by the discoverer but kept here for
    /// internal lookups (we filter them out before sorting).
    pub no_display: bool,
}

pub fn parse(id: &str, path: &Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut sections: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut current = String::new();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            current = rest.to_string();
            sections.entry(current.clone()).or_default();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(current.clone())
                .or_default()
                .push((key.trim().to_string(), value.trim().to_string()));
        }
    }

    let entry = sections
        .iter()
        .find(|(name, _)| name.as_str() == "Desktop Entry")
        .map(|(_, kv)| kv)?;

    // Locale-suffixed keys (e.g. `Name[en]`) — pick the bare one, else the first
    // locale variant as a fallback.
    fn field<'a>(entry: &'a [(String, String)], key: &str) -> Option<&'a str> {
        if let Some((_, v)) = entry.iter().find(|(k, _)| k == key) {
            return Some(v.as_str());
        }
        for (k, v) in entry {
            if let Some(rest) = k.strip_prefix(&format!("{key}[")) {
                if rest.ends_with(']') {
                    return Some(v.as_str());
                }
            }
        }
        None
    }

    let type_ = field(entry, "Type").unwrap_or("Application");
    if type_ != "Application" {
        return None;
    }

    let exec_raw = field(entry, "Exec").unwrap_or("").to_string();
    // Only `Application` entries with an Exec are useful here; Link types etc. lack one.
    if exec_raw.is_empty() {
        return None;
    }

    Some(DesktopApp {
        id: id.to_string(),
        name: field(entry, "Name").unwrap_or(id).to_string(),
        generic_name: field(entry, "GenericName").map(str::to_string),
        comment: field(entry, "Comment").map(str::to_string),
        exec: exec_raw,
        icon: field(entry, "Icon").map(str::to_string),
        categories: field(entry, "Categories")
            .unwrap_or("")
            .split(';')
            .filter(|c| !c.is_empty())
            .map(str::to_string)
            .collect(),
        keywords: field(entry, "Keywords")
            .unwrap_or("")
            .split(';')
            .filter(|c| !c.is_empty())
            .map(str::to_string)
            .collect(),
        terminal: field(entry, "Terminal").map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false),
        no_display: field(entry, "NoDisplay").map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false),
    })
}