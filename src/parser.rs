use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

static SEASON_EP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bS(\d{1,2})[\s\-_.]*E(\d{1,4})\b").expect("Valid regex syntax")
});

static SEASON_FOLDER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:Season\s*|S)(\d{1,2})$").expect("Valid regex syntax"));

static BRACKET_EP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\[(\d{1,4})(?:v\d)?\]").expect("Valid regex syntax"));

static EP_END_MARKER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[\s\-_]+(\d{1,4})(?:v\d)?\s+(?:END|FINAL|FIN)\b").expect("Valid regex syntax")
});

static EP_SUBTITLE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[\s\-_]+(\d{1,4})(?:v\d)?[\s\-_]+.*$").expect("Valid regex syntax")
});

static EP_AT_END_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\s\-_]+(\d{1,4})(?:v\d)?$").expect("Valid regex syntax"));

static EP_PREFIX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:Episode|Ep|E)[\s\-_.]*(\d{1,4})(?:v\d)?\b").expect("Valid regex syntax")
});

static CLEAN_BRACKETS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]*\]|\([^\)]*\)").expect("Valid regex syntax"));

static TECH_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(1080p|720p|480p|2160p|4k|x264|x265|h264|h265|hevc|avc|av1|10bit|8bit|hi10p|ma10p|flac|aac|opus|ac3|dts|mp3|dvd|bd|bluray|blu-ray|bdrip|web-dl|webrip|hdtv|japanese)\b")
        .expect("Valid regex syntax")
});

static END_MARKER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(END|FINAL|FIN)\b").expect("Valid regex syntax"));

static HAS_SEASON_WORD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:season|\d+(?:st|nd|rd|th))\b").expect("Valid regex syntax")
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimeMetadata {
    pub title: String,
    pub season: Option<u32>,
    pub episode: Option<String>,
}

fn extract_season_from_folder(path: &Path) -> Option<u32> {
    let parent = path.parent()?;
    let folder_name = parent.file_name()?.to_str()?;
    SEASON_FOLDER_REGEX
        .captures(folder_name)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

fn extract_components(
    stem: &str,
    folder_season: Option<u32>,
) -> (Option<u32>, Option<String>, String) {
    if let Some(caps) = SEASON_EP_REGEX.captures(stem) {
        let season = caps.get(1).and_then(|m| m.as_str().parse().ok());
        let episode = caps.get(2).map(|m| m.as_str().to_string());
        let base = caps.get(0).map_or(stem, |m| &stem[..m.start()]);
        return (season.or(folder_season), episode, base.to_string());
    }

    if let Some(caps) = BRACKET_EP_REGEX.captures(stem) {
        let episode = caps.get(1).map(|m| m.as_str().to_string());
        return (folder_season, episode, stem.to_string());
    }

    if let Some(caps) = EP_END_MARKER_REGEX.captures(stem) {
        let episode = caps.get(1).map(|m| m.as_str().to_string());
        let base = caps.get(0).map_or(stem, |m| &stem[..m.start()]);
        return (folder_season, episode, base.to_string());
    }

    if let Some(caps) = EP_SUBTITLE_REGEX.captures(stem) {
        let episode = caps.get(1).map(|m| m.as_str().to_string());
        let base = caps.get(0).map_or(stem, |m| &stem[..m.start()]);
        return (folder_season, episode, base.to_string());
    }

    if let Some(caps) = EP_AT_END_REGEX.captures(stem) {
        let episode = caps.get(1).map(|m| m.as_str().to_string());
        let base = caps.get(0).map_or(stem, |m| &stem[..m.start()]);
        return (folder_season, episode, base.to_string());
    }

    if let Some(caps) = EP_PREFIX_REGEX.captures(stem) {
        let episode = caps.get(1).map(|m| m.as_str().to_string());
        let base = caps.get(0).map_or(stem, |m| &stem[..m.start()]);
        return (folder_season, episode, base.to_string());
    }

    (folder_season, None, stem.to_string())
}

fn clean_title_tags(raw: &str) -> String {
    let without_brackets = CLEAN_BRACKETS_REGEX.replace_all(raw, "");
    let without_tech = TECH_REGEX.replace_all(&without_brackets, "");
    let without_end = END_MARKER_REGEX.replace_all(&without_tech, "");
    without_end
        .trim()
        .trim_matches(&['-', '_', '.', ' '] as &[char])
        .to_string()
}

fn format_search_title(base_title: &str, season: Option<u32>) -> String {
    if let Some(s) = season {
        if s > 1 && !HAS_SEASON_WORD_REGEX.is_match(base_title) {
            return format!("{} Season {}", base_title, s);
        }
    }
    base_title.to_string()
}

fn resolve_fallback_title(path: &Path, folder_season: Option<u32>, stem: &str) -> String {
    if folder_season.is_some() {
        if let Some(grandparent) = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
        {
            let gp_str = grandparent.to_string_lossy().trim().to_string();
            if !gp_str.is_empty() {
                return gp_str;
            }
        }
    }

    if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
        let parent_str = parent.to_string_lossy().trim().to_string();
        if !parent_str.is_empty() {
            return parent_str;
        }
    }

    stem.to_string()
}

pub fn parse_anime_filename(file_path: &Path) -> AnimeMetadata {
    let stem = file_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let folder_season = extract_season_from_folder(file_path);
    let (season, episode, raw_base) =
        if stem.chars().all(|c| c.is_ascii_digit()) && !stem.is_empty() {
            (folder_season, Some(stem.clone()), String::new())
        } else {
            extract_components(&stem, folder_season)
        };

    let cleaned_base = clean_title_tags(&raw_base);
    let base_title = if cleaned_base.is_empty() {
        resolve_fallback_title(file_path, folder_season, &stem)
    } else {
        cleaned_base
    };

    let final_title = format_search_title(&base_title, season);

    AnimeMetadata {
        title: final_title,
        season,
        episode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_vcb_studio_parsing() {
        let p = PathBuf::from(
            "[VCB-Studio] Ore wo Suki nano wa Omae dake ka yo [01][Ma10p_1080p][x265_flac_aac].mkv",
        );
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Ore wo Suki nano wa Omae dake ka yo");
        assert_eq!(meta.season, None);
        assert_eq!(meta.episode, Some("01".to_string()));
    }

    #[test]
    fn test_yuru_camp_season_2_scene_format() {
        let p =
            PathBuf::from("/home/paisen/Videos/Anime/Yuru Camp/Season 02/Yuru Camp - S02E01.mkv");
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Yuru Camp Season 2");
        assert_eq!(meta.season, Some(2));
        assert_eq!(meta.episode, Some("01".to_string()));
    }

    #[test]
    fn test_episode_at_end_standard() {
        let p = PathBuf::from("/home/paisen/Videos/Anime/Yuru Camp/Yuru Camp - 01.koto");
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Yuru Camp");
        assert_eq!(meta.season, None);
        assert_eq!(meta.episode, Some("01".to_string()));
    }

    #[test]
    fn test_episode_end_marker() {
        let p = PathBuf::from("/home/paisen/Videos/Anime/Yuru Camp/Yuru Camp - 12 END.koto");
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Yuru Camp");
        assert_eq!(meta.season, None);
        assert_eq!(meta.episode, Some("12".to_string()));
    }

    #[test]
    fn test_specials_with_episode_subtitle() {
        let p = PathBuf::from(
            "/home/paisen/Videos/Anime/Yuru Camp△ Specials/Yuru Camp△ Specials - 01 - へやキャン△episode0.mkv",
        );
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Yuru Camp△ Specials");
        assert_eq!(meta.season, None);
        assert_eq!(meta.episode, Some("01".to_string()));
    }

    #[test]
    fn test_s01e01_with_tags() {
        let p = PathBuf::from(
            "/home/paisen/Videos/Anime/Non Non Biyori/Season 01/Non Non Biyori - S01E01 [Japanese] [1080p].koto",
        );
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Non Non Biyori");
        assert_eq!(meta.season, Some(1));
        assert_eq!(meta.episode, Some("01".to_string()));
    }

    #[test]
    fn test_fallback_folder_resolution_for_bare_number() {
        let p = PathBuf::from("/home/paisen/Videos/Anime/Yuru Camp/Season 02/01.mkv");
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Yuru Camp Season 2");
        assert_eq!(meta.season, Some(2));
        assert_eq!(meta.episode, Some("01".to_string()));
    }

    #[test]
    fn test_all_files_in_user_videos_if_present() {
        let base = Path::new("/home/paisen/Videos/Anime");
        if !base.exists() {
            return;
        }
        let mut count = 0;
        for entry in walkdir::WalkDir::new(base)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "mkv" || ext == "koto" {
                    let meta = parse_anime_filename(path);
                    assert!(
                        !meta.title.is_empty(),
                        "Title should not be empty for {:?}",
                        path
                    );
                    assert!(
                        meta.episode.is_some(),
                        "Episode should be extracted for {:?}",
                        path
                    );
                    count += 1;
                }
            }
        }
        assert_eq!(count, 52, "Expected to verify all 52 anime files");
    }
}
