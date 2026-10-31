//! Bounded native actions for reviewed, current-user application data.
use std::future::Future;
use std::time::Duration;

use anyhow::{bail, Result};

use crate::domain::system::{capture_output_env, ProcessOutput};

pub(super) async fn reset_permissions(app_id: &str) -> Result<()> {
    reset_with(app_id, |args| async move {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        Ok(capture_output_env(
            "flatpak",
            &args,
            &[("LC_ALL", "C")],
            Duration::from_secs(10),
        )
        .await?)
    })
    .await
}

async fn reset_with<F, Fut>(app_id: &str, capture: F) -> Result<()>
where
    F: Fn(Vec<String>) -> Fut,
    Fut: Future<Output = Result<ProcessOutput>>,
{
    if !app_id.contains('.')
        || app_id.starts_with('.')
        || app_id.ends_with('.')
        || !app_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        bail!("Invalid Flatpak permission identity");
    }
    let output = capture(vec!["permission-reset".into(), app_id.into()]).await?;
    if !output.success || !output.stderr.trim().is_empty() {
        bail!(
            "Flatpak could not reset this app's permissions: {}",
            output.stderr.trim()
        );
    }
    let verified = capture(vec!["permission-show".into(), app_id.into()]).await?;
    if !verified.success || !verified.stderr.trim().is_empty() || !verified.stdout.trim().is_empty()
    {
        bail!("Saved Flatpak permissions could not be verified absent");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(success: bool) -> ProcessOutput {
        ProcessOutput {
            success,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(if success { 0 } else { 1 }),
        }
    }

    #[tokio::test]
    async fn permission_action_is_exact_and_never_global_or_privileged() {
        reset_with("org.example.App", |args| async move {
            assert!(
                args == ["permission-reset", "org.example.App"]
                    || args == ["permission-show", "org.example.App"]
            );
            Ok(output(true))
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn invalid_or_failed_permission_actions_fail_closed() {
        for name in ["--all", "", "org.example/other"] {
            assert!(reset_with(name, |_| async {
                panic!("invalid identity reached command")
            })
            .await
            .is_err());
        }
        assert!(
            reset_with("org.example.App", |_| async { Ok(output(false)) })
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn successful_reset_with_remaining_or_unreadable_permissions_is_incomplete() {
        let result = reset_with("org.example.App", |args| async move {
            let mut result = output(true);
            if args[0] == "permission-show" {
                result.stdout = "a remaining grant".into();
            }
            Ok(result)
        })
        .await;
        assert!(result.is_err());
    }
}
