use tracing::info;
use anyhow::Result;

pub fn info(name: &str) -> Result<()> {
    info!("Retrieving info for package: {}", name);
    Ok(())
}
