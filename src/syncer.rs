use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq)]
pub enum SyncResult {
    Success(String),
    WarningLargeShift(String),
    DirectCopy,
}

pub struct SubtitleSyncer;

impl SubtitleSyncer {
    pub fn new() -> Self {
        Self
    }

    /// Automatically sync unaligned subtitle to video audio track
    pub fn sync_subtitle(&self, video_path: &Path, raw_sub_path: &Path, output_sub_path: &Path) -> Result<SyncResult> {
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

                return Ok(evaluate_alass_output(&combined));
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

                return Ok(evaluate_alass_output(&combined));
            }
        }

        // 3. Fallback: Direct Copy
        Self::fallback_sync(raw_sub_path, output_sub_path)?;
        Ok(SyncResult::DirectCopy)
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

pub fn parse_alass_shift(output: &str) -> String {
    let re = regex::Regex::new(r"shifted block of (\d+) subtitles.*by ([^\n]+)").unwrap();
    if let Some(cap) = re.captures(output) {
        let count = &cap[1];
        let offset = &cap[2];
        format!("Shifted {} subtitle lines by {}", count, offset.trim())
    } else {
        "Audio voice timing aligned successfully".to_string()
    }
}

pub fn evaluate_alass_output(output: &str) -> SyncResult {
    let shift_info = parse_alass_shift(output);

    let has_neg_warning = output.contains("negative timings");
    let is_large = if let Some(offset_sec) = parse_alass_offset_seconds(output) {
        offset_sec.abs() > 10.0
    } else {
        false
    };

    if has_neg_warning || is_large {
        SyncResult::WarningLargeShift(shift_info)
    } else {
        SyncResult::Success(shift_info)
    }
}

pub fn parse_alass_offset_seconds(output: &str) -> Option<f64> {
    let re = regex::Regex::new(r"shifted block of \d+ subtitles.*by (-?)(?:(\d+):)?(\d+):(\d+(?:\.\d+)?)").unwrap();
    if let Some(cap) = re.captures(output) {
        let is_neg = &cap[1] == "-";
        let h: f64 = cap.get(2).map_or(0.0, |m| m.as_str().parse().unwrap_or(0.0));
        let m: f64 = cap[3].parse().unwrap_or(0.0);
        let s: f64 = cap[4].parse().unwrap_or(0.0);
        let total = h * 3600.0 + m * 60.0 + s;
        Some(if is_neg { -total } else { total })
    } else {
        None
    }
}

pub fn parse_offset_string(input: &str) -> Result<i64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }

    if let Some(ms_str) = trimmed.strip_suffix("ms").or_else(|| trimmed.strip_suffix("MS")) {
        let ms: i64 = ms_str.trim().parse().context("Invalid milliseconds offset")?;
        return Ok(ms);
    }

    if let Some(s_str) = trimmed.strip_suffix('s').or_else(|| trimmed.strip_suffix('S')) {
        let s: f64 = s_str.trim().parse().context("Invalid seconds offset")?;
        return Ok((s * 1000.0).round() as i64);
    }

    if trimmed.contains('.') {
        let s: f64 = trimmed.parse().context("Invalid seconds offset")?;
        return Ok((s * 1000.0).round() as i64);
    }

    let ms: i64 = trimmed.parse().context("Invalid millisecond offset")?;
    Ok(ms)
}

pub fn apply_manual_offset(input_path: &Path, output_path: &Path, offset_ms: i64) -> Result<()> {
    if offset_ms == 0 {
        std::fs::copy(input_path, output_path)?;
        return Ok(());
    }

    let content = std::fs::read_to_string(input_path)
        .context("Failed to read subtitle file for manual offset")?;

    let shifted = shift_subtitle_text(&content, offset_ms);
    std::fs::write(output_path, shifted)
        .context("Failed to write shifted subtitle file")?;

    Ok(())
}

pub fn shift_subtitle_text(content: &str, offset_ms: i64) -> String {
    let srt_re = regex::Regex::new(r"(\d{2}:\d{2}:\d{2}[,\.]\d{3})\s*-->\s*(\d{2}:\d{2}:\d{2}[,\.]\d{3})").unwrap();
    let ass_dialogue_re = regex::Regex::new(r"^(Dialogue:\s*[^,]+,)(\d+:\d{2}:\d{2}\.\d{2}),(\d+:\d{2}:\d{2}\.\d{2}),(.*)$").unwrap();

    let mut result_lines = Vec::new();

    for line in content.lines() {
        if srt_re.is_match(line) {
            let replaced = srt_re.replace_all(line, |caps: &regex::Captures| {
                let start = shift_srt_timestamp(&caps[1], offset_ms);
                let end = shift_srt_timestamp(&caps[2], offset_ms);
                format!("{} --> {}", start, end)
            });
            result_lines.push(replaced.to_string());
        } else if ass_dialogue_re.is_match(line) {
            let replaced = ass_dialogue_re.replace(line, |caps: &regex::Captures| {
                let prefix = &caps[1];
                let start = shift_ass_timestamp(&caps[2], offset_ms);
                let end = shift_ass_timestamp(&caps[3], offset_ms);
                let rest = &caps[4];
                format!("{}{},{},{}", prefix, start, end, rest)
            });
            result_lines.push(replaced.to_string());
        } else {
            result_lines.push(line.to_string());
        }
    }

    let mut out = result_lines.join("\n");
    if content.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn shift_srt_timestamp(ts: &str, offset_ms: i64) -> String {
    let parts: Vec<&str> = ts.split(|c| c == ':' || c == ',' || c == '.').collect();
    if parts.len() == 4 {
        if let (Ok(h), Ok(m), Ok(s), Ok(ms)) = (
            parts[0].parse::<i64>(),
            parts[1].parse::<i64>(),
            parts[2].parse::<i64>(),
            parts[3].parse::<i64>(),
        ) {
            let total = h * 3_600_000 + m * 60_000 + s * 1000 + ms;
            let shifted = (total + offset_ms).max(0);
            let new_h = shifted / 3_600_000;
            let rem = shifted % 3_600_000;
            let new_m = rem / 60_000;
            let rem2 = rem % 60_000;
            let new_s = rem2 / 1000;
            let new_ms = rem2 % 1000;
            return format!("{:02}:{:02}:{:02},{:03}", new_h, new_m, new_s, new_ms);
        }
    }
    ts.to_string()
}

fn shift_ass_timestamp(ts: &str, offset_ms: i64) -> String {
    let parts: Vec<&str> = ts.split(|c| c == ':' || c == '.').collect();
    if parts.len() == 4 {
        if let (Ok(h), Ok(m), Ok(s), Ok(cs)) = (
            parts[0].parse::<i64>(),
            parts[1].parse::<i64>(),
            parts[2].parse::<i64>(),
            parts[3].parse::<i64>(),
        ) {
            let total = h * 3_600_000 + m * 60_000 + s * 1000 + cs * 10;
            let shifted = (total + offset_ms).max(0);
            let new_h = shifted / 3_600_000;
            let rem = shifted % 3_600_000;
            let new_m = rem / 60_000;
            let rem2 = rem % 60_000;
            let new_s = rem2 / 1000;
            let new_cs = (rem2 % 1000) / 10;
            return format!("{}:{:02}:{:02}.{:02}", new_h, new_m, new_s, new_cs);
        }
    }
    ts.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_offset_strings() {
        assert_eq!(parse_offset_string("500").unwrap(), 500);
        assert_eq!(parse_offset_string("+500").unwrap(), 500);
        assert_eq!(parse_offset_string("-1000").unwrap(), -1000);
        assert_eq!(parse_offset_string("250ms").unwrap(), 250);
        assert_eq!(parse_offset_string("-250ms").unwrap(), -250);
        assert_eq!(parse_offset_string("1.5").unwrap(), 1500);
        assert_eq!(parse_offset_string("+1.5s").unwrap(), 1500);
        assert_eq!(parse_offset_string("-0.8s").unwrap(), -800);
        assert_eq!(parse_offset_string("0").unwrap(), 0);
    }

    #[test]
    fn test_shift_srt_subtitles() {
        let srt = "1\n00:00:03,100 --> 00:00:05,400\nHello\n";
        let shifted = shift_subtitle_text(srt, 500);
        assert!(shifted.contains("00:00:03,600 --> 00:00:05,900"));

        let shifted_back = shift_subtitle_text(srt, -1000);
        assert!(shifted_back.contains("00:00:02,100 --> 00:00:04,400"));
    }

    #[test]
    fn test_shift_ass_subtitles() {
        let ass = "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:01:05.20,0:01:08.50,Default,,0,0,0,,Konichiwa\n";
        let shifted = shift_subtitle_text(ass, 1000);
        assert!(shifted.contains("Dialogue: 0,0:01:06.20,0:01:09.50,Default,,0,0,0,,Konichiwa"));
    }

    #[test]
    fn test_evaluate_alass_output_safety() {
        let safe_out = "shifted block of 559 subtitles with length 0:23:30.617 by 0:00:00.000";
        assert_eq!(
            evaluate_alass_output(safe_out),
            SyncResult::Success("Shifted 559 subtitle lines by 0:00:00.000".to_string())
        );

        let small_shift = "shifted block of 500 subtitles with length 0:23:30.000 by 0:00:01.250";
        assert_eq!(
            evaluate_alass_output(small_shift),
            SyncResult::Success("Shifted 500 subtitle lines by 0:00:01.250".to_string())
        );

        let large_shift = "shifted block of 1064 subtitles with length 1:10:15.122 by -0:19:24.900\nwarn: some subtitles now have negative timings";
        assert_eq!(
            evaluate_alass_output(large_shift),
            SyncResult::WarningLargeShift("Shifted 1064 subtitle lines by -0:19:24.900".to_string())
        );
    }
}
