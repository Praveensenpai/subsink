# ⛩️ 字幕シンク • SUBSINK

> **Blazing-Fast Interactive TUI Japanese Anime Subtitle Fetcher & Voice-Aligned Auto-Syncer.**

[![Rust](https://img.shields.io/badge/Language-Rust-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Sync](https://img.shields.io/badge/Engine-ALASS-brightgreen.svg?style=flat-square)](https://github.com/kaegi/alass)
[![Jimaku](https://img.shields.io/badge/Provider-Jimaku.cc-purple.svg?style=flat-square)](https://jimaku.cc)
[![Kitsunekko](https://img.shields.io/badge/Provider-Kitsunekko-yellow.svg?style=flat-square)](https://kitsunekko.net)

---

```text
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│ ⛩️  S U B S I N K   [1. Video] ──► [2. Database] ──► [3. Subtitle] ──► [4. Sync] ──► [5. Done]  │
├──────────────────────────────────────────────────────────┬──────────────────────────────────┤
│ ❯ Search: heya camp                                      │ Video Details / Anime Metadata   │
│   • Heya Camp (Room Camp) [へやキャン△]                  │ ──────────────────────────────── │
│     Provider: [Jimaku.cc] [Kitsunekko]                   │ Romaji:    Heya Camp             │
│   • Yuru Camp (Laid-Back Camp) [ゆるキャン△]             │ English:   Room Camp             │
│   • Yuru Camp Season 2                                   │ Japanese:  へやキャン△           │
│                                                          │ Matches:   12 episodes + OVA/SP  │
├──────────────────────────────────────────────────────────┴──────────────────────────────────┤
│ Hotkeys: [↑/↓] Navigate  •  [Type] Fuzzy Search  •  [Enter] Select  •  [p] Play  •  [Esc] Back │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

**`subsink`** is a high-performance interactive terminal user interface (TUI) written in Rust designed for Japanese language learners and immersion fans. It automates discovering raw anime video files, fuzzy-searching 7,200+ Japanese anime titles across both **Jimaku.cc** and **Kitsunekko**, downloading subtitle archives, and automatically aligning speech timings to video audio using **ALASS**.

---

## ✨ Features

- 🖥️ **Full-Screen Interactive Ratatui TUI**: Built with `ratatui 0.29` & `crossterm 0.28`. Two-column split layout with real-time video inspection, live fuzzy database search, and audio alignment modals.
- 🦊 **Dual Subtitle Providers**: Smart dual aggregation querying both **Jimaku.cc** and **Kitsunekko** in parallel, linked via normalized title hashing. Never miss an OVA or TV episode.
- ⚡ **Instant Local Fuzzy Match**: Caches 7,200+ unified anime titles locally with 30-day disk caching for `< 5ms` instant search as you type.
- 🎯 **Automated ALASS Audio Alignment**: Uses voice activity detection (VAD) to shift subtitle timestamps directly against your video's audio stream.
- 🏷️ **Smart Filename Parser**: Automatically strips release group tags (`[VCB-Studio]`, `1080p`, `x265`, etc.) to extract clean anime titles and episode numbers.
- 🎬 **Direct MPV Playback**: Press `[p]` upon completion to immediately test synchronized subtitles in `mpv`.
- 🕹️ **Headless / CLI Mode**: Retains `--cli` flag for classic inquirer wizard workflows or scripted environments.

---

## ⚙️ Prerequisites

1. **Rust & Cargo**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. **ffmpeg** (for audio extraction):
   ```bash
   # Arch Linux / Omarchy
   sudo pacman -S ffmpeg
   ```
3. **ALASS** (Subtitle Synchronization Engine):
   ```bash
   cargo install alass-cli
   ```

---

## 🚀 Installation

### Quick Install (Pre-compiled Linux Binary)

```bash
curl -fsSL -H 'Cache-Control: no-cache' https://raw.githubusercontent.com/Praveensenpai/subsink/main/install.sh | bash
```

### Build from Source

```bash
git clone https://github.com/Praveensenpai/subsink.git
cd subsink
cargo build --release
install -m 755 target/release/subsink ~/.local/bin/subsink
```

---

## 🎧 Workflow Overview

```text
[Raw Video (.mkv)] ──► [subsink TUI] ──► [Jimaku + Kitsunekko] ──► [ALASS Audio Sync] ──► [<name>.ja.srt]
                                                                                                │
                                                                                                ▼
                                                                                         [mpv / impd]
```

1. Run `subsink` anywhere in your terminal.
2. Filter and choose your raw anime video file.
3. Live fuzzy search through 7,200+ anime titles across Jimaku and Kitsunekko.
4. Select your preferred subtitle release.
5. `subsink` downloads, aligns timestamps via ALASS, and outputs `<video_stem>.ja.<ext>`.
6. Press `[p]` to immediately launch `mpv` with synced subtitles, or `[q]` to exit.

---

## 📜 License

MIT License © [Praveensenpai](https://github.com/Praveensenpai)
