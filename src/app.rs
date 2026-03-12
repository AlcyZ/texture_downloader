use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use tokio::fs::create_dir_all;

use crate::download;

const DEFAULT_DOWNLOAD_DIR: &str = "./textures";
const DEFAULT_DOWNLOAD_MODE: DownloadMode = DownloadMode::All;

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
    #[arg(short, long, default_value = DEFAULT_DOWNLOAD_DIR)]
    dir: PathBuf,

    /// Defines reporting mode. Simple just prints a list of times with connectivity status.
    #[arg(short, long, value_enum, default_value_t = DEFAULT_DOWNLOAD_MODE)]
    pub mode: DownloadMode,
}

pub struct DownloadArgs {
    dir: PathBuf,
    mode: DownloadMode,
}

impl DownloadArgs {
    pub async fn ensure_download_directory_exists(&self) {
        if !self.dir.exists() {
            let _ = create_dir_all(&self.dir).await;
        }

        if self.mode == DownloadMode::All || self.mode == DownloadMode::Textures {
            let mut dir = self.dir.clone();
            dir.push("textures");
            if !dir.exists() {
                let _ = create_dir_all(dir).await;
            }
        }

        if self.mode == DownloadMode::All || self.mode == DownloadMode::Skybox {
            let mut dir = self.dir.clone();
            dir.push("skybox");
            if !dir.exists() {
                let _ = create_dir_all(dir).await;
            }
        }
    }

    pub fn mode(&self) -> DownloadMode {
        self.mode
    }

    pub fn download_textures_dir(&self) -> PathBuf {
        let mut dir = self.dir.clone();
        dir.push("textures");
        dir
    }

    pub fn download_skybox_dir(&self) -> PathBuf {
        let mut dir = self.dir.clone();
        dir.push("skybox");
        dir
    }
}

impl From<Cli> for DownloadArgs {
    fn from(value: Cli) -> Self {
        Self {
            dir: value.dir,
            mode: value.mode,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, PartialEq, Copy)]
pub enum DownloadMode {
    All,
    Textures,
    Skybox,
}
