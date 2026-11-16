# CODEBASE.md: subsink Semantic Digest

> **Notice**: This file is an AI-optimized semantic index. Do not write narrative prose. Keep token density high.

## 1. System Topology & Data Flow
```text
Interactive TUI / CLI Entry (src/main.rs)
  │
  ├──> TUI Engine (src/tui.rs)
  │      ├──> TerminalGuard & Crossterm alternate screen / raw mode
  │      ├──> App State & Action Dispatcher (src/tui/app.rs)
  │      ├──> Layout & Frame Orchestrator (src/tui/render.rs)
  │      ├──> Views: Video / Anime / Subtitle cards (src/tui/views.rs)
  │      ├──> Dialogs: Syncing spinner & completion popup (src/tui/dialogs.rs)
  │      └──> Theme Tokens & Badges (src/tui/theme.rs)
  │
  ├──> Scanner (src/scanner.rs) ──> WalkDir ~/Videos & CWD (depth 8) ──> Deduplicated Raw Videos
  ├──> Parser (src/parser.rs) ──> AnimeMetadata { title, episode }
  ├──> Search (src/search.rs) ──> Live Fuzzy Match & Skim Scorer ──> UnifiedAnimeEntry
  ├──> Provider (src/provider.rs)
  │      ├──> JimakuProvider (src/provider/jimaku.rs) ──> Jimaku.cc Scraper / Local Cache
  │      └──> KitsunekkoProvider (src/provider/kitsunekko.rs) ──> Kitsunekko Scraper / Local Cache
  ├──> Syncer (src/syncer.rs) ──> ALASS Audio Voice Alignment / Manual Shift
  └──> Legacy CLI UI (src/ui.rs) ──> Inquire Prompts, Spinners & Render Config (`--cli`)
```

## 2. Global Constraints & Architecture Patterns
- **Primary Language & Edition**: Rust 2021 edition (1.80+)
- **Architectural Paradigm**: Full-screen Ratatui (0.29) TUI with Crossterm (0.28) raw mode, fallback legacy CLI (`--cli`), and role-based domain separation (`scanner`, `parser`, `search`, `provider`, `syncer`, `tui`).
- **Hard Constraints**: <400 lines/file (<300 soft), <60 lines/fn, zero production `unwrap()`/`expect()`, 0 warnings (`cargo clippy -- -D warnings`).
- **Target Distribution**: Linux `x86_64` standalone binary (`subsink-linux-x86_64.tar.gz`) via GitHub Releases and local binary install (`~/.local/bin/subsink`).

## 3. Module & Interface Skeleton

### `src/main.rs` (Role: cli/orchestrator, Lines: 282)
- **Responsibility**: Coordinates TUI default entrypoint (`run_tui_app`) with legacy line-prompt CLI mode (`--cli`), help/version flag dispatching.
- **Imports**: `crate::parser`, `crate::provider`, `crate::scanner`, `crate::search`, `crate::syncer`, `crate::tui`, `crate::ui`, `inquire::{Select, Text}`, `anyhow::*`
- **Types & Enums**:
  ```rust
  enum CliAction { Tui, Cli, Help, Version }
  ```
- **Public Functions & Signatures**:
  ```rust
  async fn main() -> Result<()>
  fn parse_cli_args(args: &[String]) -> Result<CliAction>
  fn print_help()
  ```
- **Consumers**: Process entrypoint (`bin "subsink"`).

### `src/tui.rs` (Role: tui/entry, Lines: 209)
- **Responsibility**: Manages terminal lifecycle via RAII `TerminalGuard`, raw mode, panic hooks, frame draw loop (60ms event polling), and async action dispatching.
- **Public Functions & Signatures**:
  ```rust
  pub struct TerminalGuard;
  pub async fn run_tui(videos: Vec<PathBuf>, entries: Vec<UnifiedAnimeEntry>) -> Result<()>
  ```

### `src/tui/app.rs` (Role: tui/state, Lines: 319)
- **Responsibility**: Central state machine tracking current step (`SelectVideo`, `SearchAnime`, `SelectSubtitle`, `Syncing`, `Completed`), query filters, selection indices, loading indicators, and keyboard events.
- **Types & Enums**:
  ```rust
  pub enum AppStep { SelectVideo, SearchAnime, SelectSubtitle, Syncing, Completed }
  pub enum AppAction { FetchSubtitles(UnifiedAnimeEntry, PathBuf), DownloadAndSync(SubtitleSearchResult, PathBuf), PlayMpv(PathBuf, PathBuf) }
  pub struct App { ... }
  ```

### `src/tui/theme.rs` (Role: tui/theme, Lines: 48)
- **Responsibility**: Centralized color tokens (accent cyan, success green, magenta Jimaku badge, yellow Kitsunekko badge), rounded border block constructors, and highlighted list styles.

### `src/tui/render.rs` (Role: tui/render, Lines: 114)
- **Responsibility**: High-level layout frame orchestrator: 3-row layout (top stepper header, active step view/modal, bottom hotkey bar).
- **Public Functions & Signatures**:
  ```rust
  pub fn render(frame: &mut Frame<'_>, app: &App)
  ```

### `src/tui/views.rs` (Role: tui/views, Lines: 319)
- **Responsibility**: Split two-column card rendering:
  - Video selector: live filename filter, natural sorted list, parsed metadata inspector.
  - Anime database search: live fuzzy query input, candidate entries, multi-lingual name breakdown, provider availability tags.
  - Subtitle release browser: tagged releases (`[ja] [Jimaku.cc] / [Kitsunekko]`), format details, release title.

### `src/tui/dialogs.rs` (Role: tui/dialogs, Lines: 108)
- **Responsibility**: Centered modal popups for active audio synchronization (spinning braille indicator) and completion summary card with playback trigger (`[p]` launch in mpv).

### `src/search.rs` (Role: domain/search, Lines: 196)
- **Responsibility**: Real-time fuzzy anime title search, multi-token matching, case-insensitive scoring (`SkimMatcherV2`), and fallback ranking across Romaji, English, and Japanese names.

### `src/scanner.rs` (Role: infra/io, Lines: 349)
- **Responsibility**: Recursively scans `~/Videos` and current working directory up to depth 8 for anime video files, deduplicates canonical paths, formats relative display paths, prioritizes files in CWD, and sorts episode numbers naturally.

### `src/parser.rs` (Role: domain/logic, Lines: 287)
- **Responsibility**: Extraction of anime title, season (`Option<u32>`), and episode (`Option<String>`) across scene formats (`S02E01`), standard end-of-stem numbers, end markers (`END`/`FINAL`), bracketed rips, specials with subtitles, and parent directory hierarchy fallback.

### `src/provider.rs` (Role: infra/api, Lines: 393)
- **Responsibility**: Coordinates multi-source subtitle retrieval across Jimaku.cc and Kitsunekko, maintains unified 30-day cache, resolves cross-provider titles via normalized alphanumeric hashing, and extracts subtitle files (.srt, .ass, .zip).
- **Submodules**: `pub mod jimaku; pub mod kitsunekko;`

### `src/provider/jimaku.rs` (Role: infra/api/jimaku, Lines: 178)
- **Responsibility**: Jimaku.cc index caching (`jimaku_index.json`), HTML parsing, and per-entry file scraping (`entry_{id}_files.json`).

### `src/provider/kitsunekko.rs` (Role: infra/api/kitsunekko, Lines: 194)
- **Responsibility**: Kitsunekko directory list caching (`kitsunekko_index.json`), URL-encoded path parsing, and folder file scraping (`kitsunekko_{slug}_files.json`).

### `src/syncer.rs` (Role: domain/infra, Lines: 329)
- **Responsibility**: Integrates with ALASS audio synchronization tool or executes manual millisecond subtitle offset shifting for SRT and ASS subtitle formats.

### `src/ui.rs` (Role: cli/presentation, Lines: 78)
- **Responsibility**: Custom console themes, banners, step indicators, render configurations, and progress spinners for `--cli` mode.

## 4. Execution Lifecycle Trace
1. **Startup**: `subsink` parses CLI flags. Launches interactive Ratatui TUI by default, or line-oriented inquirer wizard if `--cli` is passed.
2. **Step 1 (Video Picker)**: Video files in CWD and `~/Videos` are displayed in left list with natural episode sorting; right panel shows parsed metadata, file size, detected episode. Live filter narrows list.
3. **Step 2 (Database Search)**: Unified Jimaku & Kitsunekko anime index (~7,200 titles) with live fuzzy query input; right panel details Romaji, English, Japanese titles, and provider badges.
4. **Step 3 (Subtitle Release Browser)**: Releases from Jimaku and Kitsunekko displayed with language badge and color-coded provider tag; right panel shows format and source download URL.
5. **Step 4 (Download & Align Modal)**: Centered popup displays live spinner while downloading archive, unpacking `.zip` / `.ass`, and running ALASS audio voice activity alignment against video stream.
6. **Step 5 (Completion & Player Trigger)**: Modal displays resulting timing offset and destination path; user can press `[p]` to immediately launch mpv with synced subtitle or `[q]` to exit.

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
- **2026-11-17 (v0.1.9 - Full Ratatui TUI Interface)**:
  - Replaced line-prompt inquirer interface with a full-screen interactive **Ratatui (0.29)** TUI powered by **Crossterm (0.28)**.
  - Implemented 5-step stepper header (`Video ──► Database ──► Subtitle ──► Sync ──► Done`).
  - Implemented split two-column card views with live fuzzy filtering for videos, anime titles, and subtitle releases.
  - Implemented color-coded provider badges (`[Jimaku.cc]` in Magenta, `[Kitsunekko]` in Yellow).
  - Implemented animated audio synchronization popup dialog with braille spinner.
  - Implemented completion screen with instant `[p]` launch into `mpv`.
  - Retained `--cli` flag for headless / piped / legacy CLI workflows.
- **2026-11-17 (v0.1.8 - Kitsunekko Dual-Provider Integration)**:
  - Added native **Kitsunekko** provider support (`src/provider/kitsunekko.rs`) alongside Jimaku.cc.
  - Implemented `UnifiedAnimeEntry` with cross-provider linking via `normalize_title`.
  - Added concurrent dual-provider fetching (`tokio::join!`) with 30-day disk caching.
  - Added special/OVA preservation in `filter_subtitles`.
