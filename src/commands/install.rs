use std::{fs::File, path::Path};

use anyhow::Result;
use tracing::info;

pub fn install<P: AsRef<Path>>(tarball_path: P) -> Result<()> {
    let file = File::open(&tarball_path)?;
    let tar = flate2::read::GzDecoder::new(file);
    let _archive = tar::Archive::new(tar);

    // archive.unpack("/usr/local/")?;
    info!("Installed package from {}", tarball_path.as_ref().display());

    Ok(())
}
