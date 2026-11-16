pub mod app;
pub mod dialogs;
pub mod render;
pub mod theme;
pub mod views;

use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::provider::{SubtitleProvider, SubtitleSearchResult, UnifiedAnimeEntry};
use crate::syncer::SubtitleSyncer;
use app::{App, AppAction, AppStep};

pub struct TerminalGuard;

impl TerminalGuard {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;

        let default_panic = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), LeaveAlternateScreen);
            default_panic(info);
        }));

        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}

pub async fn run_tui(videos: Vec<PathBuf>, entries: Vec<UnifiedAnimeEntry>) -> Result<()> {
    let _guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(videos, entries);
    let provider = SubtitleProvider::new();
    let syncer = SubtitleSyncer::new();

    while !app.should_quit {
        terminal.draw(|f| render::render(f, &app))?;
        if event::poll(Duration::from_millis(60))? {
            if let Event::Key(key) = event::read()? {
                if let Some(action) = app.handle_key(key) {
                    dispatch_action(&mut terminal, &mut app, &provider, &syncer, action).await?;
                }
            }
        } else {
            app.spinner_tick = app.spinner_tick.wrapping_add(1);
        }
    }

    Ok(())
}

async fn dispatch_action<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    provider: &SubtitleProvider,
    syncer: &SubtitleSyncer,
    action: AppAction,
) -> Result<()> {
    match action {
        AppAction::FetchSubtitles(entry, video) => {
            terminal.draw(|f| render::render(f, app))?;
            handle_fetch_subtitles(app, provider, &entry, &video).await;
        }
        AppAction::DownloadAndSync(sub, video) => {
            handle_download_and_sync(terminal, app, provider, syncer, &sub, &video).await?;
        }
        AppAction::PlayMpv(video, subtitle) => {
            handle_play_mpv(terminal, &video, &subtitle)?;
        }
    }
    Ok(())
}

async fn handle_fetch_subtitles(
    app: &mut App,
    provider: &SubtitleProvider,
    entry: &UnifiedAnimeEntry,
    video: &Path,
) {
    let meta = crate::parser::parse_anime_filename(video);
    let path_str = video.to_string_lossy().to_lowercase();
    let title_str = meta.title.to_lowercase();
    let is_special = title_str.contains("sp")
        || title_str.contains("ova")
        || title_str.contains("special")
        || path_str.contains("sp")
        || path_str.contains("ova")
        || path_str.contains("special");

    let files = provider
        .fetch_entry_files(entry, meta.episode.as_deref(), is_special)
        .await
        .unwrap_or_default();

    app.subtitles = files;
    app.selected_sub_idx = 0;
    app.is_loading = false;
    app.step = AppStep::SelectSubtitle;
}

async fn handle_download_and_sync<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    provider: &SubtitleProvider,
    syncer: &SubtitleSyncer,
    sub: &SubtitleSearchResult,
    video: &Path,
) -> Result<()> {
    let parent_dir = video.parent().unwrap_or_else(|| Path::new("."));
    let video_stem = video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    app.status_message = "Downloading subtitle archive...".to_string();
    terminal.draw(|f| render::render(f, app))?;

    match provider.download_subtitle(sub, parent_dir).await {
        Ok(temp_sub) => {
            app.status_message =
                "Aligning audio with ALASS voice activity detection...".to_string();
            terminal.draw(|f| render::render(f, app))?;

            let ext = temp_sub
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("srt");
            let final_path = parent_dir.join(format!("{}.ja.{}", video_stem, ext));
            let sync_res = syncer.sync_subtitle(video, &temp_sub, &final_path);
            let _ = std::fs::remove_file(&temp_sub);

            finish_sync(app, sync_res, final_path);
        }
        Err(e) => {
            app.sync_result_text = format!("Download error: {}", e);
            app.is_loading = false;
            app.step = AppStep::Completed;
        }
    }
    Ok(())
}

fn finish_sync(app: &mut App, sync_res: Result<crate::syncer::SyncResult>, final_path: PathBuf) {
    match sync_res {
        Ok(res) => {
            let text = match res {
                crate::syncer::SyncResult::Success(shift) => {
                    format!("Aligned successfully ({})", shift)
                }
                crate::syncer::SyncResult::WarningLargeShift(shift) => {
                    format!("Warning: Large shift applied ({})", shift)
                }
                crate::syncer::SyncResult::DirectCopy => {
                    "Direct copy (no offset adjustment)".to_string()
                }
            };
            app.sync_result_text = text;
            app.output_path = Some(final_path);
        }
        Err(e) => {
            app.sync_result_text = format!("Sync error: {}", e);
        }
    }
    app.is_loading = false;
    app.step = AppStep::Completed;
}

fn handle_play_mpv<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    video: &Path,
    subtitle: &Path,
) -> Result<()> {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);

    let _ = Command::new("mpv")
        .arg(video)
        .arg(format!("--sub-file={}", subtitle.display()))
        .status();

    let _ = enable_raw_mode();
    let _ = execute!(stdout(), EnterAlternateScreen);
    terminal.clear()?;
    Ok(())
}
