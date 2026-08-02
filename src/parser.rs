use regex::Regex;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AnimeMetadata {
    pub raw_filename: String,
    pub title: String,
    pub episode: Option<String>,
    pub extension: String,
}

pub fn parse_anime_filename(file_path: &Path) -> AnimeMetadata {
    let filename = file_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let stem = file_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| filename.clone());

    let ext = file_path
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "mkv".to_string());

    // 1. Extract episode number if present
    // Patterns: [01], - 01, S01E01, E01, [01v2]
    let ep_regex = Regex::new(r"(?i)(?:\[(\d{1,3})(?:v\d)?\]|[\s\-_]+(\d{1,3})[\s\-_]|E(\d{1,3})|S\d+E(\d{1,3}))").unwrap();
    let episode = ep_regex.captures(&stem).and_then(|caps| {
        caps.get(1)
            .or_else(|| caps.get(2))
            .or_else(|| caps.get(3))
            .or_else(|| caps.get(4))
            .map(|m| m.as_str().to_string())
    });

    // 2. Clean title: remove release group brackets [...] and technical tags
    let clean_regex = Regex::new(r"\[[^\]]*\]|\([^\)]*\)").unwrap();
    let cleaned = clean_regex.replace_all(&stem, "");
    
    // Remove resolution, video codec tags if any remaining
    let tech_regex = Regex::new(r"(?i)(1080p|720p|480p|x264|x265|hevc|flac|aac|ma10p|dvd|bd|web-dl)").unwrap();
    let cleaned_no_tech = tech_regex.replace_all(&cleaned, "");

    // Remove episode numbers from title string
    let ep_clean_regex = Regex::new(r"(?i)(?:\bE\d{1,3}\b|[\s\-_]+\d{1,3}\b)").unwrap();
    let final_title = ep_clean_regex.replace_all(&cleaned_no_tech, "");

    let title = final_title
        .trim()
        .trim_matches(&['-', '_', ' '] as &[char])
        .to_string();

    let fallback_title = if title.is_empty() { stem } else { title };

    AnimeMetadata {
        raw_filename: filename,
        title: fallback_title,
        episode,
        extension: ext,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_vcb_studio_parsing() {
        let p = PathBuf::from("[VCB-Studio] Ore wo Suki nano wa Omae dake ka yo [01][Ma10p_1080p][x265_flac_aac].mkv");
        let meta = parse_anime_filename(&p);
        assert_eq!(meta.title, "Ore wo Suki nano wa Omae dake ka yo");
        assert_eq!(meta.episode, Some("01".to_string()));
    }
}
