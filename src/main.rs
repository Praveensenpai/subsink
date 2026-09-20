mod parser;
mod provider;
mod scanner;
mod syncer;
mod ui;

use anyhow::{bail, Result};
use inquire::Autocomplete;
use inquire::CustomUserError;
use inquire::{Select, Text};
use std::path::Path;

#[derive(Clone)]
struct JimakuAutocompleter {
    entries: Vec<provider::CachedJimakuEntry>,
}

impl Autocomplete for JimakuAutocompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        let matches = provider::SubtitleProvider::fuzzy_filter_entries(&self.entries, input);
        Ok(matches
            .into_iter()
            .map(|e| {
                if let Some(ref jp) = e.japanese_name {
                    format!("{} ({})", e.name, jp)
                } else {
                    e.name
                }
            })
            .collect())
    }

    fn get_completion(
        &mut self,
        _input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Option<String>, CustomUserError> {
        Ok(highlighted_suggestion)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    match parse_cli_args(&std::env::args().skip(1).collect::<Vec<_>>())? {
        CliAction::Run => {}
        CliAction::Help => {
            print_help();
            return Ok(());
        }
        CliAction::Version => {
            println!("subsink {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    }

    inquire::set_global_render_config(ui::custom_render_config());
    ui::print_banner();

    // STEP 1: Select Anime Video File
    ui::print_step(1, 4, "Select Raw Anime Video File");
    let video_path = scanner::select_video_file()?;
    let file_display_name = video_path
        .file_name()
        .map(|f| f.to_string_lossy())
        .unwrap_or_else(|| "video".into());
    ui::print_success(&format!("Selected File: {}", file_display_name));

    let parent_dir = video_path.parent().unwrap_or_else(|| Path::new("."));
    let video_stem = video_path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "video".into());

    // STEP 2: Load Jimaku Index & Live Search
    ui::print_step(2, 4, "Search Subtitle Database (Live Fuzzy Match)");
    let spinner = ui::create_spinner("Loading Jimaku subtitle index...");
    let provider = provider::SubtitleProvider::new();
    let entries = provider.ensure_jimaku_cache().await?;
    spinner.finish_and_clear();
    ui::print_success(&format!(
        "Loaded {} anime titles into local index",
        entries.len()
    ));

    let meta = parser::parse_anime_filename(&video_path);
    let default_query = meta.title.clone();

    let autocompleter = JimakuAutocompleter {
        entries: entries.clone(),
    };

    let selected_anime_str = Text::new("Search anime title (type to filter live):")
        .with_default(&default_query)
        .with_autocomplete(autocompleter)
        .prompt()?;

    let matched_entry = resolve_entry(&entries, &selected_anime_str);

    let entry = match matched_entry {
        Some(e) => e,
        None => {
            ui::print_warning("No matching anime entry selected.");
            return Ok(());
        }
    };

    ui::print_success(&format!("Matched Anime: {}", entry.name));

    // STEP 3: Fetch Files for Matched Anime (Cached for 30 Days)
    ui::print_step(3, 4, "Fetching Available Subtitle Files (30 Days Cache)");
    let fetch_spinner = ui::create_spinner("Loading episode subtitle files...");
    let results = provider
        .fetch_entry_files(entry, meta.episode.as_deref())
        .await?;
    fetch_spinner.finish_and_clear();

    if results.is_empty() {
        ui::print_warning("No subtitle files found for this anime entry.");
        return Ok(());
    }

    let display_options: Vec<String> = results
        .iter()
        .map(|r| format!("[{}] {}", r.language, r.title))
        .collect();

    let selected_display =
        Select::new("Choose subtitle file to download & sync:", display_options).prompt()?;
    let choice_idx = results
        .iter()
        .position(|r| format!("[{}] {}", r.language, r.title) == selected_display)
        .unwrap_or(0);

    let selected_sub = &results[choice_idx];

    // STEP 4: Download & Subtitle Timing Action
    ui::print_step(4, 4, "Download & Subtitle Timing");
    let download_spinner = ui::create_spinner("Downloading subtitle file...");
    let temp_sub = provider.download_subtitle(selected_sub, parent_dir).await?;
    download_spinner.finish_and_clear();
    ui::print_success("Subtitle downloaded successfully.");

    let sub_ext = temp_sub
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("srt");
    let target_sub_filename = format!("{}.ja.{}", video_stem, sub_ext);
    let target_sub_path = parent_dir.join(&target_sub_filename);

    let timing_options = vec![
        "⚡ Auto-Sync with ALASS (Audio Voice Alignment)",
        "⏱️  Manual offset shift (e.g. +500ms, -1.2s)",
        "📁 Keep original (direct download, no sync)",
    ];

    let selected_action = Select::new("Choose subtitle timing action:", timing_options).prompt()?;

    if selected_action.starts_with("⚡ Auto-Sync") {
        let sync_spinner = ui::create_spinner("Aligning subtitle timings with audio via ALASS...");
        let syncer = syncer::SubtitleSyncer::new();
        let sync_res = syncer.sync_subtitle(&video_path, &temp_sub, &target_sub_path)?;
        sync_spinner.finish_and_clear();

        match sync_res {
            syncer::SyncResult::Success(shift_summary) => {
                ui::print_success(&format!("ALASS Alignment: {}", shift_summary));
            }
            syncer::SyncResult::WarningLargeShift(shift_summary) => {
                ui::print_warning(&format!(
                    "ALASS produced an unusually large shift: {}",
                    shift_summary
                ));
                ui::print_warning(
                    "Preserving downloaded subtitle without sync to prevent corruption.",
                );
                std::fs::copy(&temp_sub, &target_sub_path)?;
            }
            syncer::SyncResult::DirectCopy => {
                ui::print_info("Subtitle file saved directly alongside raw video.");
            }
        }
    } else if selected_action.starts_with("⏱️") {
        let offset_str = Text::new("Enter time offset (e.g. +500ms, -1.2s, 800):")
            .with_default("0")
            .prompt()?;
        let offset_ms = syncer::parse_offset_string(&offset_str)?;
        syncer::apply_manual_offset(&temp_sub, &target_sub_path, offset_ms)?;
        ui::print_success(&format!("Applied manual offset: {:+}ms", offset_ms));
    } else {
        std::fs::copy(&temp_sub, &target_sub_path)?;
        ui::print_info("Original subtitle saved directly without timing modifications.");
    }

    if temp_sub != target_sub_path {
        let _ = std::fs::remove_file(&temp_sub);
    }

    ui::print_success(&format!("Successfully saved: {}", target_sub_filename));
    ui::print_info(&format!("Saved to: {}", target_sub_path.display()));
    println!();

    Ok(())
}

enum CliAction {
    Run,
    Help,
    Version,
}

fn parse_cli_args(args: &[String]) -> Result<CliAction> {
    match args {
        [] => Ok(CliAction::Run),
        [flag] if matches!(flag.as_str(), "-h" | "--help") => Ok(CliAction::Help),
        [flag] if matches!(flag.as_str(), "-v" | "-V" | "--version") => Ok(CliAction::Version),
        [argument, ..] => bail!("Unknown argument: {argument}\n\nRun `subsink --help` for usage."),
    }
}

fn print_help() {
    println!(
        "subsink {}\n\nUsage: subsink [OPTION]\n\nOptions:\n  -h, --help       Show this help message\n  -v, -V, --version  Show the version",
        env!("CARGO_PKG_VERSION")
    );
}

fn resolve_entry<'a>(
    entries: &'a [provider::CachedJimakuEntry],
    selected: &str,
) -> Option<&'a provider::CachedJimakuEntry> {
    let trimmed = selected.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Exact match against formatted suggestion "name (japanese_name)" or "name"
    if let Some(entry) = entries.iter().find(|e| {
        let formatted = if let Some(ref jp) = e.japanese_name {
            format!("{} ({})", e.name, jp)
        } else {
            e.name.clone()
        };
        trimmed == formatted
    }) {
        return Some(entry);
    }

    // 2. Exact match against name, english_name, or japanese_name
    if let Some(entry) = entries.iter().find(|e| {
        trimmed.eq_ignore_ascii_case(&e.name)
            || e.english_name
                .as_deref()
                .is_some_and(|eng| trimmed.eq_ignore_ascii_case(eng))
            || (e.japanese_name.as_deref() == Some(trimmed))
    }) {
        return Some(entry);
    }

    // 3. Fallback: fuzzy filter / highest scoring entry
    let fuzzy_top = provider::SubtitleProvider::fuzzy_filter_entries(entries, trimmed);
    if let Some(top) = fuzzy_top.into_iter().next() {
        return entries.iter().find(|e| e.id == top.id);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{parse_cli_args, resolve_entry, CliAction};
    use crate::provider::CachedJimakuEntry;

    #[test]
    fn recognizes_help_and_version_flags() {
        assert!(matches!(
            parse_cli_args(&["--help".to_string()]).unwrap(),
            CliAction::Help
        ));
        assert!(matches!(
            parse_cli_args(&["-v".to_string()]).unwrap(),
            CliAction::Version
        ));
        assert!(parse_cli_args(&["--unknown".to_string()]).is_err());
    }

    #[test]
    fn resolves_exact_autocomplete_entry_with_shared_prefix() {
        let entries = vec![
            CachedJimakuEntry {
                id: 882,
                name: "Ore wo Suki nano wa Omae dake ka yo".to_string(),
                english_name: Some("ORESUKI: Are you the only one who loves me?".to_string()),
                japanese_name: Some("俺を好きなのはお前だけかよ".to_string()),
            },
            CachedJimakuEntry {
                id: 10420,
                name: "Ore wo Suki nano wa Omae dake ka yo: Oretachi no Game Set".to_string(),
                english_name: Some("ORESUKI: Are you the only one who loves me?: Our Playball / Our End Run / Our Game".to_string()),
                japanese_name: Some("俺を好きなのはお前だけかよ～俺たちのゲームセット～".to_string()),
            },
        ];

        let selected = "Ore wo Suki nano wa Omae dake ka yo: Oretachi no Game Set (俺を好きなのはお前だけかよ～俺たちのゲームセット～)";
        let resolved = resolve_entry(&entries, selected);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().id, 10420);
        assert_eq!(
            resolved.unwrap().name,
            "Ore wo Suki nano wa Omae dake ka yo: Oretachi no Game Set"
        );
    }

    #[test]
    fn resolves_fuzzy_query_fallback() {
        let entries = vec![
            CachedJimakuEntry {
                id: 882,
                name: "Ore wo Suki nano wa Omae dake ka yo".to_string(),
                english_name: Some("ORESUKI: Are you the only one who loves me?".to_string()),
                japanese_name: Some("俺を好きなのはお前だけかよ".to_string()),
            },
            CachedJimakuEntry {
                id: 10420,
                name: "Ore wo Suki nano wa Omae dake ka yo: Oretachi no Game Set".to_string(),
                english_name: Some("ORESUKI: Are you the only one who loves me?: Our Playball / Our End Run / Our Game".to_string()),
                japanese_name: Some("俺を好きなのはお前だけかよ～俺たちのゲームセット～".to_string()),
            },
        ];

        let resolved = resolve_entry(&entries, "Oretachi no Game Set");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().id, 10420);
    }
}
