//! Read-only snapd requests through its local socket. No store access or elevation.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::domain::system::capture_stdout;

#[derive(Deserialize)]
struct Envelope<T> {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "status-code")]
    status: u16,
    result: T,
}

#[derive(Deserialize)]
pub(super) struct Revision {
    pub name: String,
    pub revision: String,
    pub status: String,
    #[serde(rename = "installed-size")]
    pub size: Option<u64>,
    #[serde(rename = "mounted-from")]
    pub path: Option<String>,
    pub summary: Option<String>,
    pub confinement: Option<String>,
    #[serde(default)]
    pub trymode: bool,
    #[serde(default)]
    pub components: Vec<Component>,
}

#[derive(Deserialize)]
pub(super) struct Component {
    pub name: String,
    pub revision: Option<String>,
    #[serde(rename = "installed-size", default, deserialize_with = "optional_size")]
    pub size: Option<u64>,
}

fn optional_size<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_u64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("invalid size")),
        Some(serde_json::Value::String(number)) => {
            number.parse().map(Some).map_err(serde::de::Error::custom)
        }
        _ => Err(serde::de::Error::custom("invalid size")),
    }
}

#[derive(Deserialize)]
pub(super) struct SnapshotSet {
    pub id: u64,
    pub snapshots: Vec<Snapshot>,
}

#[derive(Deserialize)]
pub(super) struct Snapshot {
    pub snap: String,
    pub size: Option<u64>,
}

pub(super) async fn revisions(name: &str) -> Result<Vec<Revision>> {
    let url = format!("http://localhost/v2/snaps?select=all&snaps={name}");
    request(&url).await
}

pub(super) async fn snapshots() -> Result<Vec<SnapshotSet>> {
    request("http://localhost/v2/snapshots").await
}

async fn request<T: DeserializeOwned>(url: &str) -> Result<T> {
    let output = capture_stdout(
        "curl",
        &[
            "--disable",
            "--silent",
            "--show-error",
            "--fail",
            "--noproxy",
            "*",
            "--max-time",
            "6",
            "--unix-socket",
            "/run/snapd.socket",
            url,
        ],
        Duration::from_secs(8),
    )
    .await
    .context("Could not read local Snap metadata")?;
    parse(&output)
}

fn parse<T: DeserializeOwned>(output: &str) -> Result<T> {
    let envelope: Envelope<T> =
        serde_json::from_str(output).context("Invalid Snap metadata response")?;
    if envelope.kind != "sync" || envelope.status != 200 {
        bail!("Snap metadata request failed")
    }
    Ok(envelope.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_active_retained_and_component_sizes() {
        let revisions: Vec<Revision> = parse(r#"{"type":"sync","status-code":200,"result":[{"name":"code","revision":"1","status":"installed"},{"name":"code","revision":"2","status":"active","installed-size":42,"components":[{"name":"extra","revision":"3","installed-size":"12"}]}]}"#).unwrap();
        assert_eq!(revisions[0].size, None);
        assert_eq!(revisions[1].components[0].size, Some(12));
    }

    #[test]
    fn errors_and_truncated_responses_are_not_empty_inventory() {
        assert!(
            parse::<Vec<Revision>>(r#"{"type":"error","status-code":500,"result":[]}"#).is_err()
        );
        assert!(parse::<Vec<Revision>>(r#"{"type":"sync","status-code":200,"result":["#).is_err());
    }
}
