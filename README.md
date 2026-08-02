# ⛩️ subsink

> **Minimalist, Blazing-Fast Japanese Anime Subtitle Fetcher & Auto-Syncer for Passive Language Immersion.**

[![Rust](https://img.shields.io/badge/Language-Rust-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Sync](https://img.shields.io/badge/Engine-ALASS-brightgreen.svg?style=flat-square)](https://github.com/kaegi/alass)
[![Provider](https://img.shields.io/badge/Provider-Jimaku.cc-purple.svg?style=flat-square)](https://jimaku.cc)

---

```text
┌───────────────────────────────────────────────────────────────┐
│        ⛩️  S U B S I N K  ──  Japanese Anime SubSyncer        │
└───────────────────────────────────────────────────────────────┘
```

**`subsink`** is a high-performance terminal UI utility written in Rust designed for Japanese language learners and passive immersion fans. It automates selecting raw anime files, fuzzy-searching 4,500+ Japanese anime titles from **Jimaku.cc**, downloading subtitle archives, and automatically aligning speech timings to video audio using **ALASS**.

---

## ✨ Features

- ⚡ **Instant Local Fuzzy Match**: Caches Jimaku's 4,500+ anime index locally for instant `< 5ms` live fuzzy searching as you type.
- 🎯 **Automated ALASS Audio Alignment**: Uses voice activity detection (VAD) to shift subtitle timestamps directly against your video's audio stream.
- 🏷️ **Smart Filename Parser**: Automatically strips release group tags (`[VCB-Studio]`, `1080p`, `x265`, etc.) to extract clean anime titles and episode numbers.
- 📦 **30-Day Per-Anime Cache**: Remembers episode file listings for 30 days—binge 12 episodes with zero network delay.
- 🎨 **Minimalist Aesthetic TUI**: Clean terminal cards, smooth progress spinners, and deep anime color palette.

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
[Raw Video (.mkv)] ──► [subsink] ──► [Jimaku.cc Search] ──► [ALASS Audio Sync] ──► [<video_name>.ja.srt]
                                                                                            │
                                                                                            ▼
                                                                                     [immersionpod / impd]
```

1. Run `subsink` anywhere in your terminal.
2. Select your raw anime video file from `~/Videos`.
3. Type to filter through 4,500+ anime titles live.
4. Select the matching episode subtitle.
5. `subsink` downloads, aligns timestamps via ALASS, and outputs `<video_name>.ja.srt` alongside your raw video file for instant use with **`impd`** / **`mpd`** / **`mpv`**.

---

## 📜 License

MIT License © [Praveensenpai](https://github.com/Praveensenpai)
