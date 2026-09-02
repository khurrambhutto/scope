//! Shared command execution, timeouts, privilege escalation, and environment helpers.
//!
//! All package-manager access goes through here so timeouts and error handling
//! stay consistent. The frontend never runs shell commands; the backend uses
//! typed `std`/`tokio` `Command` invocations with explicit argv — never
//! `sh -c` with frontend-provided strings.

use std::time::Duration;

use tokio::process::Command;

use crate::operations::AuthMethod;
use crate::operations::OperationResult;

/// Capture stdout of a command as a UTF-8 string, with a timeout.
///
/// Returns the trimmed stdout on success. If the command is missing, exits
/// non-zero, or exceeds the timeout, this returns `Err` with a readable cause.
pub async fn capture_stdout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> anyhow::Result<String> {
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
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(program);
            candidate.is_file().then_some(candidate)
        })
    })
}

/// Default per-command timeout for package scans.
pub const SCAN_TIMEOUT: Duration = Duration::from_secs(30);

/// Resolve a binary to an absolute path (pkexec and env cleanup prefer this).
pub fn abs(bin: &str) -> String {
    for dir in ["/usr/bin", "/bin", "/usr/local/bin", "/usr/sbin", "/sbin"] {
        let p = format!("{dir}/{bin}");
        if std::path::Path::new(&p).is_file() {
            return p;
        }
    }
    bin.to_string()
}

/// Run a command with an optional `pkexec env DEBIAN_FRONTEND=noninteractive`
/// prefix, capturing combined output and enforcing a timeout.
///
/// This is shared between uninstall and update operations.
pub async fn run_elevated(
    program: &str,
    args: &[&str],
    auth: AuthMethod,
    timeout: Duration,
) -> OperationResult {
    let program_abs = abs(program);
    let mut argv: Vec<String> = Vec::new();
    let display_program: String;
    match auth {
        AuthMethod::Pkexec => {
            argv.push("env".into());
            argv.push("DEBIAN_FRONTEND=noninteractive".into());
            argv.push(program_abs.clone());
            display_program = format!("pkexec env DEBIAN_FRONTEND=noninteractive {program_abs}");
        }
        AuthMethod::None => {
            display_program = program_abs.clone();
        }
    }
    for a in args {
        argv.push((*a).to_string());
    }
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();

    let mut cmd = match auth {
        AuthMethod::Pkexec => {
            let mut c = tokio::process::Command::new("pkexec");
            c.args(&argv_refs);
            c
        }
        AuthMethod::None => {
            let mut c = tokio::process::Command::new(&program_abs);
            c.args(
                &argv[if matches!(auth, AuthMethod::Pkexec) {
                    3
                } else {
                    0
                }..],
            );
            c
        }
    };

    let started = std::time::Instant::now();
    let output = tokio::time::timeout(timeout, cmd.output()).await;

    let elapsed = started.elapsed();
    let logs_suffix = format!(
        "\n[scope] ran: {} {:?} ({}ms)",
        display_program,
        args,
        elapsed.as_millis()
    );

    match output {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let logs = format!("--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}{logs_suffix}");
            let success = out.status.success();
            let exit_code = out.status.code();
            let op_label = if program == "apt" {
                "Operation"
            } else {
                "Operation"
            };
            let message = if success {
                format!("{op_label} completed successfully.")
            } else {
                let first_err = stderr
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("command failed");
                format!("{op_label} failed (exit {:?}): {}", exit_code, first_err)
            };
            OperationResult {
                success,
                message,
                logs,
                exit_code,
            }
        }
        Ok(Err(e)) => OperationResult {
            success: false,
            message: format!("Failed to start command: {e}"),
            logs: format!("spawn error: {e}{logs_suffix}"),
            exit_code: None,
        },
        Err(_) => OperationResult {
            success: false,
            message: format!("Operation timed out after {timeout:?}."),
            logs: format!("timed out after {timeout:?}{logs_suffix}"),
            exit_code: None,
        },
    }
}
