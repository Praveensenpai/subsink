use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::sync::LazyLock;

use super::{read_cached_json, SubtitleSearchResult, CACHE_TTL_30_DAYS};

static ENTRY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"<div class="entry" data-extra="([^"]+)">\s*<a href="/entry/(\d+)"[^>]*>(.*?)</a>"#,
    )
    .expect("Valid regex syntax")
});

static FILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href="(/entry/\d+/download/[^"]+)"[^>]*>(.*?)</a>"#).expect("Valid regex syntax")
});

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedJimakuEntry {
    pub id: u64,
    pub name: String,
    pub english_name: Option<String>,
    pub japanese_name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct JimakuDataExtra {
    name: Option<String>,
    english_name: Option<String>,
    japanese_name: Option<String>,
}

pub struct JimakuProvider {
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl JimakuProvider {
    pub fn new(client: reqwest::Client, cache_dir: PathBuf) -> Self {
        Self { client, cache_dir }
    }

    /// Load or update local Jimaku index cache (~/.cache/subsink/jimaku_index.json)
    pub async fn ensure_index(&self) -> Result<Vec<CachedJimakuEntry>> {
        let cache_path = self.cache_dir.join("jimaku_index.json");

        if let Some(entries) =
            read_cached_json::<Vec<CachedJimakuEntry>>(&cache_path, CACHE_TTL_30_DAYS)
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

    async fn fetch_index(&self) -> Result<Vec<CachedJimakuEntry>> {
        let resp = self.client.get("https://jimaku.cc/").send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let html_doc = resp.text().await?;
        let mut entries = Vec::new();

        for cap in ENTRY_RE.captures_iter(&html_doc) {
            let data_extra_raw = &cap[1];
            let id: u64 = cap[2].parse().unwrap_or(0);
            if id == 0 {
                continue;
            }

            let decoded_extra = html_escape::decode_html_entities(data_extra_raw);
            let extra: Option<JimakuDataExtra> = serde_json::from_str(&decoded_extra).ok();
            let title_decoded = html_escape::decode_html_entities(&cap[3]).to_string();

            let name = extra
                .as_ref()
                .and_then(|e| e.name.clone())
                .unwrap_or(title_decoded);
            let english_name = extra.as_ref().and_then(|e| e.english_name.clone());
            let japanese_name = extra.as_ref().and_then(|e| e.japanese_name.clone());

            entries.push(CachedJimakuEntry {
                id,
                name,
                english_name,
                japanese_name,
            });
        }

        Ok(entries)
    }

    /// Fetch files for a selected entry ID with 30 days local cache (~/.cache/subsink/entry_{id}_files.json)
    pub async fn fetch_files(
        &self,
        id: u64,
        anime_name: &str,
    ) -> Result<Vec<SubtitleSearchResult>> {
        let cache_path = self.cache_dir.join(format!("entry_{}_files.json", id));

        let cached_files: Option<Vec<SubtitleSearchResult>> =
            read_cached_json(&cache_path, CACHE_TTL_30_DAYS);

        match cached_files {
            Some(files) if !files.is_empty() => Ok(files),
            _ => {
                let fetched = self.scrape_entry_files(id, anime_name).await?;
                if !fetched.is_empty() {
                    if let Ok(file) = File::create(&cache_path) {
                        let _ = serde_json::to_writer(file, &fetched);
                    }
                }
                Ok(fetched)
            }
        }
    }

    async fn scrape_entry_files(
        &self,
        id: u64,
        anime_name: &str,
    ) -> Result<Vec<SubtitleSearchResult>> {
        let entry_url = format!("https://jimaku.cc/entry/{}", id);
        let mut results = Vec::new();

        let resp = match self.client.get(&entry_url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(results),
        };

        let html = match resp.text().await {
            Ok(t) => t,
            Err(_) => return Ok(results),
        };

        for cap in FILE_RE.captures_iter(&html) {
            let rel_url = &cap[1];
            let file_name = html_escape::decode_html_entities(&cap[2]).to_string();
            let full_url = format!("https://jimaku.cc{}", rel_url);

            let ext = if file_name.ends_with(".ass") {
                "ass"
            } else if file_name.ends_with(".srt") {
                "srt"
            } else if file_name.ends_with(".zip") {
                "zip"
            } else {
                "sub"
            };

            let display_title = format!("{} - {}", anime_name, file_name);

            results.push(SubtitleSearchResult {
                provider: "Jimaku.cc".to_string(),
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
