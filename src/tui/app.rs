use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use fuzzy_matcher::skim::SkimMatcherV2;
use std::path::PathBuf;

use crate::provider::{SubtitleSearchResult, UnifiedAnimeEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStep {
    SelectVideo,
    SearchAnime,
    SelectSubtitle,
    Syncing,
    Completed,
}

pub enum AppAction {
    FetchSubtitles(UnifiedAnimeEntry, PathBuf),
    DownloadAndSync(SubtitleSearchResult, PathBuf),
    PlayMpv(PathBuf, PathBuf),
}

pub struct App {
    pub step: AppStep,
    pub videos: Vec<PathBuf>,
    pub video_filter: String,
    pub filtered_video_indices: Vec<usize>,
    pub selected_video_idx: usize,
    pub selected_video: Option<PathBuf>,
    pub anime_entries: Vec<UnifiedAnimeEntry>,
    pub anime_query: String,
    pub filtered_anime_indices: Vec<usize>,
    pub selected_anime_idx: usize,
    pub selected_anime: Option<UnifiedAnimeEntry>,
    pub subtitles: Vec<SubtitleSearchResult>,
    pub selected_sub_idx: usize,
    pub selected_sub: Option<SubtitleSearchResult>,
    pub status_message: String,
    pub is_loading: bool,
    pub spinner_tick: usize,
    pub sync_result_text: String,
    pub output_path: Option<PathBuf>,
    pub should_quit: bool,
}

impl App {
    pub fn new(videos: Vec<PathBuf>, entries: Vec<UnifiedAnimeEntry>) -> Self {
        let video_count = videos.len();
        let mut app = Self {
            step: AppStep::SelectVideo,
            videos,
            video_filter: String::new(),
            filtered_video_indices: (0..video_count).collect(),
            selected_video_idx: 0,
            selected_video: None,
            anime_entries: entries,
            anime_query: String::new(),
            filtered_anime_indices: Vec::new(),
            selected_anime_idx: 0,
            selected_anime: None,
            subtitles: Vec::new(),
            selected_sub_idx: 0,
            selected_sub: None,
            status_message: "Ready".to_string(),
            is_loading: false,
            spinner_tick: 0,
            sync_result_text: String::new(),
            output_path: None,
            should_quit: false,
        };
        app.filter_videos();
        app
    }

    pub fn filter_videos(&mut self) {
        let q = self.video_filter.trim().to_lowercase();
        if q.is_empty() {
            self.filtered_video_indices = (0..self.videos.len()).collect();
        } else {
            self.filtered_video_indices = self
                .videos
                .iter()
                .enumerate()
                .filter(|(_, p)| p.to_string_lossy().to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect();
        }
        self.selected_video_idx = 0;
    }

    pub fn filter_animes(&mut self) {
        let q = self.anime_query.trim();
        if q.is_empty() {
            self.filtered_anime_indices = (0..self.anime_entries.len().min(50)).collect();
        } else {
            let matcher = SkimMatcherV2::default().ignore_case();
            let mut scored = Vec::new();
            for (idx, entry) in self.anime_entries.iter().enumerate() {
                if let Some(score) = crate::search::score_entry(&matcher, entry, q) {
                    scored.push((score, idx));
                }
            }
            scored.sort_by_key(|b| std::cmp::Reverse(b.0));
            self.filtered_anime_indices = scored.into_iter().take(50).map(|(_, i)| i).collect();
        }
        self.selected_anime_idx = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return None;
        }

        match self.step {
            AppStep::SelectVideo => self.handle_video_key(key),
            AppStep::SearchAnime => self.handle_anime_key(key),
            AppStep::SelectSubtitle => self.handle_subtitle_key(key),
            AppStep::Syncing => None,
            AppStep::Completed => self.handle_completed_key(key),
        }
    }

    fn handle_video_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Up => {
                if self.selected_video_idx > 0 {
                    self.selected_video_idx -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_video_idx + 1 < self.filtered_video_indices.len() {
                    self.selected_video_idx += 1;
                }
            }
            KeyCode::Backspace => {
                self.video_filter.pop();
                self.filter_videos();
            }
            KeyCode::Char(c) => {
                self.video_filter.push(c);
                self.filter_videos();
            }
            KeyCode::Enter => {
                if let Some(&real_idx) = self.filtered_video_indices.get(self.selected_video_idx) {
                    let video = self.videos[real_idx].clone();
                    let meta = crate::parser::parse_anime_filename(&video);
                    self.selected_video = Some(video);
                    self.anime_query = meta.title;
                    self.filter_animes();
                    self.step = AppStep::SearchAnime;
                }
            }
            _ => {}
        }
        None
    }

    fn handle_anime_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        match key.code {
            KeyCode::Esc => {
                self.step = AppStep::SelectVideo;
            }
            KeyCode::Up => {
                if self.selected_anime_idx > 0 {
                    self.selected_anime_idx -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_anime_idx + 1 < self.filtered_anime_indices.len() {
                    self.selected_anime_idx += 1;
                }
            }
            KeyCode::Backspace => {
                self.anime_query.pop();
                self.filter_animes();
            }
            KeyCode::Char(c) => {
                self.anime_query.push(c);
                self.filter_animes();
            }
            KeyCode::Enter => {
                if let Some(&real_idx) = self.filtered_anime_indices.get(self.selected_anime_idx) {
                    let entry = self.anime_entries[real_idx].clone();
                    self.selected_anime = Some(entry.clone());
                    if let Some(ref vid) = self.selected_video {
                        self.is_loading = true;
                        self.status_message =
                            "Fetching subtitles from Jimaku & Kitsunekko...".to_string();
                        return Some(AppAction::FetchSubtitles(entry, vid.clone()));
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn handle_subtitle_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.step = AppStep::SearchAnime;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_sub_idx > 0 {
                    self.selected_sub_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_sub_idx + 1 < self.subtitles.len() {
                    self.selected_sub_idx += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(sub) = self.subtitles.get(self.selected_sub_idx) {
                    self.selected_sub = Some(sub.clone());
                    if let Some(ref vid) = self.selected_video {
                        self.is_loading = true;
                        self.step = AppStep::Syncing;
                        self.status_message = "Downloading and aligning subtitle...".to_string();
                        return Some(AppAction::DownloadAndSync(sub.clone(), vid.clone()));
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn handle_completed_key(&mut self, key: KeyEvent) -> Option<AppAction> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if let (Some(ref vid), Some(ref sub)) = (&self.selected_video, &self.output_path) {
                    return Some(AppAction::PlayMpv(vid.clone(), sub.clone()));
                }
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::PathBuf;

    fn mock_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_app_initialization_and_filtering() {
        let videos = vec![
            PathBuf::from("/anime/Heya Camp E01.mkv"),
            PathBuf::from("/anime/Yuru Camp E01.mkv"),
        ];
        let entries = vec![
            UnifiedAnimeEntry {
                name: "Heya Camp".into(),
                english_name: Some("Room Camp".into()),
                japanese_name: Some("へやキャン△".into()),
                jimaku_id: Some(100),
                kitsunekko_slug: Some("heya-camp".into()),
            },
            UnifiedAnimeEntry {
                name: "Yuru Camp".into(),
                english_name: Some("Laid-Back Camp".into()),
                japanese_name: Some("ゆるキャン△".into()),
                jimaku_id: Some(101),
                kitsunekko_slug: None,
            },
        ];

        let mut app = App::new(videos, entries);
        assert_eq!(app.filtered_video_indices.len(), 2);
        assert_eq!(app.selected_video_idx, 0);

        // Video navigation
        app.handle_key(mock_key(KeyCode::Down));
        assert_eq!(app.selected_video_idx, 1);
        app.handle_key(mock_key(KeyCode::Down));
        assert_eq!(app.selected_video_idx, 1);
        app.handle_key(mock_key(KeyCode::Up));
        assert_eq!(app.selected_video_idx, 0);

        // Filter video
        app.handle_key(mock_key(KeyCode::Char('Y')));
        app.handle_key(mock_key(KeyCode::Char('u')));
        assert_eq!(app.filtered_video_indices.len(), 1);
        assert_eq!(app.filtered_video_indices[0], 1);

        // Select video -> advance to SearchAnime
        let action = app.handle_key(mock_key(KeyCode::Enter));
        assert!(action.is_none());
        assert_eq!(app.step, AppStep::SearchAnime);
        assert_eq!(app.anime_query, "Yuru Camp");

        // Anime filter and selection
        let action = app.handle_key(mock_key(KeyCode::Enter));
        assert!(matches!(action, Some(AppAction::FetchSubtitles(..))));
    }

    #[test]
    fn test_completed_screen_actions() {
        let mut app = App::new(vec![], vec![]);
        app.step = AppStep::Completed;
        app.selected_video = Some(PathBuf::from("test.mkv"));
        app.output_path = Some(PathBuf::from("test.ja.srt"));

        let play_action = app.handle_key(mock_key(KeyCode::Char('p')));
        assert!(matches!(play_action, Some(AppAction::PlayMpv(..))));

        app.handle_key(mock_key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }
}
