//! Verify native uninstall outcomes before reporting removal or cleaning profiles.

use std::future::Future;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use super::OperationTarget;
use crate::domain::package::InstallScope;
use crate::domain::system::{capture_output_env, ProcessOutput};

const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
const SNAP_TIMEOUT: Duration = Duration::from_secs(8);

/// Confirm that the exact native installation is absent. Residual APT
/// configuration and unavailable metadata are incomplete uninstall outcomes.
pub async fn verify_absent(target: &OperationTarget) -> Result<()> {
    verify_with(target, |query| async move {
        let args: Vec<&str> = query.args.iter().map(String::as_str).collect();
        capture_output_env(query.program, &args, &[("LC_ALL", "C")], query.timeout)
            .await
            .context("Could not query the uninstall outcome")
    })
    .await
}

struct Query {
    program: &'static str,
    args: Vec<String>,
    timeout: Duration,
}

async fn verify_with<F, Fut>(target: &OperationTarget, capture: F) -> Result<()>
where
    F: FnOnce(Query) -> Fut,
    Fut: Future<Output = Result<ProcessOutput>>,
{
    let query = match target {
        OperationTarget::Apt { package_id } => {
            validate_name(package_id, b"+.-:")?;
            Query {
                program: "dpkg-query",
                args: vec![
                    "-W".into(),
                    "-f=${Status}\n".into(),
                    "--".into(),
                    package_id.clone(),
                ],
                timeout: QUERY_TIMEOUT,
            }
        }
        OperationTarget::Snap { package_id } => {
            validate_name(package_id, b"-_")?;
            Query {
                program: "snap",
                args: vec!["list".into(), package_id.clone()],
                timeout: SNAP_TIMEOUT,
            }
        }
        OperationTarget::Flatpak { app_id, scope } => {
            validate_flatpak_id(app_id)?;
            Query {
                program: "flatpak",
                args: vec![
                    "list".into(),
                    match scope {
                        InstallScope::User => "--user".into(),
                        InstallScope::System => "--system".into(),
                    },
                    "--app".into(),
                    "--columns=application".into(),
                ],
                timeout: QUERY_TIMEOUT,
            }
        }
        _ => bail!("Native uninstall verification is unavailable for this source"),
    };
    let output = capture(query).await?;
    if output.success && !output.stderr.trim().is_empty() {
        bail!(
            "Could not verify uninstall without query warnings: {}",
            output.stderr.trim()
        );
    }
    match target {
        OperationTarget::Apt { package_id } => verify_apt(package_id, &output),
        OperationTarget::Snap { package_id } => verify_snap(package_id, &output),
        OperationTarget::Flatpak { app_id, .. } => verify_flatpak(app_id, &output),
        _ => bail!("Native uninstall verification is unavailable for this source"),
    }
}

fn validate_name(name: &str, punctuation: &[u8]) -> Result<()> {
    if !name
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || punctuation.contains(&byte))
    {
        bail!("Invalid package identity for uninstall verification");
    }
    Ok(())
}

fn validate_flatpak_id(id: &str) -> Result<()> {
    validate_name(id, b"._-")?;
    if id.split('.').count() < 3 || id.split('.').any(str::is_empty) {
        bail!("Invalid Flatpak application identity for uninstall verification");
    }
    Ok(())
}

fn verify_apt(package: &str, output: &ProcessOutput) -> Result<()> {
    if !output.success {
        let absent = format!("dpkg-query: no packages found matching {package}");
        if output.exit_code == Some(1)
            && output.stdout.trim().is_empty()
            && output.stderr.trim() == absent
        {
            return Ok(());
        }
        bail!(
            "Could not verify that {package} was purged: {}",
            output.stderr.trim()
        );
    }
    let status: Vec<_> = output.stdout.split_whitespace().collect();
    match status.as_slice() {
        [selection, "ok", "not-installed"]
            if matches!(
                *selection,
                "unknown" | "install" | "hold" | "deinstall" | "purge"
            ) =>
        {
            Ok(())
        }
        [_, _, "config-files"] => {
            bail!("{package} still has package configuration after purge")
        }
        [_, _, "installed"] => bail!("{package} is still installed after uninstall"),
        _ => bail!("Could not verify a clean purge of {package}: unexpected dpkg state"),
    }
}

fn verify_snap(package: &str, output: &ProcessOutput) -> Result<()> {
    if !output.success
        && output.exit_code == Some(1)
        && output.stdout.trim().is_empty()
        && output.stderr.trim() == "error: no matching snaps installed"
    {
        return Ok(());
    }
    if !output.success {
        bail!(
            "Could not verify that {package} was removed: {}",
            output.stderr.trim()
        );
    }
    let mut lines = output.stdout.lines().filter(|line| !line.trim().is_empty());
    let Some(header) = lines.next() else {
        bail!("Could not verify that {package} was removed: empty snap listing");
    };
    if !header.split_whitespace().take(2).eq(["Name", "Version"]) {
        bail!("Could not verify that {package} was removed: unexpected snap listing");
    }
    for line in lines {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 6 {
            bail!("Could not verify that {package} was removed: incomplete snap listing");
        }
        if fields[0] == package {
            bail!("{package} is still installed after uninstall");
        }
    }
    // A targeted listing cannot prove absence by returning unrelated rows or
    // only a header; snap normally reports absence through its diagnostic.
    bail!("Could not verify that {package} was removed: target missing from a successful query")
}

fn verify_flatpak(app_id: &str, output: &ProcessOutput) -> Result<()> {
    if !output.success {
        bail!(
            "Could not verify that {app_id} was removed: {}",
            output.stderr.trim()
        );
    }
    let mut found = false;
    for line in output.stdout.lines().filter(|line| !line.trim().is_empty()) {
        let id = line.trim();
        validate_flatpak_id(id).context("Could not verify removal: malformed Flatpak inventory")?;
        found |= id == app_id;
    }
    if found {
        bail!("{app_id} is still installed in the reviewed Flatpak scope");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(success: bool, stdout: &str, stderr: &str) -> ProcessOutput {
        ProcessOutput {
            success,
            exit_code: Some(if success { 0 } else { 1 }),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[tokio::test]
    async fn apt_query_checks_the_exact_target_without_update_queries() {
        let target = OperationTarget::Apt {
            package_id: "paperling:amd64".into(),
        };
        verify_with(&target, |query| async move {
            assert_eq!(query.program, "dpkg-query");
            assert_eq!(
                query.args,
                ["-W", "-f=${Status}\n", "--", "paperling:amd64"]
            );
            assert_eq!(query.timeout, QUERY_TIMEOUT);
            Ok(output(
                false,
                "",
                "dpkg-query: no packages found matching paperling:amd64\n",
            ))
        })
        .await
        .unwrap();
    }

    #[test]
    fn apt_residual_configuration_is_not_a_complete_purge() {
        assert!(verify_apt(
            "paperling",
            &output(true, "deinstall ok config-files\n", "")
        )
        .unwrap_err()
        .to_string()
        .contains("configuration"));
    }

    #[test]
    fn apt_installed_or_partial_state_does_not_authorize_data_cleanup() {
        for status in [
            "install ok installed",
            "install ok half-installed",
            "install reinstreq not-installed",
            "install ok unpacked",
            "",
            "purge ok not-installed\ninstall ok installed",
        ] {
            assert!(
                verify_apt("paperling", &output(true, status, "")).is_err(),
                "{status}"
            );
        }
    }

    #[test]
    fn apt_accepts_a_clean_not_installed_state() {
        assert!(verify_apt("paperling", &output(true, "purge ok not-installed\n", "")).is_ok());
    }

    #[test]
    fn apt_failed_or_partial_query_is_not_absence() {
        for (stdout, stderr) in [("", "dpkg-query: error: permission denied"), ("install ok installed", "dpkg-query: no packages found matching paperling"), ("", "dpkg-query: no packages found matching other"), ("", "dpkg-query: no packages found matching paperling\ndpkg-query: error: database corrupt")] {
            assert!(verify_apt("paperling", &output(false, stdout, stderr)).is_err());
        }
    }

    #[tokio::test]
    async fn snap_absence_requires_the_native_exact_diagnostic() {
        let target = OperationTarget::Snap {
            package_id: "paperling".into(),
        };
        verify_with(&target, |query| async move {
            assert_eq!(query.program, "snap");
            assert_eq!(query.args, ["list", "paperling"]);
            assert_eq!(query.timeout, SNAP_TIMEOUT);
            Ok(output(false, "", "error: no matching snaps installed\n"))
        })
        .await
        .unwrap();
    }

    #[test]
    fn snap_still_installed_is_reported() {
        assert!(verify_snap("paperling", &output(true, "Name Version Rev Tracking Publisher Notes\npaperling 1.0 4 latest/stable publisher -\n", "")).unwrap_err().to_string().contains("still installed"));
    }

    #[test]
    fn snap_unavailable_malformed_and_partial_queries_are_unknown() {
        for result in [
            output(false, "", "error: cannot communicate with server"),
            output(false, "paperling 1.0", "error: no matching snaps installed"),
            output(true, "Name Version Rev Tracking Publisher Notes\n", ""),
            output(
                true,
                "Name Version Rev Tracking Publisher Notes\nother 1 2 stable publisher -\n",
                "",
            ),
            output(
                true,
                "Name Version Rev Tracking Publisher Notes\npaperling",
                "",
            ),
        ] {
            assert!(verify_snap("paperling", &result).is_err());
        }
    }

    #[tokio::test]
    async fn flatpak_query_keeps_the_reviewed_scope_and_compares_exact_ids() {
        for (scope, flag) in [
            (InstallScope::User, "--user"),
            (InstallScope::System, "--system"),
        ] {
            let target = OperationTarget::Flatpak {
                app_id: "org.example.App".into(),
                scope,
            };
            verify_with(&target, |query| async move {
                assert_eq!(query.program, "flatpak");
                assert_eq!(query.args, ["list", flag, "--app", "--columns=application"]);
                Ok(output(
                    true,
                    "org.example.AppExtra\norg.example.Other\n",
                    "",
                ))
            })
            .await
            .unwrap();
        }
    }

    #[test]
    fn flatpak_empty_success_proves_absence_in_that_scope() {
        assert!(verify_flatpak("org.example.App", &output(true, "", "")).is_ok());
    }

    #[test]
    fn flatpak_still_present_unreadable_and_malformed_are_not_absence() {
        for result in [
            output(true, "org.example.App\n", ""),
            output(false, "", "permission denied"),
            output(false, "org.example.Other\n", "partial inventory"),
            output(true, "Application\n", ""),
            output(true, "org.example.Other\t1.2\n", ""),
        ] {
            assert!(verify_flatpak("org.example.App", &result).is_err());
        }
    }

    #[tokio::test]
    async fn unavailable_command_is_not_an_absent_package() {
        let target = OperationTarget::Apt {
            package_id: "paperling".into(),
        };
        let error = verify_with(&target, |_| async { bail!("injected command timeout") })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("timeout"));
    }

    #[tokio::test]
    async fn wildcard_package_identity_is_rejected_before_querying() {
        let target = OperationTarget::Apt {
            package_id: "paper*".into(),
        };
        assert!(verify_with(&target, |_| async {
            panic!("invalid identity was queried")
        })
        .await
        .is_err());
    }
}
