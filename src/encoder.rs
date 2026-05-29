use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
};

use crate::config::Config;
use anyhow::Result;

pub fn encode(input: &Path, output: &Path, cfg: &Config) -> Result<()> {
    let mut cmd = Command::new("HandBrakeCLI");
    cmd.arg("-i").arg(input);
    cmd.arg("-o").arg(output);

    if let Some(ref preset_file) = cfg.handbrake.preset_file {
        cmd.arg("--preset-import-file").arg(preset_file);
    }
    cmd.arg("--preset").arg(&cfg.handbrake.preset);
    cmd.args(&cfg.handbrake.extra_args);

    // Pipe stdout so we can parse progress; stderr inherits (HandBrake error messages stay visible).
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().expect("stdout was piped");

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("encode");
    let log_path = dbg!(cfg.handbrake.logging.get_log_file(stem));
    let mut log_writer: Option<BufWriter<File>> = dbg!(
        log_path
            .as_ref()
            .map(|p| {
                Ok::<_, anyhow::Error>(BufWriter::new(dbg!(
                    OpenOptions::new().create(true).append(true).open(p)?
                )))
            })
            .transpose()?
    );

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
                process_line(trimmed, &mut log_writer, cfg)?;
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Move the cursor to a new line so the next message doesn't overwrite the progress display.
    if cfg.handbrake.logging.show_progress {
        println!();
    }

    let status = child.wait()?;

    if !status.success() {
        return Err(EncodeError::HandBrakeFailed(status.code()).into());
    }

    Ok(())
}

fn process_line(line: &str, log_writer: &mut Option<BufWriter<File>>, cfg: &Config) -> Result<()> {
    if is_progress_line(line) {
        if cfg.handbrake.logging.show_progress {
            if let Some(pct) = parse_progress_pct(line) {
                print!("\rEncoding: {pct:.1}%   ");
                std::io::stdout().flush()?;
            }
        }
        // Progress lines are not written to the log — they're too noisy.
    } else if let Some(writer) = log_writer {
        writeln!(writer, "{line}")?;
    } else {
        println!("Writer was None, line: {line}");
    }
    Ok(())
}

fn is_progress_line(line: &str) -> bool {
    line.starts_with("Encoding:") || line.starts_with("Muxing:")
}

fn parse_progress_pct(line: &str) -> Option<f32> {
    // Input: "Encoding: task 1 of 1, 45.67 % (120.00 fps, avg 115.43 fps, ETA 00h15m30s)"
    // Find the `%`, take everything before it, then extract the number immediately before.
    let pct_pos = line.find('%')?;
    let before = line[..pct_pos].trim_end();
    let num_start = before.rfind(|c: char| !c.is_ascii_digit() && c != '.')? + 1;
    before.get(num_start..)?.parse().ok()
}

#[derive(Debug)]
enum EncodeError {
    HandBrakeFailed(Option<i32>),
}

impl std::error::Error for EncodeError {}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::HandBrakeFailed(Some(code)) => {
                write!(f, "HandBrakeCLI failed with exit code {code}")
            }
            EncodeError::HandBrakeFailed(None) => write!(f, "HandBrakeCLI failed (no exit code)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_progress_line ---

    #[test]
    fn progress_line_encoding_prefix() {
        assert!(is_progress_line(
            "Encoding: task 1 of 1, 45.67 % (120.00 fps, avg 115.43 fps, ETA 00h15m30s)"
        ));
    }

    #[test]
    fn progress_line_muxing_prefix() {
        assert!(is_progress_line("Muxing: task 2 of 2, 99.00 %"));
    }

    #[test]
    fn progress_line_rejects_other_output() {
        assert!(!is_progress_line("Opening /dev/sr0"));
        assert!(!is_progress_line("[12:34:56] scan: scanning title 1 of 10"));
        assert!(!is_progress_line(""));
    }

    // --- parse_progress_pct ---

    #[test]
    fn parse_progress_typical_line() {
        let pct = parse_progress_pct(
            "Encoding: task 1 of 1, 45.67 % (120.00 fps, avg 115.43 fps, ETA 00h15m30s)",
        )
        .unwrap();
        assert!((pct - 45.67).abs() < 0.01, "expected ~45.67, got {pct}");
    }

    #[test]
    fn parse_progress_zero() {
        let pct = parse_progress_pct(
            "Encoding: task 1 of 1, 0.00 % (0.00 fps, avg 0.00 fps, ETA 00h00m00s)",
        )
        .unwrap();
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn parse_progress_hundred() {
        let pct = parse_progress_pct("Encoding: task 1 of 1, 100.00 %").unwrap();
        assert!((pct - 100.0).abs() < 0.01, "expected ~100.0, got {pct}");
    }

    #[test]
    fn parse_progress_muxing_line() {
        let pct = parse_progress_pct("Muxing: task 2 of 2, 75.50 %").unwrap();
        assert!((pct - 75.50).abs() < 0.01, "expected ~75.50, got {pct}");
    }

    #[test]
    fn parse_progress_no_percent_returns_none() {
        assert_eq!(parse_progress_pct("Opening /dev/sr0"), None);
        assert_eq!(parse_progress_pct(""), None);
    }

    #[test]
    fn parse_progress_without_trailing_detail() {
        // Some builds omit the fps/ETA detail.
        let pct = parse_progress_pct("Encoding: task 1 of 1, 33.33 %").unwrap();
        assert!((pct - 33.33).abs() < 0.01, "expected ~33.33, got {pct}");
    }
}
