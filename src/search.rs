use anyhow::Result;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use inquire::Select;

use crate::provider::UnifiedAnimeEntry;

fn match_candidate(matcher: &SkimMatcherV2, entry: &UnifiedAnimeEntry, query: &str) -> Option<i64> {
    let mut best = matcher.fuzzy_match(&entry.name, query);

    if let Some(ref eng) = entry.english_name {
        if let Some(s) = matcher.fuzzy_match(eng, query) {
            best = Some(best.map_or(s, |p| p.max(s)));
        }
    }

    if let Some(ref jp) = entry.japanese_name {
        if let Some(s) = matcher.fuzzy_match(jp, query) {
            best = Some(best.map_or(s, |p| p.max(s)));
        }
    }

    best
}

pub fn score_entry(matcher: &SkimMatcherV2, entry: &UnifiedAnimeEntry, query: &str) -> Option<i64> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Some(0);
    }

    let full_score = match_candidate(matcher, entry, trimmed);

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() > 1 {
        let mut token_score_sum = 0i64;
        let mut all_matched = true;

        for w in &words {
            if let Some(s) = match_candidate(matcher, entry, w) {
                token_score_sum += s;
            } else {
                all_matched = false;
                break;
            }
        }

        if all_matched {
            return Some(full_score.map_or(token_score_sum, |fs| fs.max(token_score_sum)));
        }
    }

    full_score
}

pub fn fuzzy_filter_entries(entries: &[UnifiedAnimeEntry], query: &str) -> Vec<UnifiedAnimeEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return entries.iter().take(25).cloned().collect();
    }

    let matcher = SkimMatcherV2::default().ignore_case();
    let mut scored = Vec::new();

    for entry in entries {
        if let Some(score) = score_entry(&matcher, entry, trimmed) {
            scored.push((score, entry.clone()));
        }
    }

    scored.sort_by_key(|b| std::cmp::Reverse(b.0));
    scored.into_iter().map(|(_, e)| e).take(25).collect()
}

pub fn select_anime_entry(
    entries: Vec<UnifiedAnimeEntry>,
    default_query: &str,
) -> Result<Option<UnifiedAnimeEntry>> {
    let matcher = SkimMatcherV2::default().ignore_case();
    let trimmed_default = default_query.trim();

    let has_matches =
        !trimmed_default.is_empty() && !fuzzy_filter_entries(&entries, trimmed_default).is_empty();

    let scorer = |input: &str, entry: &UnifiedAnimeEntry, _val: &str, _idx: usize| -> Option<i64> {
        score_entry(&matcher, entry, input)
    };

    let mut prompt = Select::new("Search anime title (type to filter live):", entries)
        .with_scorer(&scorer)
        .with_page_size(10)
        .with_help_message("Type to filter live • ↑↓ navigate • Enter select • Esc cancel");

    if has_matches {
        prompt = prompt.with_starting_filter_input(trimmed_default);
    }

    match prompt.prompt() {
        Ok(entry) => Ok(Some(entry)),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(None),
        Err(e) => Err(anyhow::Error::from(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<UnifiedAnimeEntry> {
        vec![
            UnifiedAnimeEntry {
                name: "Yuru Camp△".to_string(),
                english_name: Some("Laid-Back Camp".to_string()),
                japanese_name: Some("ゆるキャン△".to_string()),
                jimaku_id: Some(681),
                kitsunekko_slug: Some("Yuru+Camp".to_string()),
            },
            UnifiedAnimeEntry {
                name: "Yuru Camp△ SEASON 2".to_string(),
                english_name: Some("LAID-BACK CAMP SEASON2".to_string()),
                japanese_name: Some("ゆるキャン△ SEASON２".to_string()),
                jimaku_id: Some(325),
                kitsunekko_slug: Some("Yuru+Camp+S2".to_string()),
            },
            UnifiedAnimeEntry {
                name: "Yuru Camp△ SEASON 2 OVA".to_string(),
                english_name: None,
                japanese_name: Some("ゆるキャン△ SEASON 2 OVA".to_string()),
                jimaku_id: Some(9442),
                kitsunekko_slug: None,
            },
            UnifiedAnimeEntry {
                name: "Yuru Camp△ SEASON 3".to_string(),
                english_name: Some("Laid-Back Camp Season 3".to_string()),
                japanese_name: Some("ゆるキャン△ SEASON３".to_string()),
                jimaku_id: Some(2350),
                kitsunekko_slug: Some("Yuru+Camp+S3".to_string()),
            },
            UnifiedAnimeEntry {
                name: "Ore wo Suki nano wa Omae dake ka yo".to_string(),
                english_name: Some("ORESUKI: Are you the only one who loves me?".to_string()),
                japanese_name: Some("俺を好きなのはお前だけかよ".to_string()),
                jimaku_id: Some(882),
                kitsunekko_slug: None,
            },
        ]
    }

    #[test]
    fn test_case_insensitive_matching_season() {
        let entries = sample_entries();
        let matches = fuzzy_filter_entries(&entries, "Yuru Camp Season 2");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].jimaku_id, Some(325));
    }

    #[test]
    fn test_multi_token_and_abbreviation_matching() {
        let entries = sample_entries();
        let matches = fuzzy_filter_entries(&entries, "yuru camp 2");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].jimaku_id, Some(325));
    }

    #[test]
    fn test_out_of_order_words() {
        let entries = sample_entries();
        let matches = fuzzy_filter_entries(&entries, "Season 2 Yuru Camp");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].jimaku_id, Some(325));
    }

    #[test]
    fn test_english_title_matching() {
        let entries = sample_entries();
        let matches = fuzzy_filter_entries(&entries, "laid back camp 2");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].jimaku_id, Some(325));
    }

    #[test]
    fn test_japanese_title_matching() {
        let entries = sample_entries();
        let matches = fuzzy_filter_entries(&entries, "ゆるキャン");
        assert_eq!(matches.len(), 4);
    }

    #[test]
    fn test_empty_query_returns_defaults() {
        let entries = sample_entries();
        let matches = fuzzy_filter_entries(&entries, "");
        assert_eq!(matches.len(), 5);
    }
}
