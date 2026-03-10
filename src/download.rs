use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::Utc;
use futures::{stream::FuturesUnordered, StreamExt};
use reqwest::Client;
use scraper::{Html, Selector};
use tokio::fs;

use crate::app::DownloadArgs;

const TEXTURE_URL: &str = "https://freestylized.com/all-textures/";

pub async fn run(args: DownloadArgs) -> Result<()> {
    let client = Client::builder().build().context("Build HTTP client")?;

    let pages = fetch_texture_pages(&client).await?;
    log(format!("found {} pages", pages.len()));

    let (downloads, misses) = fetch_download_links(&client, pages).await;
    log(format!(
        "found {} downloads and {} invalid links",
        downloads.len(),
        misses.len()
    ));

    args.ensure_download_directory_exists().await;
    download_textures(&client, downloads, args.download_dir()).await;

    Ok(())
}

async fn download_textures(client: &Client, downloads: Vec<Download>, download_dir: &PathBuf) {
    let mut tasks = FuturesUnordered::new();
    log(format!("prepare download of {} tasks", tasks.len()));

    for download in downloads {
        let client = client.clone();
        let download_dir = download_dir.clone();
        let url = match &download.info {
            DownloadInfo::Zip(url) | DownloadInfo::GDrive(url) => url.clone(),
        };
        let page_title = download.page_title.clone();

        tasks.push(async move { download_file(&client, url, download_dir, page_title).await });
    }

    let mut count = 0;
    while tasks.next().await.is_some() {
        count = count + 1;
        log(format!("Finished download no. {count}"));
    }
}

async fn fetch_download_links(client: &Client, pages: Vec<String>) -> (Vec<Download>, Vec<String>) {
    let mut downloads = Vec::with_capacity(pages.len());
    let mut misses = Vec::new();
    let mut tasks = FuturesUnordered::new();

    for page in pages {
        let client = client.clone();
        tasks.push(async move { extract_download_data(&client, page, "1K").await });
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

async fn download_file<S: AsRef<str>, P: AsRef<Path>>(
    client: &Client,
    url: String,
    download_dir: P,
    page_title: S,
) -> Result<()> {
    let safe_name = page_title.as_ref().replace(' ', "_");
    let filename = format!("{}.zip", safe_name);
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
    page_title: String,
}

impl Download {
    fn zip(url: String, page_title: String) -> Self {
        Self {
            info: DownloadInfo::Zip(url),
            page_title,
        }
    }

    fn gdrive(url: String, page_title: String) -> Self {
        Self {
            info: DownloadInfo::GDrive(url),
            page_title,
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

    let page_title = document
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|t| t.text().collect::<String>())
        .unwrap_or_else(|| "unknown_texture".into());

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
                        page_title,
                    ))),

                    _ if link.contains("drive.google.com") => Ok(ExtractLinkResult::Found(
                        Download::gdrive(link.into(), page_title),
                    )),

                    _ => Ok(ExtractLinkResult::Missed(page.as_ref().into())),
                };
            }
        }
    }

    Ok(ExtractLinkResult::Missed(page.as_ref().into()))
}

async fn fetch_texture_pages(client: &Client) -> Result<Vec<String>> {
    let body = client.get(TEXTURE_URL).send().await?.text().await?;

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
