//! APT simulation parsing and apply-time transaction comparison.

use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::domain::operations::Operation;
use crate::domain::package::PackageSource;
use crate::domain::safety;
use crate::domain::system::capture_output_env;

const SIMULATION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptTransaction {
    pub installs: Vec<String>,
    pub upgrades: Vec<String>,
    pub removals: Vec<String>,
    pub fingerprint: String,
}

pub async fn simulate(operation: Operation, package_id: &str) -> anyhow::Result<AptTransaction> {
    let action = match operation {
        Operation::Uninstall => "remove",
        Operation::Update | Operation::Install => "install",
    };
    let output = capture_output_env(
        "apt-get",
        &["-s", action, "-y", "--", package_id],
        &[("LC_ALL", "C")],
        SIMULATION_TIMEOUT,
    )
    .await?;
    if !output.success {
        anyhow::bail!("APT simulation failed: {}", output.stderr.trim())
    }
    parse(&output.stdout, operation, package_id)
}

pub fn parse(
    stdout: &str,
    operation: Operation,
    package_id: &str,
) -> anyhow::Result<AptTransaction> {
    let mut installs = Vec::new();
    let mut upgrades = Vec::new();
    let mut removals = Vec::new();
    for line in stdout.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("Remv ") {
            if let Some(name) = rest.split_whitespace().next() {
                removals.push(name.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("Inst ") {
            if let Some(name) = rest.split_whitespace().next() {
                if rest
                    .split_whitespace()
                    .nth(1)
                    .is_some_and(|value| value.starts_with('['))
                {
                    upgrades.push(name.to_string());
                } else {
                    installs.push(name.to_string());
                }
            }
        }
    }
    installs.sort();
    installs.dedup();
    upgrades.sort();
    upgrades.dedup();
    removals.sort();
    removals.dedup();
    let target_present = match operation {
        Operation::Uninstall => removals.iter().any(|name| name == package_id),
        Operation::Install | Operation::Update => installs
            .iter()
            .chain(&upgrades)
            .any(|name| name == package_id),
    };
    if !target_present {
        anyhow::bail!("APT simulation did not include the requested package")
    }
    for removal in &removals {
        let protection = safety::check_package(PackageSource::Apt, removal);
        if protection.protected {
            anyhow::bail!(
                "APT would remove protected package '{removal}': {}",
                protection.reason.unwrap_or_else(|| "protected".into())
            )
        }
    }
    // Include versions and configuration actions, not just package names.
    let mut actions: Vec<_> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| {
            ["Inst ", "Remv ", "Conf "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    actions.sort();
    let canonical = format!(
        "install:{}\nupgrade:{}\nremove:{}\nactions:{}\n",
        installs.join(","),
        upgrades.join(","),
        removals.join(","),
        actions.join("\n")
    );
    let fingerprint = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    Ok(AptTransaction {
        installs,
        upgrades,
        removals,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_new_dependencies_from_upgrades_with_architecture_brackets() {
        let transaction = parse(
            "Inst demo (2.0 local-deb [amd64])\nInst libexample1 (1.0 Ubuntu [amd64])\nInst helper [1.0] (2.0 Ubuntu [amd64])\n",
            Operation::Install, "demo",
        ).unwrap();
        assert_eq!(transaction.installs, vec!["demo", "libexample1"]);
        assert_eq!(transaction.upgrades, vec!["helper"]);
    }

    #[test]
    fn fingerprint_changes_when_only_dependency_version_changes() {
        let first = parse(
            "Inst demo (1.0 local-deb [all])\nInst helper (1.0 Ubuntu [amd64])\n",
            Operation::Install,
            "demo",
        )
        .unwrap();
        let second = parse(
            "Inst demo (1.0 local-deb [all])\nInst helper (2.0 Ubuntu [amd64])\n",
            Operation::Install,
            "demo",
        )
        .unwrap();
        assert_ne!(first.fingerprint, second.fingerprint);
    }

    #[test]
    fn desktop_app_removal_cannot_remove_the_ubuntu_session() {
        for protected in ["ubuntu-session", "ubuntu-desktop-minimal"] {
            let output = format!("Remv nautilus [1.0]\nRemv {protected} [1.0]\n");
            let error = parse(&output, Operation::Uninstall, "nautilus").unwrap_err();
            assert!(error.to_string().contains(protected));
        }
    }

    #[test]
    fn install_rejects_indirect_protected_removals() {
        assert!(parse(
            "Inst demo (1.0 local-deb [all])\nRemv systemd [255]\n",
            Operation::Install,
            "demo"
        )
        .is_err());
    }

    #[test]
    fn parses_and_fingerprints_an_update_transaction() {
        let transaction = parse(
            "Inst gimp [2.10] (3.0 Ubuntu:stable [amd64])\nConf gimp (3.0 Ubuntu:stable [amd64])\n",
            Operation::Update,
            "gimp",
        )
        .unwrap();
        assert_eq!(transaction.upgrades, vec!["gimp"]);
    }

    #[test]
    fn rejects_an_indirect_protected_removal() {
        let error = parse(
            "Remv gimp [2.10]\nRemv systemd [255]\n",
            Operation::Uninstall,
            "gimp",
        )
        .unwrap_err();
        assert!(error.to_string().contains("protected package 'systemd'"));
    }

    #[test]
    fn rejects_output_without_the_requested_action() {
        assert!(parse("0 upgraded, 0 removed.\n", Operation::Uninstall, "gimp").is_err());
    }

    #[test]
    fn fingerprint_changes_when_transaction_changes() {
        let first = parse("Remv gimp [1]\n", Operation::Uninstall, "gimp").unwrap();
        let second = parse(
            "Remv gimp [1]\nRemv gimp-data [1]\n",
            Operation::Uninstall,
            "gimp",
        )
        .unwrap();
        assert_ne!(first.fingerprint, second.fingerprint);
    }
}
