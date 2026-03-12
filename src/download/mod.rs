use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::{future::BoxFuture, stream::FuturesUnordered, StreamExt};
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::fs;

use crate::app::{DownloadArgs, DownloadMode};

const TEXTURE_URL: &str = "https://freestylized.com/all-textures/";
const SKYBOX_URL: &str = "https://freestylized.com/all-skybox/";

enum DownloadTarget {
    Textures,
    Skybox,
}

pub async fn run(args: DownloadArgs) -> Result<()> {
    args.ensure_download_directory_exists().await;

    let client = Client::builder().build().context("Build HTTP client")?;
    let button_text_textures = "1K";
    let button_text_skybox = "2K";

    let download_tex = async || {
        download(
            DownloadTarget::Textures,
            button_text_textures,
            &client,
            args.download_textures_dir(),
        )
        .await
    };

    let download_sky = async || {
        download(
            DownloadTarget::Skybox,
            button_text_skybox,
            &client,
            args.download_skybox_dir(),
        )
        .await
    };

    match args.mode() {
        DownloadMode::All => {
            let mut tasks: FuturesUnordered<BoxFuture<Result<()>>> = FuturesUnordered::new();

            tasks.push(Box::pin(download_tex()));
            tasks.push(Box::pin(download_sky()));

            while tasks.next().await.is_some() {}
        }
        DownloadMode::Textures => download_tex().await?,
        DownloadMode::Skybox => download_sky().await?,
    }

    Ok(())
}

async fn download(
    target: DownloadTarget,
    button_text: &str,
    client: &Client,
    download_dir: PathBuf,
) -> Result<()> {
    let url = match target {
        DownloadTarget::Textures => TEXTURE_URL,
        DownloadTarget::Skybox => SKYBOX_URL,
    };
    let pages = fetch_download_pages(&client, url).await?;
    log(format!("found {} pages", pages.len()));

    let (downloads, misses) = fetch_download_links(&client, pages, button_text).await;
    log(format!(
        "found {} downloads and {} invalid links",
        downloads.len(),
        misses.len()
    ));

    download_data(&client, downloads, download_dir, target).await;

    Ok(())
}

async fn download_data(
    client: &Client,
    downloads: Vec<Download>,
    download_dir: PathBuf,
    target: DownloadTarget,
) {
    let downloads_len = downloads.len();
    let kind = match target {
        DownloadTarget::Textures => "texture",
        DownloadTarget::Skybox => "skybox",
    };
    let mut tasks = FuturesUnordered::new();
    log(format!(
        "Prepare downloading for {} {}",
        downloads_len, kind
    ));

    for download in downloads {
        let client = client.clone();
        let download_dir = download_dir.clone();
        let url = match &download.info {
            DownloadInfo::Zip(url) | DownloadInfo::GDrive(url) => url.clone(),
        };
        let filename = download.filename.clone();

        tasks.push(async move { download_file(&client, url, download_dir, filename).await });
    }

    let kind = format!("{kind}:");
    let mut count = 0;
    while tasks.next().await.is_some() {
        count = count + 1;

        let msg = format!("Finished download {kind:<8} {} of {}", count, downloads_len,);
        log(msg);
    }
}

async fn fetch_download_links(
    client: &Client,
    pages: Vec<String>,
    button_text: &str,
) -> (Vec<Download>, Vec<String>) {
    let mut downloads = Vec::with_capacity(pages.len());
    let mut misses = Vec::new();
    let mut tasks = FuturesUnordered::new();

    for page in pages {
        let client = client.clone();
        tasks.push(async move { extract_download_data(&client, page, button_text).await });
    }

    while let Some(result) = tasks.next().await {
        if let Ok(download) = result {
            match download {
                ExtractLinkResult::Found(download_type) => downloads.push(download_type),
                ExtractLinkResult::Missed(page) => misses.push(page),
            }
        }
    }

    (downloads, misses)
}

async fn download_file<P: AsRef<Path>>(
    client: &Client,
    url: String,
    download_dir: P,
    filename: String,
) -> Result<()> {
    let mut path = download_dir.as_ref().to_path_buf();
    path.push(&filename);

    if Path::new(&path).exists() {
        println!("skip {}", filename);
        return Ok(());
    }

    let bytes = client
        .get(&url)
        .send()
        .await
        .context("send download request")?
        .bytes()
        .await
        .context("collecting download bytes")?;

    fs::write(path, bytes).await.context("save texture")?;

    Ok(())
}

#[derive(Debug, Clone)]
struct Download {
    info: DownloadInfo,
    filename: String,
}

impl Download {
    fn zip(url: String, filename: String) -> Self {
        Self {
            info: DownloadInfo::Zip(url),
            filename,
        }
    }

    fn gdrive(url: String, filename: String) -> Self {
        Self {
            info: DownloadInfo::GDrive(url),
            filename,
        }
    }
}

enum ExtractLinkResult {
    Found(Download),
    Missed(String),
}

#[derive(Debug, Clone)]
enum DownloadInfo {
    Zip(String),
    GDrive(String),
}

async fn extract_download_data<P: AsRef<str>, B: AsRef<str>>(
    client: &Client,
    page: P,
    button: B,
) -> Result<ExtractLinkResult> {
    let body = client.get(page.as_ref()).send().await?.text().await?;

    let document = Html::parse_document(&body);
    let selector = Selector::parse(".breakdance-link").unwrap();
    let filename = get_filename(&document);

    for element in document.select(&selector) {
        let text = element.text().collect::<String>();

        if text
            .to_lowercase()
            .contains(&button.as_ref().to_lowercase())
        {
            if let Some(link) = element.value().attr("href") {
                return match () {
                    _ if link.ends_with(".zip") => Ok(ExtractLinkResult::Found(Download::zip(
                        link.into(),
                        filename,
                    ))),

                    _ if link.contains("drive.google.com") => Ok(ExtractLinkResult::Found(
                        Download::gdrive(link.into(), filename),
                    )),

                    _ => Ok(ExtractLinkResult::Missed(page.as_ref().into())),
                };
            }
        }
    }

    Ok(ExtractLinkResult::Missed(page.as_ref().into()))
}

fn get_filename(document: &Html) -> String {
    let page_title = document
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|t| t.text().collect::<String>())
        .unwrap_or_else(|| "unknown_texture".into());

    let safe = |s: &str| format!("{}.zip", s.replace(' ', "_"));

    if let Some(part) = page_title.split('|').next() {
        return safe(part);
    }

    return safe(&page_title[0..20]);
}

async fn fetch_download_pages(client: &Client, url: &str) -> Result<Vec<String>> {
    let body = client.get(url).send().await?.text().await?;

    let document = Html::parse_document(&body);
    let selector = Selector::parse(".ee-posts-grid .ee-post .ee-post-image-link").unwrap();

    let mut pages = HashSet::new();

    for element in document.select(&selector) {
        if let Some(link) = element.value().attr("href") {
            pages.insert(link.to_string());
        }
    }

    Ok(pages.into_iter().collect())
}

fn log<S: AsRef<str>>(msg: S) {
    println!(
        "[{}]: {}",
        Utc::now().format("%H:%M").to_string(),
        msg.as_ref()
    )
}
