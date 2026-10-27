use super::*;
use tokio::task::JoinSet;

/// Read-only timings for the actual scanner paths on this machine.
#[tokio::test]
#[ignore = "reads installed packages and queries update services"]
async fn profile_live_scan() {
    let mut tasks = JoinSet::new();
    let start = std::time::Instant::now();
    for scanner in scanners() {
        tasks.spawn(async move {
            let start = std::time::Instant::now();
            let source = scanner.source();
            if scanner.is_available().await {
                let report = scanner.scan().await;
                println!("{source:?} inventory: {:?}", start.elapsed());
                let report = match report {
                    Ok(mut report) => {
                        scanner.check_updates(&mut report).await;
                        Ok(report)
                    }
                    Err(error) => Err(error),
                };
                println!(
                    "{source:?}: {:?}, packages={:?}",
                    start.elapsed(),
                    report.as_ref().map(|report| report.packages.len())
                );
            }
        });
    }
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    println!("parallel sources: {:?}", start.elapsed());
    let start = std::time::Instant::now();
    let mut inventory_identity = Vec::new();
    let (packages, _) = scan_all_with_inventory(|(packages, _)| {
        inventory_identity = identities(&packages);
        println!(
            "installed list ready: {:?}, {} packages",
            start.elapsed(),
            packages.len()
        );
    })
    .await;
    assert_eq!(inventory_identity, identities(&packages));
    println!(
        "full scan: {:?}, {} packages",
        start.elapsed(),
        packages.len()
    );
}

fn identities(packages: &[InstalledPackage]) -> Vec<(String, String)> {
    let mut identities: Vec<_> = packages
        .iter()
        .map(|package| (package.key.clone(), package.version.clone()))
        .collect();
    identities.sort_unstable();
    identities
}
