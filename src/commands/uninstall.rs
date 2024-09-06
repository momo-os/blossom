use tracing::info;
use anyhow::Result;

pub fn uninstall(name: &str) -> Result<()> {
    info!("Removing package: {}", name);
    Ok(())
}