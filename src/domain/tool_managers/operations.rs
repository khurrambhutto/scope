//! One typed command builder for tool previews and execution.
use super::{inventory_recorded, validate, validate_package_name, ToolInstallation, UpdateRecipe};
use crate::domain::{
    operations::{Operation, OperationPlan, OperationResult, PlanStep},
    package::PackageSource,
    system,
};
use anyhow::{bail, Context, Result};
use std::{path::PathBuf, time::Duration};

pub struct ToolCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub summary: String,
}

pub fn command(
    id: &str,
    installation: &ToolInstallation,
    operation: Operation,
    candidate: &str,
) -> Result<ToolCommand> {
    validate_package_name(installation.source, id)?;
    let protection = crate::domain::safety::check_package(installation.source, id);
    if protection.protected {
        bail!("{}", protection.reason.unwrap_or_default());
    }
    if let Some(reason) = &installation.protection_reason {
        bail!("{reason}");
    }
    validate(installation)?;
    let mut args: Vec<String> = match (installation.source, operation) {
        (PackageSource::Npm | PackageSource::Bun, _) => {
            super::javascript_commands::command(id, installation, operation)?
        }
        (PackageSource::Uv, Operation::Uninstall) => {
            vec!["tool".into(), "uninstall".into(), id.into()]
        }
        (PackageSource::Uv, Operation::Update) => {
            if !matches!(installation.update, UpdateRecipe::Uv { .. }) {
                bail!("Update checks for this uv source are unsupported");
            }
            vec!["tool".into(), "upgrade".into(), id.into()]
        }
        (PackageSource::Pipx, Operation::Uninstall) => vec!["uninstall".into(), id.into()],
        (PackageSource::Pipx, Operation::Update) => {
            let UpdateRecipe::Pipx {
                pip_args,
                pinned,
                index_source,
                ..
            } = &installation.update
            else {
                bail!("pipx update metadata missing");
            };
            if *pinned {
                bail!("This pipx tool is pinned");
            }
            if !index_source {
                bail!("Update checks for this pipx source are unsupported");
            }
            let mut args = vec!["upgrade".into(), id.into()];
            if !pip_args.is_empty() {
                args.push(format!(
                    "--pip-args={}",
                    pip_args
                        .iter()
                        .map(|arg| quote(arg))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            args
        }
        (PackageSource::Cargo, Operation::Uninstall) => vec![
            "uninstall".into(),
            "--root".into(),
            installation.root.display().to_string(),
            id.into(),
        ],
        (PackageSource::Cargo, Operation::Update) => {
            let UpdateRecipe::Cargo {
                origin,
                options,
                version_req,
            } = &installation.update
            else {
                bail!("Cargo update metadata missing");
            };
            let mut args = vec![
                "install".into(),
                "--root".into(),
                installation.root.display().to_string(),
            ];
            if origin == "registry+https://github.com/rust-lang/crates.io-index"
                || origin == "sparse+https://index.crates.io/"
            {
                let version = semver::Version::parse(candidate)
                    .context("Cargo update candidate is unavailable")?;
                if !semver::VersionReq::parse(version_req.as_deref().unwrap_or("*"))?
                    .matches(&version)
                {
                    bail!("Candidate violates the original Cargo version constraint");
                }
                args.extend(["--registry".into(), "crates-io".into()]);
                // Preserve the user's original version range. Cargo checks and rebuilds the latest allowed version.
                if let Some(requirement) = version_req {
                    args.extend(["--version".into(), requirement.clone()]);
                }
            } else if origin.starts_with("git+") {
                let (url, reference, pinned) = super::updates::git_source(origin)?;
                if pinned {
                    bail!("This Cargo Git installation is pinned");
                }
                args.extend(["--git".into(), url]);
                if let Some(branch) = reference.strip_prefix("refs/heads/") {
                    args.extend(["--branch".into(), branch.into()]);
                }
            } else {
                bail!("This Cargo source cannot be updated safely through Scope");
            }
            args.extend(options.clone());
            args.push(id.into());
            args
        }
        _ => bail!("Unsupported tool operation"),
    };
    if installation.source == PackageSource::Uv {
        args.extend(["--color".into(), "never".into(), "--no-progress".into()]);
    }
    let mut shown = args.clone();
    for arg in &mut shown {
        if arg.starts_with("--pip-args=") {
            *arg = "--pip-args=<recorded pip options>".into();
        }
    }
    let summary = format!(
        "{} {}",
        quote(&installation.manager.display().to_string()),
        shown
            .iter()
            .map(|arg| quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    Ok(ToolCommand {
        program: installation.manager.clone(),
        args,
        summary,
    })
}

fn quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_/.:=,+".contains(c))
    {
        value.into()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

pub fn steps(
    id: &str,
    installation: &ToolInstallation,
    operation: Operation,
    candidate: &str,
) -> Result<Vec<PlanStep>> {
    let command = command(id, installation, operation, candidate)?;
    let mut steps = vec![PlanStep {
        description: match operation {
            Operation::Uninstall => format!("Remove the verified {} installation '{id}'.", installation.source.id()),
            Operation::Update => format!("Upgrade '{id}' using its original {} installation settings. The manager resolves compatible versions when this command runs.", installation.source.id()),
            Operation::Install => bail!("Tool installs are not supported"),
        }, command_summary: command.summary,
    }, PlanStep { description: format!("Installation root: {}", installation.root.display()), command_summary: "User installation; no administrator authorization.".into() }];
    if let UpdateRecipe::Javascript { bin, node, .. } = &installation.update {
        steps.push(PlanStep {
            description: format!("Global command directory: {}", bin.display()),
            command_summary: String::new(),
        });
        if let Some(node) = node {
            steps.push(PlanStep {
                description: format!("Node runtime: {}", node.display()),
                command_summary: String::new(),
            });
        }
    }
    steps.extend(installation.notes.iter().map(|note| PlanStep {
        description: note.clone(),
        command_summary: String::new(),
    }));
    Ok(steps)
}

pub async fn verify(id: &str, expected: &ToolInstallation, version: &str) -> Result<()> {
    let manager = system::resolve(super::manager_name(expected.source))
        .context("The tool manager is unavailable")?;
    if manager != expected.manager {
        bail!("The recorded tool manager changed. Rescan before continuing.");
    }
    if matches!(expected.source, PackageSource::Npm | PackageSource::Bun) {
        super::javascript_safety::verify_context(expected).await?;
    }
    verify_metadata(id, expected, version).await
}

pub(super) async fn verify_metadata(
    id: &str,
    expected: &ToolInstallation,
    version: &str,
) -> Result<()> {
    validate(expected)?;
    let packages = inventory_recorded(expected).await?;
    let live = packages
        .iter()
        .find(|pkg| {
            pkg.package_id == id
                && pkg
                    .tool_installation
                    .as_ref()
                    .is_some_and(|i| i.root == expected.root)
        })
        .context("Tool installation is no longer present. Rescan and preview again.")?;
    if live.version != version || live.tool_installation.as_deref() != Some(expected) {
        bail!("The tool's version, origin, commands, or installation settings changed. Preview again.");
    }
    Ok(())
}

pub async fn apply(
    plan: &OperationPlan,
    id: &str,
    installation: &ToolInstallation,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let result = async {
        // Kept here as well as backend probing so direct tool apply callers also fail closed.
        verify(id, installation, &plan.current_version).await?;
        let recipe = command(id, installation, plan.operation, &plan.target_version)?;
        let mut cmd = tokio::process::Command::new(&recipe.program);
        cmd.args(&recipe.args).envs(&installation.env).env("PIP_NO_INPUT", "1").env("GIT_TERMINAL_PROMPT", "0").current_dir(super::working_directory(installation)?);
        // Builders may launch children. A dedicated group lets timeout stop those too.
        cmd.process_group(0);
        let timeout = if plan.operation == Operation::Update { Duration::from_secs(30 * 60) } else { Duration::from_secs(180) };
        let mut result = system::run_prepared(cmd, &recipe.summary, &[], timeout, on_line).await;
        if result.success {
            let live = match inventory_recorded(installation).await {
                Ok(live) => live,
                Err(error) => {
                    result.success = false;
                    result.message = format!("The manager finished, but final tool state could not be verified: {error}");
                    return Ok(result);
                }
            };
            let current = live.iter().find(|pkg| pkg.package_id == id && pkg.tool_installation.as_ref().is_some_and(|i| i.root == installation.root));
            result.message = match (plan.operation, current) {
                (Operation::Uninstall, None) => format!("'{id}' was uninstalled."),
                (Operation::Uninstall, Some(_)) => { result.success = false; "The manager finished but the installation is still present. Rescan before retrying.".into() }
                (Operation::Update, Some(pkg)) if pkg.version != plan.current_version => format!("'{id}' updated to {}.", pkg.version_label()),
                (Operation::Update, Some(pkg)) if pkg.tool_installation.as_ref().is_some_and(|i| i.receipt_hash != installation.receipt_hash) => format!("'{id}' installation refreshed. Release version remains {}.", pkg.version_label()),
                (Operation::Update, Some(_)) => format!("'{id}' is already at the version allowed by its installation settings. No version change."),
                (Operation::Update, None) => { result.success = false; "The manager finished but the installation is missing. Check the operation log.".into() }
                (Operation::Install, _) => { result.success = false; "Tool installs are not supported.".into() }
            };
        }
        Ok::<_, anyhow::Error>(result)
    }.await;
    result.unwrap_or_else(|error| OperationResult {
        success: false,
        message: error.to_string(),
        logs: String::new(),
        exit_code: None,
    })
}
