# CODEBASE.md: subsink Semantic Digest

> **Notice**: This file is an AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
CLI Entry (src/main.rs)
  │
  ├──> Scanner (src/scanner.rs) ──> WalkDir ~/Videos & CWD (depth 8) ──> Deduplicated Raw Videos
  ├──> Parser (src/parser.rs) ──> AnimeMetadata { title, episode }
  ├──> Provider (src/provider.rs) ──> Jimaku API / Local Cache ──> Download Subtitle (.srt / .ass)
  ├──> Syncer (src/syncer.rs) ──> ALASS Audio Voice Alignment / Manual Shift
  └──> UI (src/ui.rs) ──> Inquire Prompts, Spinners & Render Config
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language & Edition**: Rust 2021 edition (1.80+)
- **Architectural Paradigm**: Modular CLI with clear role-based separation (`scanner`, `parser`, `provider`, `syncer`, `ui`)
- **Hard Constraints**: <400 lines/file (<300 soft), <60 lines/fn, zero production `unwrap()`/`expect()`, 0 warnings (`cargo clippy -- -D warnings`).
- **Target Distribution**: Linux `x86_64` standalone binary (`subsink-linux-x86_64.tar.gz`) via GitHub Releases and local binary install (`~/.local/bin/subsink`).

## 3. Module & Interface Skeleton

### `src/main.rs` (Role: cli/orchestrator, Lines: 334)
- **Responsibility**: Coordinates 4-step wizard workflow (video selection, database search, subtitle download, audio alignment).
- **Imports**: `crate::parser`, `crate::provider`, `crate::scanner`, `crate::syncer`, `crate::ui`, `inquire::*`, `anyhow::*`
- **Types & Enums**:
  ```rust
  struct JimakuAutocompleter { entries: Vec<provider::CachedJimakuEntry> }
  enum CliAction { Run, Help, Version }
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn main() -> Result<()>
  fn parse_cli_args(args: &[String]) -> Result<CliAction>
  fn print_help()
  fn resolve_entry<'a>(entries: &'a [provider::CachedJimakuEntry], selected: &str) -> Option<&'a provider::CachedJimakuEntry>
  ```
- **Consumers**: Process entrypoint (`bin "subsink"`).
- **Side Effects / I/O**: Terminal I/O, subprocess invocations, file creation for subtitles.

### `src/scanner.rs` (Role: infra/io, Lines: 300)
- **Responsibility**: Recursively scans `~/Videos` and current working directory up to depth 8 for anime video files, deduplicates canonical paths, formats relative display paths, and sorts episode numbers naturally.
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
  pub fn select_video_file() -> Result<PathBuf>
  pub fn natural_path_cmp(left: &Path, right: &Path) -> Ordering
  pub fn natural_cmp(left: &str, right: &str) -> Ordering
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: Filesystem directory walking (`dirs::video_dir()` and `std::env::current_dir()`), terminal selection prompt.

### `src/parser.rs` (Role: domain/logic, Lines: 84)
- **Responsibility**: Cleans raw anime file stem names (removing release group tags, video codecs, bracket tags) and extracts episode numbers using static `LazyLock` regexes.
- **Imports**: `regex::Regex`, `std::path::Path`, `std::sync::LazyLock`
- **Types & Enums**:
  ```rust
  pub struct AnimeMetadata { pub title: String, pub episode: Option<String> }
  ```
- **Public Functions & Signatures**:
  ```rust
  pub fn parse_anime_filename(file_path: &Path) -> AnimeMetadata
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: Pure in-memory parsing.

### `src/provider.rs` (Role: infra/api, Lines: 385)
- **Responsibility**: Fetches Jimaku subtitle metadata, caches anime index locally (~/.cache/subsink) with 30-day TTL, executes fuzzy matching, and downloads/extracts subtitles.
- **Imports**: `reqwest::Client`, `serde::{Deserialize, Serialize}`, `fuzzy_matcher::*`, `tempfile::*`, `zip::*`, `anyhow::Result`, `std::sync::LazyLock`
- **Types & Enums**:
  ```rust
  pub struct SubtitleProvider { client: Client, cache_dir: PathBuf }
  pub struct CachedJimakuEntry { pub id: u64, pub name: String, pub english_name: Option<String>, pub japanese_name: Option<String> }
  pub struct SubtitleSearchResult { pub provider: String, pub title: String, pub episode: Option<String>, pub language: String, pub download_url: String, pub file_format: String }
  ```
- **Public Functions & Signatures**:
  ```rust
  impl SubtitleProvider {
      pub fn new() -> Self
      pub async fn ensure_jimaku_cache(&self) -> Result<Vec<CachedJimakuEntry>>
      pub fn fuzzy_filter_entries(entries: &[CachedJimakuEntry], query: &str) -> Vec<CachedJimakuEntry>
      pub async fn fetch_entry_files(&self, entry: &CachedJimakuEntry, target_ep: Option<&str>) -> Result<Vec<SubtitleSearchResult>>
      pub async fn download_subtitle(&self, result: &SubtitleSearchResult, dest_dir: &Path) -> Result<PathBuf>
  }
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: HTTP requests to `jimaku.cc`, disk cache read/write under `~/.cache/subsink`.

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
- **Public Functions & Signatures**:
  ```rust
  pub fn print_banner()
  pub fn custom_render_config() -> RenderConfig<'static>
  pub fn create_spinner(msg: &'static str) -> ProgressBar
  pub fn print_step(step: usize, total: usize, title: &str)
  pub fn print_success(msg: &str)
  pub fn print_info(msg: &str)
  pub fn print_warning(msg: &str)
  ```
- **Consumers**: `src/main.rs`
- **Side Effects / I/O**: ANSI terminal styling and stdout writing.

## 4. Execution Lifecycle Trace
1. **Startup**: `subsink` parses CLI flags (`-h`, `--help`, `-v`, `--version`).
2. **Step 1 (Scan & Select)**: `scanner::select_video_file()` discovers anime videos across `~/Videos` and current working directory up to depth 8 with deduplication, formats human-readable relative paths, and prompts user via interactive selection.
3. **Step 2 (Jimaku Search)**: `parser::parse_anime_filename()` extracts anime title & episode; `provider::SubtitleProvider` loads cached titles and provides live interactive fuzzy autocompletion.
4. **Step 3 (Subtitle Fetch)**: Fetches and displays available `.ass`/`.srt` releases from Jimaku (cached for 30 days), filtered for the target episode.
5. **Step 4 (Download & Sync)**: Downloads selected subtitle archive, then aligns timings using ALASS audio-voice recognition or applies user-specified millisecond offset.
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
- **2026-09-20 (v0.1.5)**:
  - Added `src/scanner.rs` with deep recursive search (`max_depth: 8`), expanded video extensions (`mkv`, `mp4`, `avi`, `webm`, `m4v`, `mov`, `ts`, `flv`, `wmv`), and relative path display formatting.
  - Added concurrent current working directory (`cwd`) traversal with canonical deduplication against `~/Videos`.
  - Refactored `src/main.rs` and `src/provider.rs` to keep all files under 400 lines and eliminate production `unwrap()` calls.
  - Cleaned dead code in `src/parser.rs` and converted regexes to static `LazyLock`.
  - Fixed clippy patterns across `provider.rs`, `syncer.rs`, and `ui.rs`.
  - Added full test coverage for deep nested folder scanning and relative path formatting.
