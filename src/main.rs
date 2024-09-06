use std::path::PathBuf;

use clap::{Parser, Subcommand};
use tracing::error;

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Build => {
            if let Err(e) = blossom::commands::build().await {
                error!("Failed to build package: {:?}", e);
            }
        }
        Commands::Install { package } => {
            if let Err(e) = blossom::commands::install(package) {
                error!("Failed to install package: {:?}", e);
            }
        }
        Commands::Uninstall { name } => {
            if let Err(e) = blossom::commands::uninstall(name) {
                error!("Failed to remove package: {:?}", e);
            }
        }
        Commands::Info { name } => {
            if let Err(e) = blossom::commands::info(name) {
                error!("Failed to retrieve package info: {:?}", e);
            }
        }
    }
}
