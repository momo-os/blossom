use std::{
    borrow::Cow,
    collections::HashMap,
    env::current_dir,
    fs::{self, File},
    io::{Read, Write as _},
    path::{Path, PathBuf},
    process::Command,
    sync::LazyLock,
};

use anyhow::{anyhow, bail, Result};
use bzip2::read::BzDecoder;
use camino::Utf8Path;
use clap::{Parser, Subcommand};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sha2::{Digest, Sha256 as Sha256Hasher};
use spdx::Expression;
use tar::Archive;
use tracing::{error, info};
use xz2::read::XzDecoder;

#[derive(Parser)]
#[command(name = "blossom")]
#[command(about = "Blossom - A package manager for linux", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build,
    Install {
        #[arg(short, long)]
        package: PathBuf,
    },
    Uninstall {
        #[arg(short, long)]
        name: String,
    },
    Info {
        #[arg(short, long)]
        name: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Package {
    info: Info,
    dependencies: Option<Dependencies>,
    #[serde(default)]
    sources: Vec<Source>,
    #[serde(default)]
    steps: Vec<Step>,
    #[serde(default)]
    directories: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Dependencies {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
    #[serde(default)]
    build: Vec<String>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
struct Info {
    name: String,
    version: String,
    description: String,
    #[serde_as(as = "DisplayFromStr")]
    license: Expression,
}

#[derive(Debug, Serialize, Deserialize)]
struct Source {
    url: String,
    checksum: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Step {
    name: String,
    runner: String,
    command: String,
}

static VARIABLE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"%\{([a-zA-Z0-9_]*)\}").expect("invalid regex"));

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Build => {
            if let Err(e) = build_package().await {
                error!("Failed to build package: {:?}", e);
            }
        }
        Commands::Install { package } => {
            if let Err(e) = install_package(package) {
                error!("Failed to install package: {:?}", e);
            }
        }
        Commands::Uninstall { name } => {
            if let Err(e) = remove_package(name) {
                error!("Failed to remove package: {:?}", e);
            }
        }
        Commands::Info { name } => {
            if let Err(e) = info_package(name) {
                error!("Failed to retrieve package info: {:?}", e);
            }
        }
    }
}

fn replace_vars<'a>(haystack: &'a str, info: &Info) -> Result<Cow<'a, str>> {
    let res = if let Some(captures) = VARIABLE_REGEX.captures(haystack) {
        match &captures[1] {
            "version" => VARIABLE_REGEX.replace_all(haystack, &info.version),
            _ => bail!("Wrong matcher"),
        }
    } else {
        Cow::Borrowed(haystack)
    };

    Ok(res)
}

async fn build_package() -> Result<()> {
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

    // fs::create_dir_all("sources")?;

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

fn check_hash<P: AsRef<Path>>(path: P, hash: &str) -> Result<bool> {
    let file = fs::read(path)?;
    let (hash_type, hash) = hash
        .split_once(':')
        .ok_or(anyhow!("Invalid checksum format"))?;

    let computed_hash = match hash_type {
        "blake3" => blake3::hash(&file).to_hex().to_string(),
        "sha256" => base16ct::lower::encode_string(Sha256Hasher::digest(&file).as_slice()),
        _ => bail!("Unsupported hash"),
    };

    Ok(hash == computed_hash)
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

fn _create_tarball<P: AsRef<Path>>(package_path: P, package: &Package) -> Result<()> {
    let tarball_name = format!("{}_{}.peach", package.info.name, package.info.version);
    let tarball_path = package_path.as_ref().join(&tarball_name);
    let tar_gz = File::create(&tarball_path)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    tar.append_dir_all(".", package_path)?;

    info!("Created tarball: {}", tarball_name);
    Ok(())
}

fn install_package<P: AsRef<Path>>(tarball_path: P) -> Result<()> {
    let file = File::open(&tarball_path)?;
    let tar = flate2::read::GzDecoder::new(file);
    let _archive = tar::Archive::new(tar);

    // archive.unpack("/usr/local/")?;
    info!("Installed package from {}", tarball_path.as_ref().display());

    Ok(())
}

/// Removes an installed package (mock)
fn remove_package(name: &str) -> Result<()> {
    info!("Removing package: {}", name);
    Ok(())
}

fn info_package(name: &str) -> Result<()> {
    info!("Retrieving info for package: {}", name);
    Ok(())
}
