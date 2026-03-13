use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tokio::fs::create_dir_all;

use crate::download;

const DEFAULT_DOWNLOAD_DIR: &str = "./textures";
const DEFAULT_DOWNLOAD_LIMIT: usize = 64;
const DEFAULT_SIZE_TEXTURE: TextureSize = TextureSize::_4K;
const DEFAULT_SIZE_SKYBOX: SkyboxSize = SkyboxSize::_2K;

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

    #[command(subcommand)]
    mode: DownloadMode,

    /// (Optional) Limits the amount of consecutive downloads by this value.
    #[arg(short, long, default_value_t = DEFAULT_DOWNLOAD_LIMIT)]
    limit: usize,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq)]
pub enum DownloadMode {
    All {
        #[arg(long, value_enum, default_value_t = DEFAULT_SIZE_TEXTURE)]
        size_textures: TextureSize,

        #[arg(long, value_enum, default_value_t = DEFAULT_SIZE_SKYBOX)]
        size_skybox: SkyboxSize,
    },

    Textures {
        #[arg(long, value_enum, default_value_t = DEFAULT_SIZE_TEXTURE)]
        size_textures: TextureSize,
    },

    Skybox {
        #[arg(long, value_enum, default_value_t = DEFAULT_SIZE_SKYBOX)]
        size_skybox: SkyboxSize,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq)]
pub enum TextureSize {
    _1K,
    _2K,
    _4K,
}

impl ToString for TextureSize {
    fn to_string(&self) -> String {
        match self {
            TextureSize::_1K => "1K".into(),
            TextureSize::_2K => "2K".into(),
            TextureSize::_4K => "4K".into(),
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq)]
pub enum SkyboxSize {
    _2K,
}

impl ToString for SkyboxSize {
    fn to_string(&self) -> String {
        match self {
            SkyboxSize::_2K => "2K".into(),
        }
    }
}

pub struct DownloadArgs {
    dir: PathBuf,
    mode: DownloadMode,
    limit: usize,
}

impl DownloadArgs {
    pub async fn ensure_download_directory_exists(&self) {
        if !self.dir.exists() {
            let _ = create_dir_all(&self.dir).await;
        }

        match self.mode {
            DownloadMode::All { .. } => self.create_all_dir().await,
            DownloadMode::Textures { .. } => self.create_textures_dir().await,
            DownloadMode::Skybox { .. } => self.create_skybox_dir().await,
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

    pub fn limit(&self) -> usize {
        self.limit
    }

    async fn create_all_dir(&self) {
        let _ = tokio::join!(self.create_textures_dir(), self.create_skybox_dir());
    }

    async fn create_textures_dir(&self) {
        let dir = self.download_textures_dir();
        if !dir.exists() {
            let _ = create_dir_all(dir).await;
        }
    }

    async fn create_skybox_dir(&self) {
        let dir = self.download_skybox_dir();
        if !dir.exists() {
            let _ = create_dir_all(dir).await;
        }
    }
}

impl From<Cli> for DownloadArgs {
    fn from(value: Cli) -> Self {
        Self {
            dir: value.dir,
            mode: value.mode,
            limit: value.limit,
        }
    }
}
