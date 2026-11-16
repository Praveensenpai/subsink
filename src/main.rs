use anyhow::{bail, Result};
use inquire::{Select, Text};
use std::env;
use std::path::{Path, PathBuf};

mod parser;
mod provider;
mod scanner;
mod search;
mod syncer;
pub mod tui;
mod ui;

use provider::{SubtitleProvider, SubtitleSearchResult, UnifiedAnimeEntry};
use syncer::SubtitleSyncer;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match parse_cli_args(&args)? {
        CliAction::Help => {
            print_help();
            Ok(())
        }
        CliAction::Version => {
            println!("subsink {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        CliAction::Cli => run_cli_app().await,
        CliAction::Tui => run_tui_app().await,
    }
}

async fn run_tui_app() -> Result<()> {
    let videos_dir = dirs::video_dir().unwrap_or_else(|| PathBuf::from("./"));
    let home_dir = dirs::home_dir();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let cwd_is_home = home_dir
        .as_ref()
        .and_then(|h| h.canonicalize().ok())
        .is_some_and(|h| cwd.canonicalize().is_ok_and(|c| c == h));

    let mut videos = scanner::discover_video_files(&videos_dir, &cwd, home_dir.as_deref());
    scanner::sort_video_files(&mut videos, &cwd, cwd_is_home);

    let provider = SubtitleProvider::new();
    let entries = provider.ensure_cache().await?;

    tui::run_tui(videos, entries).await
}

async fn run_cli_app() -> Result<()> {
    inquire::set_global_render_config(ui::custom_render_config());
    ui::print_banner();

    let video_path = select_cli_video()?;
    let parent_dir = video_path.parent().unwrap_or_else(|| Path::new("."));
    let video_stem = video_path
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "video".into());

    let provider = SubtitleProvider::new();
    let Some(entry) = select_cli_anime(&provider, &video_path).await? else {
        return Ok(());
    };

    let Some(selected_sub) = select_cli_subtitle(&provider, &entry, &video_path).await? else {
        return Ok(());
    };

    ui::print_step(4, 4, "Download & Subtitle Timing");
    let download_spinner = ui::create_spinner("Downloading subtitle file...");
    let temp_sub = provider
        .download_subtitle(&selected_sub, parent_dir)
        .await?;
    download_spinner.finish_and_clear();
    ui::print_success("Subtitle downloaded successfully.");

    let sub_ext = temp_sub
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("srt");
    let target_sub_filename = format!("{}.ja.{}", video_stem, sub_ext);
    let target_sub_path = parent_dir.join(&target_sub_filename);

    let syncer = SubtitleSyncer::new();
    apply_cli_timing(&syncer, &temp_sub, &target_sub_path, &video_path)?;

    if temp_sub != target_sub_path {
        let _ = std::fs::remove_file(&temp_sub);
    }

    ui::print_success(&format!("Successfully saved: {}", target_sub_filename));
    ui::print_info(&format!("Saved to: {}", target_sub_path.display()));
    println!();
    Ok(())
}

fn select_cli_video() -> Result<PathBuf> {
    ui::print_step(1, 4, "Select Raw Anime Video File");
    let video_path = scanner::select_video_file()?;
    let file_display_name = video_path
        .file_name()
        .map(|f| f.to_string_lossy())
        .unwrap_or_else(|| "video".into());
    ui::print_success(&format!("Selected File: {}", file_display_name));
    Ok(video_path)
}

async fn select_cli_anime(
    provider: &SubtitleProvider,
    video_path: &Path,
) -> Result<Option<UnifiedAnimeEntry>> {
    ui::print_step(2, 4, "Search Subtitle Database (Live Fuzzy Match)");
    let spinner = ui::create_spinner("Loading subtitle database indices (Jimaku & Kitsunekko)...");
    let entries = provider.ensure_cache().await?;
    spinner.finish_and_clear();
    ui::print_success(&format!(
        "Loaded {} anime titles into unified local index",
        entries.len()
    ));

    let meta = parser::parse_anime_filename(video_path);
    let default_query = meta.title.clone();

    let entry = search::select_anime_entry(entries, &default_query)?;
    if let Some(ref e) = entry {
        ui::print_success(&format!("Matched Anime: {}", e.name));
    } else {
        ui::print_warning("No matching anime entry selected.");
    }
    Ok(entry)
}

async fn select_cli_subtitle(
    provider: &SubtitleProvider,
    entry: &UnifiedAnimeEntry,
    video_path: &Path,
) -> Result<Option<SubtitleSearchResult>> {
    ui::print_step(3, 4, "Fetching Available Subtitle Files (30 Days Cache)");
    let fetch_spinner = ui::create_spinner("Loading episode subtitle files...");
    let meta = parser::parse_anime_filename(video_path);
    let path_str = video_path.to_string_lossy().to_lowercase();
    let title_str = meta.title.to_lowercase();
    let is_special = title_str.contains("sp")
        || title_str.contains("ova")
        || title_str.contains("special")
        || path_str.contains("sp")
        || path_str.contains("ova")
        || path_str.contains("special");

    let results = provider
        .fetch_entry_files(entry, meta.episode.as_deref(), is_special)
        .await?;
    fetch_spinner.finish_and_clear();

    if results.is_empty() {
        ui::print_warning("No subtitle files found for this anime entry.");
        return Ok(None);
    }

    let display_options: Vec<String> = results
        .iter()
        .map(|r| format!("[{}] [{}] {}", r.language, r.provider, r.title))
        .collect();

    let selected_display =
        Select::new("Choose subtitle file to download & sync:", display_options).prompt()?;
    let choice_idx = results
        .iter()
        .position(|r| format!("[{}] [{}] {}", r.language, r.provider, r.title) == selected_display)
        .unwrap_or(0);

    Ok(Some(results[choice_idx].clone()))
}

fn apply_cli_timing(
    syncer: &SubtitleSyncer,
    temp_sub: &Path,
    target_sub: &Path,
    video: &Path,
) -> Result<()> {
    let timing_options = vec![
        "Auto-align speech timing with ALASS (Recommended)",
        "Shift timing by custom offset (e.g. +500ms, -1.2s)",
        "Direct save (no timing adjustments)",
    ];

    let choice = Select::new(
        "Select timing adjustment for downloaded subtitle:",
        timing_options,
    )
    .prompt()?;

    if choice.starts_with("Auto-align") {
        let align_spinner = ui::create_spinner("Aligning audio voice activity using ALASS...");
        let sync_result = syncer.sync_subtitle(video, temp_sub, target_sub);
        align_spinner.finish_and_clear();

        match sync_result {
            Ok(syncer::SyncResult::Success(shift)) => {
                ui::print_success(&format!("Successfully aligned subtitle speech ({})", shift));
            }
            Ok(syncer::SyncResult::WarningLargeShift(shift)) => {
                ui::print_warning(&format!(
                    "Large shift applied: {}. Verify synchronization in media player.",
                    shift
                ));
            }
            Ok(syncer::SyncResult::DirectCopy) => {
                ui::print_info("Subtitle copied directly without timing modifications.");
            }
            Err(e) => {
                ui::print_warning(&format!(
                    "Auto-alignment failed: {}. Falling back to direct save.",
                    e
                ));
                std::fs::copy(temp_sub, target_sub)?;
            }
        }
    } else if choice.starts_with("Shift timing") {
        let offset_str = Text::new("Enter millisecond or second offset (e.g. +500ms, -1.2s):")
            .with_default("0")
            .prompt()?;
        let offset_ms = syncer::parse_offset_string(&offset_str)?;
        syncer::apply_manual_offset(temp_sub, target_sub, offset_ms)?;
        ui::print_success(&format!("Applied manual offset: {:+}ms", offset_ms));
    } else {
        std::fs::copy(temp_sub, target_sub)?;
        ui::print_info("Original subtitle saved directly without timing modifications.");
    }

    Ok(())
}

enum CliAction {
    Tui,
    Cli,
    Help,
    Version,
}

fn parse_cli_args(args: &[String]) -> Result<CliAction> {
    match args {
        [] => Ok(CliAction::Tui),
        [flag] if matches!(flag.as_str(), "--cli") => Ok(CliAction::Cli),
        [flag] if matches!(flag.as_str(), "-h" | "--help") => Ok(CliAction::Help),
        [flag] if matches!(flag.as_str(), "-v" | "-V" | "--version") => Ok(CliAction::Version),
        [argument, ..] => bail!("Unknown argument: {argument}\n\nRun `subsink --help` for usage."),
    }
}

fn print_help() {
    println!(
        "subsink {}\n\nUsage: subsink [OPTION]\n\nOptions:\n      --cli        Run in legacy CLI mode instead of interactive TUI\n  -h, --help       Show this help message\n  -v, -V, --version  Show the version",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::{parse_cli_args, CliAction};

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
        assert!(matches!(
            parse_cli_args(&["--cli".to_string()]).unwrap(),
            CliAction::Cli
        ));
        assert!(matches!(parse_cli_args(&[]).unwrap(), CliAction::Tui));
        assert!(parse_cli_args(&["--unknown".to_string()]).is_err());
    }
}
