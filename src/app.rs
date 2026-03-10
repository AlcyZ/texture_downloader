use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tokio::fs::create_dir_all;

use crate::download;

const DEFAULT_TEXTURES_DIR: &str = "./textures";

pub struct App;

impl App {
    pub async fn run() -> Result<()> {
        let cli = Cli::parse();

        download::run(cli.into()).await
    }
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Download utility for textures from https://freestylized.com"
)]

struct Cli {
    /// (Optional) Sets the textures download directory.
    #[arg(short, long, default_value = DEFAULT_TEXTURES_DIR)]
    pub dir: PathBuf,
}

pub struct DownloadArgs {
    dir: PathBuf,
}

impl DownloadArgs {
    pub async fn ensure_download_directory_exists(&self) {
        if !self.dir.exists() {
            let _ = create_dir_all(&self.dir).await;
        }
    }

    pub fn download_dir(&self) -> &PathBuf {
        &self.dir
    }
}

impl From<Cli> for DownloadArgs {
    fn from(value: Cli) -> Self {
        Self { dir: value.dir }
    }
}
