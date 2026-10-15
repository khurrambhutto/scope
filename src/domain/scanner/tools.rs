//! Adapter shared by the three user tool managers.
use super::{ScanReport, Scanner};
use crate::domain::{package::PackageSource, system, tool_managers};
use anyhow::Result;
use std::{future::Future, pin::Pin};

pub struct ToolScanner(pub PackageSource);
impl Scanner for ToolScanner {
    fn source(&self) -> PackageSource {
        self.0
    }
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move { system::resolve(tool_managers::manager_name(self.0)).is_some() })
    }
    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
        Box::pin(async move { tool_managers::inventory(self.0).await.map(ScanReport::ok) })
    }
    fn check_updates<'a>(
        &'a self,
        report: &'a mut ScanReport,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            report.warning = tool_managers::check_updates(&mut report.packages).await;
        })
    }
}
