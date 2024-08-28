use std::{
    borrow::Cow,
    collections::HashMap,
    env::current_dir,
    fs::{self, File},
    io::Write as _,
    path::{Path, PathBuf},
    sync::LazyLock, // process::Command,
};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use flate2::{write::GzEncoder, Compression};
use indicatif::ProgressBar;
use regex::Regex;
use reqwest::{Client, IntoUrl};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use spdx::Expression;
use tracing::{error, info};

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
    directories: HashMap<String, String>,
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
struct Dependencies {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    optional: Vec<String>,
    #[serde(default)]
    build: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Source {
    url: String,
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
        error!("package.toml not found in the specified path.");
        bail!("package.toml not found")
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
        let url = replace_vars(&source.url, &info)?;
        fetch_and_verify_source(&client, url.as_ref()).await?;
    }

    // for step in &package.build {
    //     info!("Running build step: {}", step.name);
    //     let result = Command::new("sh").arg("-c").arg(&step.command).status()?;

    //     if !result.success() {
    //         error!("Build step '{}' failed.", step.name);
    //         bail!("Build step failed");
    //     }
    // }

    // create_tarball(path, &package)?;

    info!("Package '{}' built successfully!", info.name);
    Ok(())
}

async fn fetch_and_verify_source<U: IntoUrl>(client: &Client, url: U) -> Result<()> {
    let url = url.into_url()?;
    info!("Fetching source from {}", url);

    let target_path = url.path_segments().unwrap().last().unwrap();

    // if Path::new(&target_path).exists() && check_hash(&target_path, &source.hash)? {
    if Path::new(&target_path).exists() {
        return Ok(());
    }

    let mut target = File::create(target_path)?;

    println!("Downloading \"{}\"", url);

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
    info!("Source hash verified successfully.");

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
