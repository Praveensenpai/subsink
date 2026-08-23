use anyhow::{anyhow, Result};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleSearchResult {
    pub provider: String,
    pub title: String,
    pub episode: Option<String>,
    pub language: String, // "JP" or "EN"
    pub download_url: String,
    pub file_format: String, // "ass", "srt", "zip"
}

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

pub struct SubtitleProvider {
    client: reqwest::Client,
    cache_dir: PathBuf,
}

impl SubtitleProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("./"))
            .join("subsink");
        let _ = std::fs::create_dir_all(&cache_dir);

        Self { client, cache_dir }
    }

    /// Load or update local Jimaku index cache (~/.cache/subsink/jimaku_index.json) - 30 days cache
    pub async fn ensure_jimaku_cache(&self) -> Result<Vec<CachedJimakuEntry>> {
        let cache_path = self.cache_dir.join("jimaku_index.json");
        let ttl_30_days = Duration::from_secs(30 * 86400);

        if cache_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&cache_path) {
                if let Ok(modified) = metadata.modified() {
                    if SystemTime::now().duration_since(modified).unwrap_or(Duration::from_secs(0)) < ttl_30_days {
                        if let Ok(file) = File::open(&cache_path) {
                            if let Ok(entries) = serde_json::from_reader::<_, Vec<CachedJimakuEntry>>(file) {
                                if !entries.is_empty() {
                                    return Ok(entries);
                                }
                            }
                        }
                    }
                }
            }
        }

        let entries = self.fetch_jimaku_index().await?;
        if !entries.is_empty() {
            if let Ok(file) = File::create(&cache_path) {
                let _ = serde_json::to_writer(file, &entries);
            }
        }

        Ok(entries)
    }

    async fn fetch_jimaku_index(&self) -> Result<Vec<CachedJimakuEntry>> {
        let resp = self.client.get("https://jimaku.cc/").send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let html_doc = resp.text().await?;
        let entry_re = Regex::new(r#"<div class="entry" data-extra="([^"]+)">\s*<a href="/entry/(\d+)"[^>]*>(.*?)</a>"#).unwrap();
        
        let mut entries = Vec::new();

        for cap in entry_re.captures_iter(&html_doc) {
            let data_extra_raw = &cap[1];
            let id_str = &cap[2];
            let title_raw = &cap[3];

            let id: u64 = id_str.parse().unwrap_or(0);
            if id == 0 {
                continue;
            }

            let decoded_extra = html_escape::decode_html_entities(data_extra_raw);
            let extra: Option<JimakuDataExtra> = serde_json::from_str(&decoded_extra).ok();
            let title_decoded = html_escape::decode_html_entities(title_raw).to_string();

            let name = extra.as_ref().and_then(|e| e.name.clone()).unwrap_or(title_decoded);
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

    /// Perform fast fuzzy matching against the cached entries
    pub fn fuzzy_filter_entries(entries: &[CachedJimakuEntry], query: &str) -> Vec<CachedJimakuEntry> {
        if query.trim().is_empty() {
            return entries.iter().take(20).cloned().collect();
        }

        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, CachedJimakuEntry)> = Vec::new();

        for entry in entries {
            let mut best_score = None;

            if let Some(s) = matcher.fuzzy_match(&entry.name, query) {
                best_score = Some(s);
            }
            if let Some(ref eng) = entry.english_name {
                if let Some(s) = matcher.fuzzy_match(eng, query) {
                    best_score = Some(best_score.map_or(s, |prev| prev.max(s)));
                }
            }
            if let Some(ref jp) = entry.japanese_name {
                if let Some(s) = matcher.fuzzy_match(jp, query) {
                    best_score = Some(best_score.map_or(s, |prev| prev.max(s)));
                }
            }

            if let Some(score) = best_score {
                scored.push((score, entry.clone()));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().map(|(_, entry)| entry).take(25).collect()
    }

    /// Fetch files for a selected entry ID with 30 days local cache (~/.cache/subsink/entry_{id}_files.json)
    pub async fn fetch_entry_files(&self, entry: &CachedJimakuEntry, target_ep: Option<&str>) -> Result<Vec<SubtitleSearchResult>> {
        let cache_path = self.cache_dir.join(format!("entry_{}_files.json", entry.id));
        let ttl_30_days = Duration::from_secs(30 * 86400);

        let all_files: Vec<SubtitleSearchResult> = if cache_path.exists() {
            let should_read = std::fs::metadata(&cache_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|mod_time| SystemTime::now().duration_since(mod_time).unwrap_or_default() < ttl_30_days)
                .unwrap_or(false);

            if should_read {
                if let Ok(file) = File::open(&cache_path) {
                    serde_json::from_reader(file).unwrap_or_default()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let files_to_use = if all_files.is_empty() {
            let fetched = self.scrape_entry_files(entry).await?;
            if !fetched.is_empty() {
                if let Ok(file) = File::create(&cache_path) {
                    let _ = serde_json::to_writer(file, &fetched);
                }
            }
            fetched
        } else {
            all_files
        };

        Ok(Self::filter_subtitles(files_to_use, target_ep))
    }

    pub fn filter_subtitles(files_to_use: Vec<SubtitleSearchResult>, target_ep: Option<&str>) -> Vec<SubtitleSearchResult> {
        if let Some(ep) = target_ep {
            if files_to_use.len() > 1 {
                let ep_padded = if ep.len() == 1 { format!("0{}", ep) } else { ep.to_string() };
                let filtered: Vec<SubtitleSearchResult> = files_to_use
                    .iter()
                    .filter(|r| {
                        r.title.contains(ep)
                            || r.title.contains(&ep_padded)
                            || r.title.contains(&format!("E{}", ep_padded))
                            || r.title.contains(&format!("e{}", ep_padded))
                    })
                    .cloned()
                    .collect();

                if !filtered.is_empty() {
                    return filtered;
                }
            }
        }

        files_to_use
    }

    async fn scrape_entry_files(&self, entry: &CachedJimakuEntry) -> Result<Vec<SubtitleSearchResult>> {
        let entry_url = format!("https://jimaku.cc/entry/{}", entry.id);
        let mut results = Vec::new();

        if let Ok(resp) = self.client.get(&entry_url).send().await {
            if let Ok(html) = resp.text().await {
                let file_re = Regex::new(r#"href="(/entry/\d+/download/[^"]+)"[^>]*>(.*?)</a>"#).unwrap();
                for cap in file_re.captures_iter(&html) {
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

                    let display_title = format!("{} - {}", entry.name, file_name);

                    results.push(SubtitleSearchResult {
                        provider: "Jimaku.cc".to_string(),
                        title: display_title,
                        episode: None,
                        language: "JP".to_string(),
                        download_url: full_url,
                        file_format: ext.to_string(),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Download subtitle file to target destination
    pub async fn download_subtitle(&self, result: &SubtitleSearchResult, dest_dir: &Path) -> Result<PathBuf> {
        let resp = self.client.get(&result.download_url).send().await?;
        let bytes = resp.bytes().await?;

        let temp_filename = format!("temp_sub.{}", result.file_format);
        let temp_path = dest_dir.join(&temp_filename);
        let mut file = File::create(&temp_path)?;
        file.write_all(&bytes)?;

        if result.file_format == "zip" {
            let zip_file = File::open(&temp_path)?;
            let mut archive = ZipArchive::new(zip_file)?;

            for i in 0..archive.len() {
                let mut file_in_zip = archive.by_index(i)?;
                let outpath = match file_in_zip.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue,
                };

                let ext = outpath.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "ass" || ext == "srt" {
                    let extracted_path = dest_dir.join(outpath.file_name().unwrap());
                    let mut outfile = File::create(&extracted_path)?;
                    std::io::copy(&mut file_in_zip, &mut outfile)?;
                    let _ = std::fs::remove_file(&temp_path);
                    return Ok(extracted_path);
                }
            }
            Err(anyhow!("No .ass or .srt subtitle file found inside downloaded ZIP archive"))
        } else {
            Ok(temp_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_subtitles_matches_target_episode() {
        let files = vec![
            SubtitleSearchResult {
                provider: "Jimaku.cc".to_string(),
                title: "Oresuki - S01E01.srt".to_string(),
                episode: None,
                language: "JP".to_string(),
                download_url: "https://jimaku.cc/dl/1".to_string(),
                file_format: "srt".to_string(),
            },
            SubtitleSearchResult {
                provider: "Jimaku.cc".to_string(),
                title: "Oresuki - S01E02.srt".to_string(),
                episode: None,
                language: "JP".to_string(),
                download_url: "https://jimaku.cc/dl/2".to_string(),
                file_format: "srt".to_string(),
            },
        ];

        let filtered = SubtitleProvider::filter_subtitles(files.clone(), Some("02"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Oresuki - S01E02.srt");
    }

    #[test]
    fn test_filter_subtitles_falls_back_when_no_episode_matches() {
        let files = vec![
            SubtitleSearchResult {
                provider: "Jimaku.cc".to_string(),
                title: "Oresuki - OVA Part 1.srt".to_string(),
                episode: None,
                language: "JP".to_string(),
                download_url: "https://jimaku.cc/dl/1".to_string(),
                file_format: "srt".to_string(),
            },
            SubtitleSearchResult {
                provider: "Jimaku.cc".to_string(),
                title: "Oresuki - OVA Part 2.srt".to_string(),
                episode: None,
                language: "JP".to_string(),
                download_url: "https://jimaku.cc/dl/2".to_string(),
                file_format: "srt".to_string(),
            },
        ];

        let filtered = SubtitleProvider::filter_subtitles(files.clone(), Some("13"));
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_subtitles_single_file_standalone_ova() {
        let files = vec![SubtitleSearchResult {
            provider: "Jimaku.cc".to_string(),
            title: "Ore wo Suki nano wa Omae dake ka yo: Oretachi no Game Set - [Erai-raws] Oresuki - Oretachi no Game Set (Whisper AI).srt".to_string(),
            episode: None,
            language: "JP".to_string(),
            download_url: "https://jimaku.cc/dl/10420".to_string(),
            file_format: "srt".to_string(),
        }];

        let filtered = SubtitleProvider::filter_subtitles(files.clone(), Some("13"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, files[0].title);
    }
}
