use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::sync::LazyLock;

use super::{read_cached_json, SubtitleSearchResult, CACHE_TTL_30_DAYS};

static DIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="/dirlist\.php\?dir=subtitles%2Fjapanese%2F([^/\"&]+)%2F""#)
        .expect("Valid regex syntax")
});

static FILE_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="(subtitles/japanese/[^"]+)"[^>]*>(?:<strong>)?(.*?)(?:</strong>)?</a>"#)
        .expect("Valid regex syntax")
});

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedKitsunekkoEntry {
    pub slug: String,
    pub name: String,
}

pub struct KitsunekkoProvider {
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl KitsunekkoProvider {
    pub fn new(client: reqwest::Client, cache_dir: PathBuf) -> Self {
        Self { client, cache_dir }
    }

    /// Load or update local Kitsunekko index cache (~/.cache/subsink/kitsunekko_index.json)
    pub async fn ensure_index(&self) -> Result<Vec<CachedKitsunekkoEntry>> {
        let cache_path = self.cache_dir.join("kitsunekko_index.json");

        if let Some(entries) =
            read_cached_json::<Vec<CachedKitsunekkoEntry>>(&cache_path, CACHE_TTL_30_DAYS)
        {
            if !entries.is_empty() {
                return Ok(entries);
            }
        }

        let entries = self.fetch_index().await?;
        if !entries.is_empty() {
            if let Ok(file) = File::create(&cache_path) {
                let _ = serde_json::to_writer(file, &entries);
            }
        }

        Ok(entries)
    }

    async fn fetch_index(&self) -> Result<Vec<CachedKitsunekkoEntry>> {
        let url = "https://kitsunekko.net/dirlist.php?dir=subtitles%2Fjapanese%2F";
        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(Vec::new()),
        };

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let html = match resp.text().await {
            Ok(t) => t,
            Err(_) => return Ok(Vec::new()),
        };

        let mut entries = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for cap in DIR_RE.captures_iter(&html) {
            let raw_slug = &cap[1];
            if !seen.insert(raw_slug.to_string()) {
                continue;
            }

            let decoded_name = urlencoding::decode(raw_slug)
                .unwrap_or_else(|_| raw_slug.into())
                .replace('+', " ");

            entries.push(CachedKitsunekkoEntry {
                slug: raw_slug.to_string(),
                name: decoded_name,
            });
        }

        Ok(entries)
    }

    /// Fetch files for a selected anime folder slug with 30 days local cache
    pub async fn fetch_files(
        &self,
        slug: &str,
        anime_name: &str,
    ) -> Result<Vec<SubtitleSearchResult>> {
        let safe_slug = slug.replace(['/', '+'], "_");
        let cache_path = self
            .cache_dir
            .join(format!("kitsunekko_{}_files.json", safe_slug));

        let cached_files: Option<Vec<SubtitleSearchResult>> =
            read_cached_json(&cache_path, CACHE_TTL_30_DAYS);

        match cached_files {
            Some(files) if !files.is_empty() => Ok(files),
            _ => {
                let fetched = self.scrape_folder_files(slug, anime_name).await?;
                if !fetched.is_empty() {
                    if let Ok(file) = File::create(&cache_path) {
                        let _ = serde_json::to_writer(file, &fetched);
                    }
                }
                Ok(fetched)
            }
        }
    }

    async fn scrape_folder_files(
        &self,
        slug: &str,
        anime_name: &str,
    ) -> Result<Vec<SubtitleSearchResult>> {
        let folder_url = format!(
            "https://kitsunekko.net/dirlist.php?dir=subtitles%2Fjapanese%2F{}%2F",
            slug
        );
        let mut results = Vec::new();

        let resp = match self.client.get(&folder_url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(results),
        };

        let html = match resp.text().await {
            Ok(t) => t,
            Err(_) => return Ok(results),
        };

        for cap in FILE_LINK_RE.captures_iter(&html) {
            let rel_path = &cap[1];
            let raw_text = &cap[2];

            let file_name = html_escape::decode_html_entities(raw_text).to_string();
            let file_name = if file_name.is_empty() {
                rel_path
                    .split('/')
                    .next_back()
                    .unwrap_or(rel_path)
                    .to_string()
            } else {
                file_name
            };

            let ext = if file_name.ends_with(".ass") {
                "ass"
            } else if file_name.ends_with(".srt") {
                "srt"
            } else if file_name.ends_with(".zip") {
                "zip"
            } else if file_name.ends_with(".7z") {
                "7z"
            } else if file_name.ends_with(".rar") {
                "rar"
            } else {
                "sub"
            };

            let encoded_path = rel_path
                .split('/')
                .map(urlencoding::encode)
                .collect::<Vec<_>>()
                .join("/");
            let full_url = format!("https://kitsunekko.net/{}", encoded_path);
            let display_title = format!("{} - {}", anime_name, file_name);

            results.push(SubtitleSearchResult {
                provider: "Kitsunekko".to_string(),
                title: display_title,
                episode: None,
                language: "JP".to_string(),
                download_url: full_url,
                file_format: ext.to_string(),
            });
        }

        Ok(results)
    }
}
