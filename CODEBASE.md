# CODEBASE.md: subsink Semantic Digest

> **Notice**: This file is an AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
CLI Entry (src/main.rs)
  │
  ├──> Scanner (src/scanner.rs) ──> WalkDir ~/Videos & CWD (depth 8) ──> Deduplicated Raw Videos
  ├──> Parser (src/parser.rs) ──> AnimeMetadata { title, episode }
  ├──> Search (src/search.rs) ──> Inquire Live Fuzzy Select & Skim Scorer ──> Selected UnifiedAnimeEntry
  ├──> Provider (src/provider.rs)
  │      ├──> JimakuProvider (src/provider/jimaku.rs) ──> Jimaku.cc Scraper / Local Cache
  │      └──> KitsunekkoProvider (src/provider/kitsunekko.rs) ──> Kitsunekko Scraper / Local Cache
  ├──> Syncer (src/syncer.rs) ──> ALASS Audio Voice Alignment / Manual Shift
  └──> UI (src/ui.rs) ──> Inquire Prompts, Spinners & Render Config
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language & Edition**: Rust 2021 edition (1.80+)
- **Architectural Paradigm**: Modular CLI with clear role-based separation (`scanner`, `parser`, `search`, `provider`, `syncer`, `ui`)
- **Hard Constraints**: <400 lines/file (<300 soft), <60 lines/fn, zero production `unwrap()`/`expect()`, 0 warnings (`cargo clippy -- -D warnings`).
- **Target Distribution**: Linux `x86_64` standalone binary (`subsink-linux-x86_64.tar.gz`) via GitHub Releases and local binary install (`~/.local/bin/subsink`).

## 3. Module & Interface Skeleton

### `src/main.rs` (Role: cli/orchestrator, Lines: 209)
- **Responsibility**: Coordinates 4-step wizard workflow (video selection, dual-database search, subtitle download, audio alignment).
- **Imports**: `crate::parser`, `crate::provider`, `crate::scanner`, `crate::search`, `crate::syncer`, `crate::ui`, `inquire::{Select, Text}`, `anyhow::*`
- **Types & Enums**:
  ```rust
  enum CliAction { Run, Help, Version }
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn main() -> Result<()>
  fn parse_cli_args(args: &[String]) -> Result<CliAction>
  fn print_help()
  ```
- **Consumers**: Process entrypoint (`bin "subsink"`).
- **Side Effects / I/O**: Terminal I/O, subprocess invocations, file creation for subtitles.

### `src/search.rs` (Role: domain/search, Lines: 196)
- **Responsibility**: Real-time fuzzy anime title search and selection via `inquire::Select`, multi-token matching, case-insensitive scoring (`SkimMatcherV2`), and fallback ranking across Romaji, English, and Japanese names.
- **Imports**: `fuzzy_matcher::skim::SkimMatcherV2`, `fuzzy_matcher::FuzzyMatcher`, `inquire::Select`, `anyhow::Result`, `crate::provider::UnifiedAnimeEntry`
- **Public Functions & Signatures**:
  ```rust
  pub fn score_entry(matcher: &SkimMatcherV2, entry: &UnifiedAnimeEntry, query: &str) -> Option<i64>
  pub fn fuzzy_filter_entries(entries: &[UnifiedAnimeEntry], query: &str) -> Vec<UnifiedAnimeEntry>
  pub fn select_anime_entry(entries: Vec<UnifiedAnimeEntry>, default_query: &str) -> Result<Option<UnifiedAnimeEntry>>
  ```
- **Consumers**: `src/main.rs`, `src/provider.rs`
- **Side Effects / I/O**: Interactive terminal selection prompt with live filtering.

### `src/scanner.rs` (Role: infra/io, Lines: 349)
- **Responsibility**: Recursively scans `~/Videos` and current working directory up to depth 8 for anime video files, deduplicates canonical paths, formats relative display paths, prioritizes files in CWD, and sorts episode numbers naturally.
- **Imports**: `walkdir::WalkDir`, `inquire::{Select, Text}`, `anyhow::Result`, `std::cmp::Ordering`, `std::collections::HashSet`, `std::path::{Path, PathBuf}`
- **Constants**:
  ```rust
  pub const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "webm", "m4v", "mov", "ts", "flv", "wmv"];
  pub const SCAN_MAX_DEPTH: usize = 8;
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn is_video_extension(ext: &str) -> bool
  pub fn is_video_file(path: &Path) -> bool
  pub fn scan_video_files(base_dir: &Path, max_depth: usize) -> Vec<PathBuf>
  pub fn format_video_display(path: &Path, videos_dir: &Path, cwd: &Path, cwd_is_home: bool) -> String
  pub fn discover_video_files(videos_dir: &Path, cwd: &Path, home_dir: Option<&Path>) -> Vec<PathBuf>
  pub fn sort_video_files(files: &mut [PathBuf], cwd: &Path, cwd_is_home: bool)
  pub fn select_video_file() -> Result<PathBuf>
  pub fn natural_path_cmp(left: &Path, right: &Path) -> Ordering
  pub fn natural_cmp(left: &str, right: &str) -> Ordering
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: Filesystem directory walking (`dirs::video_dir()` and `std::env::current_dir()`), terminal selection prompt.

### `src/parser.rs` (Role: domain/logic, Lines: 287)
- **Responsibility**: Robust extraction of anime title, season (`Option<u32>`), and episode (`Option<String>`) across scene formats (`S02E01`), standard end-of-stem numbers, end markers (`END`/`FINAL`), bracketed rips, specials with subtitles, and parent directory hierarchy fallback.
- **Imports**: `regex::Regex`, `std::path::Path`, `std::sync::LazyLock`
- **Types & Enums**:
  ```rust
  pub struct AnimeMetadata { pub title: String, pub season: Option<u32>, pub episode: Option<String> }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn parse_anime_filename(file_path: &Path) -> AnimeMetadata
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: Pure in-memory parsing.

### `src/provider.rs` (Role: infra/api, Lines: 365)
- **Responsibility**: Coordinates multi-source subtitle retrieval across Jimaku.cc and Kitsunekko, maintains unified 30-day cache, resolves cross-provider titles via normalized alphanumeric hashing, and extracts subtitle files (.srt, .ass, .zip).
- **Submodules**: `pub mod jimaku; pub mod kitsunekko;`
- **Imports**: `reqwest::Client`, `serde::{Deserialize, Serialize}`, `tempfile::*`, `zip::*`, `anyhow::Result`, `std::fmt`
- **Types & Enums**:
  ```rust
  pub struct SubtitleProvider { client: reqwest::Client, jimaku: jimaku::JimakuProvider, kitsunekko: kitsunekko::KitsunekkoProvider }
  pub struct UnifiedAnimeEntry { pub name: String, pub english_name: Option<String>, pub japanese_name: Option<String>, pub jimaku_id: Option<u64>, pub kitsunekko_slug: Option<String> }
  pub struct SubtitleSearchResult { pub provider: String, pub title: String, pub episode: Option<String>, pub language: String, pub download_url: String, pub file_format: String }
  ```
- **Public Functions & Signatures**:
  ```rust
  impl SubtitleProvider {
      pub fn new() -> Self
      pub async fn ensure_cache(&self) -> Result<Vec<UnifiedAnimeEntry>>
      pub async fn fetch_entry_files(&self, entry: &UnifiedAnimeEntry, target_ep: Option<&str>, is_special: bool) -> Result<Vec<SubtitleSearchResult>>
      pub fn filter_subtitles(files: Vec<SubtitleSearchResult>, target_ep: Option<&str>, is_special: bool) -> Vec<SubtitleSearchResult>
      pub async fn download_subtitle(&self, result: &SubtitleSearchResult, dest_dir: &Path) -> Result<PathBuf>
  }
  pub fn normalize_title(s: &str) -> String
  ```
- **Consumers**: `src/main.rs`, `src/search.rs`
- **Side Effects / I/O**: Disk cache read/write under `~/.cache/subsink`.

### `src/provider/jimaku.rs` (Role: infra/api/jimaku, Lines: 178)
- **Responsibility**: Jimaku.cc index caching (`jimaku_index.json`), HTML parsing, and per-entry file scraping (`entry_{id}_files.json`).
- **Types & Enums**:
  ```rust
  pub struct CachedJimakuEntry { pub id: u64, pub name: String, pub english_name: Option<String>, pub japanese_name: Option<String> }
  pub struct JimakuProvider { client: reqwest::Client, cache_dir: PathBuf }
  ```
- **Public Functions & Signatures**:
  ```rust
  impl JimakuProvider {
      pub fn new(client: reqwest::Client, cache_dir: PathBuf) -> Self
      pub async fn ensure_index(&self) -> Result<Vec<CachedJimakuEntry>>
      pub async fn fetch_files(&self, id: u64, anime_name: &str) -> Result<Vec<SubtitleSearchResult>>
  }
  ```

### `src/provider/kitsunekko.rs` (Role: infra/api/kitsunekko, Lines: 194)
- **Responsibility**: Kitsunekko directory list caching (`kitsunekko_index.json`), URL-encoded path parsing, and folder file scraping (`kitsunekko_{slug}_files.json`).
- **Types & Enums**:
  ```rust
  pub struct CachedKitsunekkoEntry { pub slug: String, pub name: String }
  pub struct KitsunekkoProvider { client: reqwest::Client, cache_dir: PathBuf }
  ```
- **Public Functions & Signatures**:
  ```rust
  impl KitsunekkoProvider {
      pub fn new(client: reqwest::Client, cache_dir: PathBuf) -> Self
      pub async fn ensure_index(&self) -> Result<Vec<CachedKitsunekkoEntry>>
      pub async fn fetch_files(&self, slug: &str, anime_name: &str) -> Result<Vec<SubtitleSearchResult>>
  }
  ```

### `src/syncer.rs` (Role: domain/infra, Lines: 329)
- **Responsibility**: Integrates with ALASS audio synchronization tool or executes manual millisecond subtitle offset shifting for SRT and ASS subtitle formats.
- **Imports**: `regex::Regex`, `std::process::Command`, `anyhow::{Context, Result}`, `std::path::{Path, PathBuf}`
- **Types & Enums**:
  ```rust
  pub enum SyncResult { Success(String), WarningLargeShift(String), DirectCopy }
  pub struct SubtitleSyncer;
  ```
- **Public Functions & Signatures**:
  ```rust
  impl SubtitleSyncer {
      pub fn new() -> Self
      pub fn sync_subtitle(&self, video_path: &Path, raw_sub_path: &Path, output_sub_path: &Path) -> Result<SyncResult>
  }
  pub fn parse_alass_shift(output: &str) -> String
  pub fn evaluate_alass_output(output: &str) -> SyncResult
  pub fn parse_alass_offset_seconds(output: &str) -> Option<f64>
  pub fn parse_offset_string(input: &str) -> Result<i64>
  pub fn apply_manual_offset(input_path: &Path, output_path: &Path, offset_ms: i64) -> Result<()>
  pub fn shift_subtitle_text(content: &str, offset_ms: i64) -> String
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: Executes `alass` / `alass-cli` subprocesses; reads and writes subtitle files on disk.

### `src/ui.rs` (Role: tui/presentation, Lines: 78)
- **Responsibility**: Custom console themes, banners, step indicators, render configurations, and progress spinners.
- **Imports**: `console::style`, `indicatif::{ProgressBar, ProgressStyle}`, `inquire::ui::*`

## 4. Execution Lifecycle Trace
1. **Startup**: `subsink` parses CLI flags (`-h`, `--help`, `-v`, `--version`).
2. **Step 1 (Scan & Select)**: `scanner::select_video_file()` discovers anime videos across `~/Videos` and current working directory up to depth 8 with deduplication.
3. **Step 2 (Dual Index Live Search)**: Loads & merges Jimaku.cc (~4,600 titles) and Kitsunekko (~2,600 titles) indices with 30-day disk caching; `search::select_anime_entry()` presents an interactive live fuzzy picker pre-filled with parsed title.
4. **Step 3 (Dual-Source Subtitle Fetch)**: Fetches available `.ass`/`.srt`/`.zip` releases concurrently from both Jimaku and Kitsunekko, tagging provider origins and filtering for target episode or specials.
5. **Step 4 (Download & Sync)**: Downloads selected subtitle archive, extracts inner subtitles if zipped, and aligns timings using ALASS audio-voice recognition.
6. **Output**: Writes final subtitle as `<video_stem>.ja.<ext>` alongside the video file.

## 5. Verification Commands
```bash
# Build
cargo build --release

# Test
cargo test --all-targets

# Lint & Format
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## 6. Recent Iteration Changes
- **2026-11-17 (v0.1.8 - Kitsunekko Dual-Provider Integration)**:
  - Added native **Kitsunekko** provider support (`src/provider/kitsunekko.rs`) alongside Jimaku.cc (`src/provider/jimaku.rs`).
  - Implemented `UnifiedAnimeEntry` with cross-provider linking via `normalize_title` alphanumeric hashing.
  - Added concurrent dual-provider fetching (`tokio::join!`) with 30-day disk caching for both index and episode listings.
  - Added special/OVA preservation in `filter_subtitles` to ensure OVA and SP episodes (e.g. `Heya Camp - SP`) are discoverable.
  - Added integration and unit tests verifying cross-provider linking and live multi-source retrieval.
