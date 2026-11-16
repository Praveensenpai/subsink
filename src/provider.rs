pub mod jimaku;
pub mod kitsunekko;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use zip::ZipArchive;

pub const CACHE_TTL_30_DAYS: Duration = Duration::from_secs(30 * 86400);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleSearchResult {
    pub provider: String,
    pub title: String,
    pub episode: Option<String>,
    pub language: String,
    pub download_url: String,
    pub file_format: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnifiedAnimeEntry {
    pub name: String,
    pub english_name: Option<String>,
    pub japanese_name: Option<String>,
    pub jimaku_id: Option<u64>,
    pub kitsunekko_slug: Option<String>,
}

impl fmt::Display for UnifiedAnimeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref jp) = self.japanese_name {
            write!(f, "{} ({})", self.name, jp)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

pub fn normalize_title(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub(crate) fn read_cached_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    ttl: Duration,
) -> Option<T> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age >= ttl {
        return None;
    }
    let file = File::open(path).ok()?;
    serde_json::from_reader(file).ok()
}

pub struct SubtitleProvider {
    client: reqwest::Client,
    jimaku: jimaku::JimakuProvider,
    kitsunekko: kitsunekko::KitsunekkoProvider,
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

        let jimaku = jimaku::JimakuProvider::new(client.clone(), cache_dir.clone());
        let kitsunekko = kitsunekko::KitsunekkoProvider::new(client.clone(), cache_dir);

        Self {
            client,
            jimaku,
            kitsunekko,
        }
    }

    /// Load or update indices from Jimaku & Kitsunekko and merge them into a unified list
    pub async fn ensure_cache(&self) -> Result<Vec<UnifiedAnimeEntry>> {
        let (jimaku_res, kitsunekko_res) =
            tokio::join!(self.jimaku.ensure_index(), self.kitsunekko.ensure_index());

        let jimaku_entries = jimaku_res.unwrap_or_default();
        let kitsunekko_entries = kitsunekko_res.unwrap_or_default();

        let mut kitsu_map: HashMap<String, String> = HashMap::new();
        for k in &kitsunekko_entries {
            let key = normalize_title(&k.name);
            if !key.is_empty() {
                kitsu_map.insert(key, k.slug.clone());
            }
        }

        let mut unified = Vec::with_capacity(jimaku_entries.len() + 500);

        for j in jimaku_entries {
            let norm_name = normalize_title(&j.name);
            let norm_en = j.english_name.as_deref().map(normalize_title);

            let matched_slug = kitsu_map
                .remove(&norm_name)
                .or_else(|| norm_en.and_then(|en| kitsu_map.remove(&en)));

            unified.push(UnifiedAnimeEntry {
                name: j.name,
                english_name: j.english_name,
                japanese_name: j.japanese_name,
                jimaku_id: Some(j.id),
                kitsunekko_slug: matched_slug,
            });
        }

        for k in kitsunekko_entries {
            let key = normalize_title(&k.name);
            if kitsu_map.contains_key(&key) {
                unified.push(UnifiedAnimeEntry {
                    name: k.name,
                    english_name: None,
                    japanese_name: None,
                    jimaku_id: None,
                    kitsunekko_slug: Some(k.slug),
                });
            }
        }

        Ok(unified)
    }

    /// Fetch files for an anime from all linked providers (Jimaku and/or Kitsunekko)
    pub async fn fetch_entry_files(
        &self,
        entry: &UnifiedAnimeEntry,
        target_ep: Option<&str>,
        is_special: bool,
    ) -> Result<Vec<SubtitleSearchResult>> {
        let jimaku_future = async {
            if let Some(id) = entry.jimaku_id {
                self.jimaku
                    .fetch_files(id, &entry.name)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        };

        let kitsu_future = async {
            if let Some(ref slug) = entry.kitsunekko_slug {
                self.kitsunekko
                    .fetch_files(slug, &entry.name)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        };

        let (jimaku_files, kitsu_files) = tokio::join!(jimaku_future, kitsu_future);

        let mut combined = jimaku_files;
        combined.extend(kitsu_files);

        Ok(Self::filter_subtitles(combined, target_ep, is_special))
    }

    pub fn filter_subtitles(
        files: Vec<SubtitleSearchResult>,
        target_ep: Option<&str>,
        is_special: bool,
    ) -> Vec<SubtitleSearchResult> {
        if files.len() <= 1 {
            return files;
        }

        let is_special_file = |title: &str| -> bool {
            let l = title.to_lowercase();
            l.contains("sp") || l.contains("ova") || l.contains("special") || l.contains("extra")
        };

        if let Some(ep) = target_ep {
            let ep_padded = if ep.len() == 1 {
                format!("0{}", ep)
            } else {
                ep.to_string()
            };

            let filtered: Vec<SubtitleSearchResult> = files
                .iter()
                .filter(|r| {
                    r.title.contains(ep)
                        || r.title.contains(&ep_padded)
                        || r.title.contains(&format!("E{}", ep_padded))
                        || r.title.contains(&format!("e{}", ep_padded))
                        || is_special_file(&r.title)
                })
                .cloned()
                .collect();

            if !filtered.is_empty() {
                return filtered;
            }
        }

        if is_special {
            let specials: Vec<SubtitleSearchResult> = files
                .iter()
                .filter(|r| is_special_file(&r.title))
                .cloned()
                .collect();
            if !specials.is_empty() {
                return specials;
            }
        }

        files
    }

    /// Download subtitle file to target destination
    pub async fn download_subtitle(
        &self,
        result: &SubtitleSearchResult,
        dest_dir: &Path,
    ) -> Result<PathBuf> {
        let resp = self.client.get(&result.download_url).send().await?;
        let bytes = resp.bytes().await?;

        let temp_filename = format!("temp_sub.{}", result.file_format);
        let temp_path = dest_dir.join(&temp_filename);
        let mut file = File::create(&temp_path)?;
        file.write_all(&bytes)?;

        if result.file_format != "zip" {
            return Ok(temp_path);
        }

        let zip_file = File::open(&temp_path)?;
        let mut archive = ZipArchive::new(zip_file)?;

        for i in 0..archive.len() {
            let mut file_in_zip = archive.by_index(i)?;
            let Some(outpath) = file_in_zip.enclosed_name().map(|p| p.to_owned()) else {
                continue;
            };

            let ext = outpath.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext == "ass" || ext == "srt" {
                let Some(file_name) = outpath.file_name() else {
                    continue;
                };
                let extracted_path = dest_dir.join(file_name);
                let mut outfile = File::create(&extracted_path)?;
                std::io::copy(&mut file_in_zip, &mut outfile)?;
                let _ = std::fs::remove_file(&temp_path);
                return Ok(extracted_path);
            }
        }

        Err(anyhow!(
            "No .ass or .srt subtitle file found inside downloaded ZIP archive"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_sub(provider: &str, title: &str, url: &str) -> SubtitleSearchResult {
        SubtitleSearchResult {
            provider: provider.to_string(),
            title: title.to_string(),
            episode: None,
            language: "JP".to_string(),
            download_url: url.to_string(),
            file_format: "srt".to_string(),
        }
    }

    #[test]
    fn test_normalize_title_strips_punctuation_and_spaces() {
        assert_eq!(normalize_title("Heya Camp△"), "heyacamp");
        assert_eq!(normalize_title("Yuru Camp△ S2!"), "yurucamps2");
        assert_eq!(normalize_title("Solo Camping for Two"), "solocampingfortwo");
    }

    #[test]
    fn test_filter_subtitles_matches_target_episode() {
        let files = vec![
            mock_sub("Jimaku", "Oresuki - S01E01.srt", "https://jimaku.cc/dl/1"),
            mock_sub("Jimaku", "Oresuki - S01E02.srt", "https://jimaku.cc/dl/2"),
        ];

        let filtered = SubtitleProvider::filter_subtitles(files, Some("02"), false);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Oresuki - S01E02.srt");
    }

    #[test]
    fn test_filter_subtitles_always_preserves_specials() {
        let files = vec![
            mock_sub("Jimaku", "Heya CAMP△ - 01.ass", "https://jimaku.cc/1"),
            mock_sub(
                "Kitsunekko",
                "Heya Camp - SP.srt",
                "https://kitsunekko.net/sp",
            ),
        ];

        let filtered = SubtitleProvider::filter_subtitles(files.clone(), Some("01"), false);
        assert_eq!(filtered.len(), 2);

        let filtered_sp = SubtitleProvider::filter_subtitles(files, Some("01"), true);
        assert_eq!(filtered_sp.len(), 2);
    }

    #[test]
    fn test_merge_unified_entries_links_counterparts() {
        let jimaku = vec![jimaku::CachedJimakuEntry {
            id: 451,
            name: "Heya Camp△".to_string(),
            english_name: Some("ROOM CAMP".to_string()),
            japanese_name: Some("へやキャン△".to_string()),
        }];
        let kitsu = vec![kitsunekko::CachedKitsunekkoEntry {
            slug: "Heya+Camp".to_string(),
            name: "Heya Camp".to_string(),
        }];

        let mut kitsu_map: HashMap<String, String> = HashMap::new();
        for k in &kitsu {
            kitsu_map.insert(normalize_title(&k.name), k.slug.clone());
        }

        let mut unified = Vec::new();
        for j in jimaku {
            let norm_name = normalize_title(&j.name);
            let matched_slug = kitsu_map.remove(&norm_name);
            unified.push(UnifiedAnimeEntry {
                name: j.name,
                english_name: j.english_name,
                japanese_name: j.japanese_name,
                jimaku_id: Some(j.id),
                kitsunekko_slug: matched_slug,
            });
        }

        assert_eq!(unified.len(), 1);
        assert_eq!(unified[0].jimaku_id, Some(451));
        assert_eq!(unified[0].kitsunekko_slug.as_deref(), Some("Heya+Camp"));
    }

    #[tokio::test]
    async fn test_live_heya_camp_fetches_both_providers() {
        let provider = SubtitleProvider::new();
        let entries = provider.ensure_cache().await.unwrap();
        let heya = entries
            .into_iter()
            .find(|e| e.name.contains("Heya Camp"))
            .expect("Heya Camp should be in unified index");

        assert!(heya.jimaku_id.is_some(), "Should link to Jimaku ID 451");
        assert!(
            heya.kitsunekko_slug.is_some(),
            "Should link to Kitsunekko slug"
        );

        let files = provider
            .fetch_entry_files(&heya, Some("01"), true)
            .await
            .unwrap();
        let has_jimaku = files.iter().any(|f| f.provider == "Jimaku.cc");
        let has_kitsunekko = files
            .iter()
            .any(|f| f.provider == "Kitsunekko" && f.title.contains("SP"));

        assert!(has_jimaku, "Should have Jimaku episode files");
        assert!(has_kitsunekko, "Should have Kitsunekko SP file");
    }
}
