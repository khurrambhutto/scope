//! Re-run command tests under a controlling terminal without changing host packages.

use std::io::IsTerminal as _;
use std::process::Stdio;

use super::*;

const FIXTURE_ENV: &str = "SCOPE_TERMINAL_TEST";
const TERMINAL_COMMAND: &str = "if test -t 0; then stty -echo; fi; printf 'operation-finished\\n'";

fn in_terminal_fixture(test: &str) -> bool {
    if std::env::var(FIXTURE_ENV).as_deref() == Ok(test) {
        assert!(std::io::stdin().is_terminal());
        return true;
    }

    // script supplies a controlling terminal; the nested test exercises Scope's
    // real background process group, including SIGTTOU during terminal changes.
    let output = std::process::Command::new("script")
        .args([
            "--quiet",
            "--return",
            "--command",
            "exec \"$SCOPE_TEST_BINARY\" --exact \"$SCOPE_TERMINAL_TEST\" --nocapture",
            "/dev/null",
        ])
        .env("SCOPE_TEST_BINARY", std::env::current_exe().unwrap())
        .env(FIXTURE_ENV, test)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Terminal fixture failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

#[tokio::test]
async fn streamed_operation_completes_when_scope_has_a_controlling_terminal() {
    if !in_terminal_fixture(
        "domain::system::terminal_tests::streamed_operation_completes_when_scope_has_a_controlling_terminal",
    ) {
        return;
    }
    let result = run_elevated(
        "sh",
        &["-c", TERMINAL_COMMAND],
        AuthMethod::None,
        Duration::from_millis(500),
        &|_| {},
    )
    .await;
    assert!(result.success, "{}\n{}", result.message, result.logs);
    assert!(result.logs.contains("operation-finished"));
}

#[tokio::test]
async fn captured_command_completes_when_scope_has_a_controlling_terminal() {
    if !in_terminal_fixture(
        "domain::system::terminal_tests::captured_command_completes_when_scope_has_a_controlling_terminal",
    ) {
        return;
    }
    let result = capture_output("sh", &["-c", TERMINAL_COMMAND], Duration::from_millis(500))
        .await
        .unwrap();
    assert!(result.success, "{}", result.stderr);
    assert_eq!(result.stdout.trim(), "operation-finished");
}
