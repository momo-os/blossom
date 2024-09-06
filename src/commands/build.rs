use std::{
    env::current_dir,
    fs::{self, File},
    io::{Read, Write as _},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Result};
use bzip2::read::BzDecoder;
use camino::Utf8Path;
use flate2::read::GzDecoder;
use indicatif::ProgressBar;
use reqwest::{Client, Url};
use tar::Archive;
use tracing::info;
use xz2::read::XzDecoder;

use crate::{
    check_hash,
    package::{Info, Package, Source},
    replace_vars,
};

pub async fn build() -> Result<()> {
    let package_path = current_dir()?.join("package.toml");

    if !package_path.exists() {
        bail!("package.toml not found in the specified path.");
    }

    let package: Package = toml_edit::de::from_str(&fs::read_to_string(package_path)?)?;

    // dbg!(&package);

    let info = package.info;
    info!(
        "Building package \"{}\" version {}",
        &info.name, &info.version
    );

    // for _dependency in package.dependencies {
    //     // info!("Installing dependency: {dependency}");
    // }

    let client = Client::new();

    fs::remove_dir_all("sources")?;

    for source in package.sources {
        let file_path = fetch_and_verify_source(&client, &source, &info).await?;
        extract_source(&file_path)?;
    }

    for step in package.steps {
        info!("Running step: {}", step.name);
        let result = Command::new("sh").arg("-c").arg(&step.command).status()?;

        if !result.success() {
            bail!("Step '{}' failed.", step.name);
        }
    }

    // create_tarball(path, &package)?;

    info!("Package '{}' built successfully!", info.name);
    Ok(())
}

async fn fetch_and_verify_source(client: &Client, source: &Source, info: &Info) -> Result<PathBuf> {
    let url: Url = replace_vars(&source.url, &info)?.as_ref().try_into()?;

    let target_path = PathBuf::from(url.path_segments().unwrap().last().unwrap());

    if Path::new(&target_path).exists() && check_hash(&target_path, &source.checksum)? {
        return Ok(target_path);
    }

    info!("Fetching source from {}", url);

    let mut target = File::create(&target_path)?;

    info!("Downloading \"{}\"", url);

    let mut res = client.get(url).send().await?;
    let len = res.content_length().unwrap_or(0);

    let progress_bar = ProgressBar::new(len);

    while let Some(chunk) = res.chunk().await? {
        progress_bar.inc(chunk.len() as u64);
        target.write_all(&chunk)?;
    }

    progress_bar.finish();

    info!("Source fetched successfully.");
    info!("Verifying source hash.");

    if !check_hash(&target_path, &source.checksum)? {
        bail!("Hash didn't match!")
    }

    info!("Source hash verified successfully.");

    Ok(target_path)
}

fn extract_source(target_path: &Path) -> Result<()> {
    let target_path = Utf8Path::from_path(target_path).unwrap();

    info!("Extracting \"{target_path}\"");

    // let archive_path = format!("sources/{}", source.name);
    // let archive_path = Utf8Path::new(&archive_path);

    // if archive_path.join(".ok").exists() {
    //     return Ok(());
    // }

    // if archive_path.exists() {
    //     fs::remove_dir_all(archive_path)?;
    // }

    let target = File::open(target_path)?;

    match target_path.extension().unwrap() {
        "xz" => {
            unpack_archive(XzDecoder::new(target))?;
        }
        "gz" => {
            unpack_archive(GzDecoder::new(target))?;
        }
        "bz2" => {
            unpack_archive(BzDecoder::new(target))?;
        }
        _ => bail!("Something went wrong extracting"),
    }

    info!("Archive extracted successfully");

    Ok(())
}

fn unpack_archive<R: Read>(decoder: R) -> Result<()> {
    // println!("Unpacking {name}");

    let mut archive = Archive::new(decoder);

    archive.unpack("sources/")?;

    Ok(())
}
