use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct SubtitleSyncer;

impl SubtitleSyncer {
    pub fn new() -> Self {
        Self
    }

    /// Automatically sync unaligned subtitle to video audio track
    /// Returns Ok(Some(shift_info_msg)) if ALASS audio sync succeeded, Ok(None) if copied without shift.
    pub fn sync_subtitle(&self, video_path: &Path, raw_sub_path: &Path, output_sub_path: &Path) -> Result<Option<String>> {
        let alass_bin = dirs::home_dir()
            .map(|h| h.join(".local/bin/alass"))
            .unwrap_or_else(|| PathBuf::from("alass"));

        // 1. Run ALASS silently
        let mut cmd = Command::new(&alass_bin);
        cmd.arg("-g")
            .arg("-l")
            .arg(video_path)
            .arg(raw_sub_path)
            .arg(output_sub_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                let stderr_str = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}\n{}", stdout_str, stderr_str);

                let shift_info = parse_alass_shift(&combined);
                return Ok(Some(shift_info));
            }
        }

        // 2. Fallback check for alass-cli
        let mut cmd_cli = Command::new("alass-cli");
        cmd_cli
            .arg("-g")
            .arg("-l")
            .arg(video_path)
            .arg(raw_sub_path)
            .arg(output_sub_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Ok(output) = cmd_cli.output() {
            if output.status.success() {
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                let stderr_str = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}\n{}", stdout_str, stderr_str);

                let shift_info = parse_alass_shift(&combined);
                return Ok(Some(shift_info));
            }
        }

        // 3. Fallback: Direct Copy
        Self::fallback_sync(raw_sub_path, output_sub_path)?;
        Ok(None)
    }

    fn fallback_sync(raw_sub_path: &Path, output_sub_path: &Path) -> Result<()> {
        let ffmpeg_check = Command::new("ffmpeg").arg("-version").output();
        if ffmpeg_check.is_err() {
            return Err(anyhow!("ffmpeg, alass, or ffsubsync is required for audio subtitle synchronization"));
        }

        std::fs::copy(raw_sub_path, output_sub_path)
            .context("Failed to copy subtitle to destination")?;

        Ok(())
    }
}

fn parse_alass_shift(output: &str) -> String {
    let re = regex::Regex::new(r"shifted block of (\d+) subtitles.*by ([^\n]+)").unwrap();
    if let Some(cap) = re.captures(output) {
        let count = &cap[1];
        let offset = &cap[2];
        format!("Shifted {} subtitle lines by {}", count, offset.trim())
    } else {
        "Audio voice timing aligned successfully".to_string()
    }
}
