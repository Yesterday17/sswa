use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::str::FromStr;
use tempfile::{NamedTempFile, TempPath};

pub fn get_duration<P: AsRef<Path>>(video_path: P) -> anyhow::Result<u32> {
    let command = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_path.as_ref().to_str().unwrap()
        ])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut result = String::new();
    command.stdout.unwrap().read_to_string(&mut result)?;
    let num = result.trim().split_once(".").map_or(result.as_str(), |r| r.0);
    Ok(u32::from_str(num)?)
}

pub fn auto_cover<P: AsRef<Path>>(input_path: P, time: u32) -> anyhow::Result<TempPath> {
    let file = NamedTempFile::new()?;
    Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-y",
            "-ss", &format!("{time}"),
            "-i", input_path.as_ref().to_str().ok_or(anyhow::anyhow!("invalid character in video_path"))?,
            "-vf", "scale=960:600:force_original_aspect_ratio=decrease,pad=960:600:-1:-1:color=black",
            "-frames:v", "1",
            "-f", "image2",
            file.path().to_str().ok_or(anyhow::anyhow!("invalid character in temp file path"))?,
        ])
        .stderr(Stdio::piped())
        .spawn()?
        .wait()?;
    Ok(file.into_temp_path())
}

pub fn scale_cover<P: AsRef<Path>>(input_path: P) -> anyhow::Result<TempPath> {
    let file = NamedTempFile::new()?;
    Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-y",
            "-i", input_path.as_ref().to_str().ok_or(anyhow::anyhow!("invalid character in video_path"))?,
            "-vf", "scale=960:600:force_original_aspect_ratio=decrease,pad=960:600:-1:-1:color=black",
            "-f", "image2",
            file.path().to_str().ok_or(anyhow::anyhow!("invalid character in temp file path"))?,
        ])
        .stderr(Stdio::piped())
        .spawn()?
        .wait()?;
    Ok(file.into_temp_path())
}