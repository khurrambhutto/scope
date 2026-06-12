mod package;
mod scanner;

use package::{sort_packages, Package, SortCriteria};

#[tauri::command]
async fn scan_packages() -> Result<Vec<Package>, String> {
    let mut packages = scanner::scan_all()
        .await
        .map_err(|error| format!("Package scan failed: {error}"))?;

    sort_packages(&mut packages, SortCriteria::SizeDesc);
    Ok(packages)
}

#[tauri::command]
async fn scan_packages_with_updates() -> Result<Vec<Package>, String> {
    let mut packages = scan_packages().await?;
    scanner::check_all_updates(&mut packages)
        .await
        .map_err(|error| format!("Update scan failed: {error}"))?;

    Ok(packages)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan_packages,
            scan_packages_with_updates
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
