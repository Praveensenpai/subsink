mod parser;
mod provider;
mod syncer;
mod ui;

use anyhow::{bail, Result};
use inquire::Autocomplete;
use inquire::CustomUserError;
use inquire::{Select, Text};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
    let video_path = select_video_file()?;
    ui::print_success(&format!("Selected File: {}", video_path.file_name().unwrap().to_string_lossy()));

    let parent_dir = video_path.parent().unwrap_or_else(|| Path::new("."));
    let video_stem = video_path.file_stem().unwrap().to_string_lossy();

    // STEP 2: Load Jimaku Index & Live Search
    ui::print_step(2, 4, "Search Subtitle Database (Live Fuzzy Match)");
    let spinner = ui::create_spinner("Loading Jimaku subtitle index...");
    let provider = provider::SubtitleProvider::new();
    let entries = provider.ensure_jimaku_cache().await?;
    spinner.finish_and_clear();
    ui::print_success(&format!("Loaded {} anime titles into local index", entries.len()));

    let meta = parser::parse_anime_filename(&video_path);
    let default_query = meta.title.clone();

    let autocompleter = JimakuAutocompleter {
        entries: entries.clone(),
    };

    let selected_anime_str = Text::new("Search anime title (type to filter live):")
        .with_default(&default_query)
        .with_autocomplete(autocompleter)
        .prompt()?;

    let matched_entry = entries.iter().find(|e| {
        selected_anime_str.starts_with(&e.name) || selected_anime_str.contains(&e.name)
    });

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
    let results = provider.fetch_entry_files(entry, meta.episode.as_deref()).await?;
    fetch_spinner.finish_and_clear();

    if results.is_empty() {
        ui::print_warning("No subtitle files found for this anime entry.");
        return Ok(());
    }

    let display_options: Vec<String> = results
        .iter()
        .map(|r| format!("[{}] {}", r.language, r.title))
        .collect();

    let selected_display = Select::new("Choose subtitle file to download & sync:", display_options).prompt()?;
    let choice_idx = results
        .iter()
        .position(|r| format!("[{}] {}", r.language, r.title) == selected_display)
        .unwrap_or(0);

    let selected_sub = &results[choice_idx];

    // STEP 4: Download, Auto-Sync & Overwrite
    ui::print_step(4, 4, "Downloading & Aligning Subtitles");
    let download_spinner = ui::create_spinner("Downloading subtitle file...");
    let temp_sub = provider.download_subtitle(selected_sub, parent_dir).await?;
    download_spinner.finish_and_clear();

    let sync_spinner = ui::create_spinner("Aligning subtitle timings with audio via ALASS...");
    
    let sub_ext = temp_sub.extension().and_then(|s| s.to_str()).unwrap_or("srt");
    let target_sub_filename = format!("{}.ja.{}", video_stem, sub_ext);
    let target_sub_path = parent_dir.join(&target_sub_filename);

    let syncer = syncer::SubtitleSyncer::new();
    let sync_res = syncer.sync_subtitle(&video_path, &temp_sub, &target_sub_path)?;
    
    if temp_sub != target_sub_path {
        let _ = std::fs::remove_file(&temp_sub);
    }
    
    sync_spinner.finish_and_clear();

    if let Some(shift_summary) = sync_res {
        ui::print_success(&format!("ALASS Alignment: {}", shift_summary));
    } else {
        ui::print_info("Subtitle file saved directly alongside raw video.");
    }

    ui::print_success(&format!("Successfully synced and overwritten: {}", target_sub_filename));
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

fn select_video_file() -> Result<PathBuf> {
    let videos_dir = dirs::video_dir().unwrap_or_else(|| PathBuf::from("./"));
    
    let mut files = Vec::new();
    for entry in WalkDir::new(&videos_dir).max_depth(3).into_iter().flatten() {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                if matches!(ext.to_lowercase().as_str(), "mkv" | "mp4" | "avi" | "webm") {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }

    if files.is_empty() {
        for entry in WalkDir::new("./").max_depth(2).into_iter().flatten() {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                    if matches!(ext.to_lowercase().as_str(), "mkv" | "mp4" | "avi" | "webm") {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    }

    if files.is_empty() {
        let custom_path_str = Text::new("Enter video file path:").prompt()?;
        return Ok(PathBuf::from(custom_path_str));
    }

    files.sort_by(|left, right| natural_path_cmp(left, right));

    let file_display_names: Vec<String> = files
        .iter()
        .map(|p| {
            let file_name = p.file_name().unwrap().to_string_lossy();
            let parent = p.parent().and_then(|par| par.file_name()).map(|f| f.to_string_lossy()).unwrap_or_default();
            format!("{}/{}", parent, file_name)
        })
        .collect();

    let choice = Select::new("Select video file from ~/Videos:", file_display_names).prompt()?;
    let selected_index = files.iter().position(|p| {
        let name = format!(
            "{}/{}",
            p.parent().and_then(|par| par.file_name()).map(|f| f.to_string_lossy()).unwrap_or_default(),
            p.file_name().unwrap().to_string_lossy()
        );
        name == choice
    }).unwrap_or(0);

    Ok(files[selected_index].clone())
}

/// Compares paths naturally, so numbered episodes sort as 2, 3, 10 rather
/// than 10, 2, 3.
fn natural_path_cmp(left: &Path, right: &Path) -> Ordering {
    natural_cmp(&left.to_string_lossy(), &right.to_string_lossy())
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let (mut left_index, mut right_index) = (0, 0);

    while left_index < left_bytes.len() && right_index < right_bytes.len() {
        let left_is_digit = left_bytes[left_index].is_ascii_digit();
        let right_is_digit = right_bytes[right_index].is_ascii_digit();

        if left_is_digit && right_is_digit {
            let left_start = left_index;
            let right_start = right_index;

            while left_index < left_bytes.len() && left_bytes[left_index].is_ascii_digit() {
                left_index += 1;
            }
            while right_index < right_bytes.len() && right_bytes[right_index].is_ascii_digit() {
                right_index += 1;
            }

            let left_number = &left_bytes[left_start..left_index];
            let right_number = &right_bytes[right_start..right_index];
            let left_significant = left_number
                .iter()
                .position(|digit| *digit != b'0')
                .unwrap_or(left_number.len() - 1);
            let right_significant = right_number
                .iter()
                .position(|digit| *digit != b'0')
                .unwrap_or(right_number.len() - 1);
            let left_number = &left_number[left_significant..];
            let right_number = &right_number[right_significant..];

            match left_number.len().cmp(&right_number.len()) {
                Ordering::Equal => match left_number.cmp(right_number) {
                    Ordering::Equal => {}
                    order => return order,
                },
                order => return order,
            }
        } else {
            let left_byte = left_bytes[left_index].to_ascii_lowercase();
            let right_byte = right_bytes[right_index].to_ascii_lowercase();

            match left_byte.cmp(&right_byte) {
                Ordering::Equal => {
                    left_index += 1;
                    right_index += 1;
                }
                order => return order,
            }
        }
    }

    left_bytes.len().cmp(&right_bytes.len())
}

#[cfg(test)]
mod tests {
    use super::{natural_cmp, parse_cli_args, CliAction};

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
    fn sorts_episode_numbers_naturally() {
        let mut episodes = vec!["episode 10.mkv", "episode 02.mkv", "episode 3.mkv"];

        episodes.sort_by(|left, right| natural_cmp(left, right));

        assert_eq!(episodes, vec!["episode 02.mkv", "episode 3.mkv", "episode 10.mkv"]);
    }
}
