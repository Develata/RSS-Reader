use anyhow::Result;
use rssr_domain::ConfigPackage;

#[derive(Default)]
pub struct ImportExportService;

impl ImportExportService {
    pub fn new() -> Self {
        Self
    }

    pub async fn export_config(&self) -> Result<Option<ConfigPackage>> {
        Ok(None)
    }
}
