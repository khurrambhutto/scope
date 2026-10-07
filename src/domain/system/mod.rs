//! Shared command execution, timeouts, privilege escalation, and environment helpers.
//!
//! All package-manager access goes through here so timeouts and error handling
//! stay consistent. The frontend never runs shell commands; the backend uses
//! typed `std`/`tokio` `Command` invocations with explicit argv — never
//! `sh -c` with frontend-provided strings.

use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::domain::operations::AuthMethod;
use crate::domain::operations::OperationResult;

const MAX_CAPTURE_BYTES: usize = 2 * 1024 * 1024;
const MAX_LOG_BYTES: usize = 1024 * 1024;
const MAX_LINE_BYTES: usize = 16 * 1024;
const MAX_STREAM_LINES: usize = 2_000;
const TRUNCATED: &str = "\n[scope] output truncated\n";

/// Failure of a command Scope tried to run.
///
/// A typed error (rather than a formatted `anyhow` string) lets callers
/// distinguish "the tool isn't installed" from "it timed out" from "it exited
/// non-zero", which is what per-source availability reporting needs.
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("failed to spawn {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{program} timed out after {timeout:?}")]
    Timeout { program: String, timeout: Duration },
    #[error("{program} failed (exit {exit_code:?}): {stderr}")]
    NonZero {
        program: String,
        exit_code: Option<i32>,
        stderr: String,
    },
}

/// Captured result of a finished command, including non-zero exits.
///
/// Probe callers need to see *how* a command failed (e.g. "package not
/// installed" vs "package manager broken"), so unlike [`capture_stdout`] this
/// returns the output instead of turning it into an error.
#[derive(Debug)]
pub struct ProcessOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Run a command and capture its full output with a timeout.
///
/// `Err` is reserved for spawn failures and timeouts; a non-zero exit is
/// reported in the returned [`ProcessOutput`] for the caller to interpret.
pub async fn capture_output(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<ProcessOutput, SystemError> {
    capture_output_env(program, args, &[], timeout).await
}

pub async fn capture_output_env(
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    timeout: Duration,
) -> Result<ProcessOutput, SystemError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .envs(envs.iter().copied())
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|source| SystemError::Spawn {
        program: program.to_string(),
        source,
    })?;
    let stdout = child.stdout.take().expect("piped stdout is available");
    let stderr = child.stderr.take().expect("piped stderr is available");
    let stdout_task = tokio::spawn(read_limited(stdout, MAX_CAPTURE_BYTES));
    let stderr_task = tokio::spawn(read_limited(stderr, MAX_CAPTURE_BYTES));
    let status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(status) => status.map_err(|source| SystemError::Spawn {
            program: program.to_string(),
            source,
        })?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(SystemError::Timeout {
                program: program.to_string(),
                timeout,
            });
        }
    };
    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();
    Ok(ProcessOutput {
        success: status.success(),
        exit_code: status.code(),
        stdout,
        stderr,
    })
}

async fn read_limited<R>(mut reader: R, limit: usize) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut kept = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];
    while let Ok(read) = reader.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..read.min(remaining)]);
        truncated |= read > remaining;
    }
    let mut output = String::from_utf8_lossy(&kept).into_owned();
    if truncated {
        output.push_str(TRUNCATED);
    }
    output
}

/// Capture stdout of a command as a UTF-8 string, with a timeout.
///
/// Returns the stdout on success. If the command is missing, exits
/// non-zero, or exceeds the timeout, this returns a typed [`SystemError`].
pub async fn capture_stdout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<String, SystemError> {
    let out = capture_output(program, args, timeout).await?;
    if out.success {
        Ok(out.stdout)
    } else {
        Err(SystemError::NonZero {
            program: program.to_string(),
            exit_code: out.exit_code,
            stderr: out.stderr.trim().to_string(),
        })
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

/// Build the command for an operation, honouring the auth method.
///
/// Returns the prepared [`Command`] (with `pkexec env
/// DEBIAN_FRONTEND=noninteractive` prefix when elevated), a human-readable
/// command string for logs, and the argv used.
fn elevated_command(
    program: &str,
    args: &[&str],
    auth: AuthMethod,
) -> (Command, String, Vec<String>) {
    let program_abs = abs(program);
    let mut argv: Vec<String> = Vec::new();
    let display_program = match auth {
        AuthMethod::Pkexec => {
            argv.push("env".into());
            argv.push("DEBIAN_FRONTEND=noninteractive".into());
            argv.push(program_abs.clone());
            format!("pkexec env DEBIAN_FRONTEND=noninteractive {program_abs}")
        }
        AuthMethod::None => program_abs.clone(),
    };
    for a in args {
        argv.push((*a).to_string());
    }

    let cmd = {
        let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        match auth {
            AuthMethod::Pkexec => {
                let mut c = Command::new("pkexec");
                c.args(&argv_refs);
                c
            }
            AuthMethod::None => {
                let mut c = Command::new(&program_abs);
                c.args(&argv_refs);
                c
            }
        }
    };
    (cmd, display_program, argv)
}

/// Read a child's stdout/stderr and forward it line-by-line.
///
/// Splits on both `\n` and `\r`: package managers draw progress bars by
/// overwriting with carriage returns, and a `\n`-only reader would hide all of
/// it until the command finishes. `\r\n` collapses to a single break.
async fn pump<R>(mut reader: R, is_stderr: bool, tx: tokio::sync::mpsc::Sender<(bool, String)>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut buf = [0u8; 4096];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                for &byte in &buf[..n] {
                    if byte == b'\n' || byte == b'\r' {
                        if !pending.is_empty() {
                            let line = String::from_utf8_lossy(&pending).to_string();
                            if tx.send((is_stderr, line)).await.is_err() {
                                return;
                            }
                            pending.clear();
                        }
                    } else if pending.len() < MAX_LINE_BYTES {
                        pending.push(byte);
                    }
                }
            }
            Err(_) => break,
        }
    }
    if !pending.is_empty() {
        let line = String::from_utf8_lossy(&pending).to_string();
        let _ = tx.send((is_stderr, line)).await;
    }
}

/// Run a command with an optional `pkexec env DEBIAN_FRONTEND=noninteractive`
/// prefix, streaming each output line to `on_line` as it is produced, then
/// capturing the combined output and enforcing a timeout.
///
/// stdout and stderr are read concurrently and interleaved; the final
/// [`OperationResult::logs`] still separates them. `on_line` is called from the
/// async runtime thread and must not block.
///
/// This is shared between uninstall and update operations. The frontend uses
/// the streamed lines to show live progress during long package-manager runs.
pub async fn run_elevated(
    program: &str,
    args: &[&str],
    auth: AuthMethod,
    timeout: Duration,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let (mut cmd, display_program, argv) = elevated_command(program, args, auth);
    cmd.kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let started = std::time::Instant::now();
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return OperationResult {
                success: false,
                message: format!("Failed to start command: {e}"),
                logs: format!("spawn error: {e}\n[scope] ran: {display_program} {argv:?}"),
                exit_code: None,
            };
        }
    };

    // Seed the live log so the panel is never blank while the command (or a
    // Polkit password prompt) is pending.
    if args.is_empty() {
        on_line(&format!("[scope] Running: {display_program}"));
    } else {
        on_line(&format!(
            "[scope] Running: {display_program} {}",
            args.join(" ")
        ));
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<(bool, String)>(512);
    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(pump(stdout, false, tx));
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        tokio::spawn(pump(stderr, true, tx));
    }
    drop(tx);

    let mut stdout_log = String::new();
    let mut stderr_log = String::new();
    let mut status: Option<std::io::Result<std::process::ExitStatus>> = None;
    let mut readers_done = false;
    let mut streamed_lines = 0_usize;

    let drain = async {
        loop {
            tokio::select! {
                maybe = rx.recv(), if !readers_done => {
                    match maybe {
                        Some((is_stderr, line)) => {
                            if streamed_lines < MAX_STREAM_LINES {
                                on_line(&line);
                                streamed_lines += 1;
                            }
                            let buf = if is_stderr { &mut stderr_log } else { &mut stdout_log };
                            if buf.len() < MAX_LOG_BYTES {
                                let remaining = MAX_LOG_BYTES - buf.len();
                                buf.push_str(&String::from_utf8_lossy(
                                    &line.as_bytes()[..line.len().min(remaining)],
                                ));
                                buf.push('\n');
                            }
                        }
                        None => readers_done = true,
                    }
                }
                res = child.wait(), if status.is_none() => {
                    status = Some(res);
                }
                else => break,
            }
        }
    };

    let timed_out = tokio::time::timeout(timeout, drain).await.is_err();
    if timed_out {
        let _ = child.kill().await;
    }
    if status.is_none() {
        status = Some(child.wait().await);
    }

    let elapsed = started.elapsed();
    let logs_suffix = format!(
        "\n[scope] ran: {} {:?} ({}ms)",
        display_program,
        args,
        elapsed.as_millis()
    );
    let logs = format!("--- stdout ---\n{stdout_log}--- stderr ---\n{stderr_log}{logs_suffix}");

    if timed_out {
        return OperationResult {
            success: false,
            message: format!("Operation timed out after {timeout:?}."),
            logs: format!("timed out after {timeout:?}{logs_suffix}"),
            exit_code: None,
        };
    }

    match status {
        Some(Ok(out)) => {
            let success = out.success();
            let exit_code = out.code();
            let message = if success {
                "Operation completed successfully.".to_string()
            } else {
                let first_err = stderr_log
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("command failed");
                format!("Operation failed (exit {exit_code:?}): {first_err}")
            };
            OperationResult {
                success,
                message,
                logs,
                exit_code,
            }
        }
        Some(Err(e)) => OperationResult {
            success: false,
            message: format!("Command failed: {e}"),
            logs: format!("wait error: {e}{logs_suffix}"),
            exit_code: None,
        },
        None => OperationResult {
            success: false,
            message: "Command did not run.".into(),
            logs: logs_suffix,
            exit_code: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[tokio::test]
    async fn streams_lines_and_captures_logs() {
        let seen = Mutex::new(Vec::new());
        let sink = |line: &str| seen.lock().unwrap().push(line.to_string());

        let out = run_elevated(
            "echo",
            &["hello", "world"],
            AuthMethod::None,
            Duration::from_secs(5),
            &sink,
        )
        .await;

        assert!(out.success, "echo should succeed: {}", out.message);
        assert!(out.logs.contains("hello world"));
        let seen = seen.into_inner().unwrap();
        assert!(
            seen.iter().any(|l| l == "hello world"),
            "streamed lines: {seen:?}"
        );
    }

    #[tokio::test]
    async fn streams_carriage_return_progress() {
        let seen = Mutex::new(Vec::new());
        let sink = |line: &str| seen.lock().unwrap().push(line.to_string());

        // `printf "a\rb\n"` mimics a progress bar: two updates split by CR.
        let out = run_elevated(
            "printf",
            &["a\\rb\\n"],
            AuthMethod::None,
            Duration::from_secs(5),
            &sink,
        )
        .await;

        assert!(out.success, "printf should succeed: {}", out.message);
        let seen = seen.into_inner().unwrap();
        assert!(seen.iter().any(|l| l == "a"), "streamed lines: {seen:?}");
        assert!(seen.iter().any(|l| l == "b"), "streamed lines: {seen:?}");
    }

    #[tokio::test]
    async fn times_out_long_running_commands() {
        let out = run_elevated(
            "sleep",
            &["5"],
            AuthMethod::None,
            Duration::from_millis(200),
            &|_| {},
        )
        .await;

        assert!(!out.success);
        assert!(
            out.message.contains("timed out"),
            "message: {}",
            out.message
        );
    }

    #[tokio::test]
    async fn capture_stdout_reports_a_nonzero_exit() {
        let err = capture_stdout("false", &[], Duration::from_secs(5))
            .await
            .unwrap_err();
        assert!(matches!(err, SystemError::NonZero { .. }), "got {err}");
    }

    #[tokio::test]
    async fn capture_output_reports_a_spawn_failure() {
        let err = capture_output("scope-no-such-binary", &[], Duration::from_secs(5))
            .await
            .unwrap_err();
        assert!(matches!(err, SystemError::Spawn { .. }), "got {err}");
    }

    #[tokio::test]
    async fn capture_output_reports_a_timeout() {
        let err = capture_output("sleep", &["5"], Duration::from_millis(100))
            .await
            .unwrap_err();
        assert!(matches!(err, SystemError::Timeout { .. }), "got {err}");
    }

    #[tokio::test]
    async fn capture_output_bounds_large_stdout() {
        let output = capture_output("seq", &["1", "1000000"], Duration::from_secs(5))
            .await
            .unwrap();
        assert!(output.stdout.len() <= MAX_CAPTURE_BYTES + TRUNCATED.len());
        assert!(output.stdout.ends_with(TRUNCATED));
    }

    #[tokio::test]
    async fn timeout_kills_and_reaps_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let script = format!("echo $$ > {}; exec sleep 30", pid_file.display());
        let error = capture_output("sh", &["-c", &script], Duration::from_millis(200))
            .await
            .unwrap_err();
        assert!(matches!(error, SystemError::Timeout { .. }));
        let pid = std::fs::read_to_string(pid_file).unwrap();
        let status = std::process::Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "timed-out child {pid} is still alive");
    }
}
