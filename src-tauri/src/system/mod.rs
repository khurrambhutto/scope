//! Shared command execution, timeouts, and environment helpers.
//!
//! All package-manager access goes through here so timeouts and error handling
//! stay consistent. The frontend never runs shell commands; the backend uses
//! typed `std`/`tokio` `Command` invocations with explicit argv — never
//! `sh -c` with frontend-provided strings.

use std::time::Duration;
use tokio::process::Command;

/// Capture stdout of a command as a UTF-8 string, with a timeout.
///
/// Returns the trimmed stdout on success. If the command is missing, exits
/// non-zero, or exceeds the timeout, this returns `Err` with a readable cause.
pub async fn capture_stdout(program: &str, args: &[&str], timeout: Duration) -> anyhow::Result<String> {
    let output = tokio::time::timeout(timeout, Command::new(program).args(args).output()).await;
    match output {
        Ok(Ok(out)) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        Ok(Ok(out)) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            anyhow::bail!("{program} failed (exit {:?}): {stderr}", out.status.code())
        }
        Ok(Err(e)) => anyhow::bail!("failed to spawn {program}: {e}"),
        Err(_) => anyhow::bail!("{program} timed out after {timeout:?}"),
    }
}

/// Whether a binary exists on `PATH`. Cheap availability probe for scanners.
pub fn which(program: &str) -> bool {
    which_lookup(program).is_some()
}

fn which_lookup(program: &str) -> Option<std::path::PathBuf> {
    // Prefer absolute / well-known locations, fall back to PATH search.
    if let Some(home) = std::env::var_os("HOME") {
        let candidate = std::path::Path::new(&home).join(".local/bin").join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for dir in ["/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        let candidate = std::path::Path::new(dir).join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths).find_map(|dir| {
                let candidate = dir.join(program);
                candidate.is_file().then_some(candidate)
            })
        })
}

/// Default per-command timeout for package scans.
pub const SCAN_TIMEOUT: Duration = Duration::from_secs(30);