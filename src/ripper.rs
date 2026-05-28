use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::config::Config;
use anyhow::Result;

const ATTR_DURATION: u32 = 9;

pub struct DiscTitle {
    pub id: u32,
    pub duration_seconds: u64,
}

/// Runs `makemkvcon -r info` and returns the list of titles found on the disc.
pub fn get_disc_info(cfg: &Config) -> Result<Vec<DiscTitle>> {
    let output = Command::new("makemkvcon")
        .args(["-r", "info", &cfg.makemkv.disc_device])
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "makemkvcon info failed with exit code {:?}",
            output.status.code()
        ));
    }

    Ok(parse_disc_info(&String::from_utf8_lossy(&output.stdout)))
}

/// Returns a min-length (seconds) that captures only the longest title on the disc.
/// Returns `None` if `titles` is empty, in which case the caller should use the configured default.
pub fn smart_min_length_seconds(titles: &[DiscTitle]) -> Option<u64> {
    let max = titles.iter().map(|t| t.duration_seconds).max()?;
    Some(max.saturating_sub(60))
}

pub fn rip_disc(title: &str, min_length_seconds: u64, cfg: &Config) -> Result<PathBuf> {
    let output_path = cfg.paths.output_dir.join(sanitize_filename(title));
    fs::create_dir_all(&output_path)?;

    let output_path_str = output_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("output path contains non-UTF-8 characters"))?;

    let mut cmd = Command::new("makemkvcon");

    if let Some(ref log_file) = cfg.makemkv.logging.log_file {
        cmd.arg(format!("--messages={}", log_file.display()));
    }

    if cfg.makemkv.logging.show_progress {
        cmd.arg("--progress=-stdout");
    } else {
        cmd.arg("--progress=-null");
    }

    cmd.args(["mkv", &cfg.makemkv.disc_device, "all", output_path_str]);
    cmd.arg(format!("--minlength={min_length_seconds}"));
    cmd.args(&cfg.makemkv.extra_args);

    let status = cmd.status()?;

    if !status.success() {
        return Err(RipError::MakeMkvFailed(status.code()).into());
    }

    find_main_feature(&output_path)
}

pub fn sanitize_filename(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric()
                || matches!(c, ' ' | '.' | '_' | '-' | '(' | ')' | '\'' | '!')
            {
                c
            } else {
                '_'
            }
        })
        .collect();

    let trimmed = sanitized.trim_matches(|c: char| c == '_' || c.is_ascii_whitespace());
    if trimmed.is_empty() { "_" } else { trimmed }.to_string()
}

fn find_main_feature(output_path: &Path) -> Result<PathBuf> {
    let best = fs::read_dir(output_path)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("mkv"))
        .filter_map(|e| {
            let size = e.metadata().ok()?.len();
            Some((e.path(), size))
        })
        .max_by_key(|(_, size)| *size)
        .map(|(path, _)| path);

    best.ok_or_else(|| anyhow::anyhow!("no MKV file found in {}", output_path.display()))
}

fn parse_disc_info(output: &str) -> Vec<DiscTitle> {
    let mut durations: HashMap<u32, u64> = HashMap::new();

    for line in output.lines() {
        let Some(rest) = line.strip_prefix("TINFO:") else {
            continue;
        };

        let mut fields = rest.splitn(4, ',');
        let (Some(id_s), Some(attr_s), Some(_), Some(val_s)) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            continue;
        };

        let Ok(title_id) = id_s.parse::<u32>() else { continue };
        let Ok(attr_id) = attr_s.parse::<u32>() else { continue };
        if attr_id != ATTR_DURATION {
            continue;
        }

        if let Some(secs) = parse_duration(val_s.trim_matches('"')) {
            durations.insert(title_id, secs);
        }
    }

    let mut titles: Vec<DiscTitle> = durations
        .into_iter()
        .map(|(id, duration_seconds)| DiscTitle { id, duration_seconds })
        .collect();
    titles.sort_by_key(|t| t.id);
    titles
}

fn parse_duration(s: &str) -> Option<u64> {
    let mut parts = s.split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(hours * 3600 + minutes * 60 + seconds)
}

#[derive(Debug, Clone)]
enum RipError {
    MakeMkvFailed(Option<i32>),
}

impl std::error::Error for RipError {}

impl std::fmt::Display for RipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RipError::MakeMkvFailed(Some(code)) => {
                write!(f, "makemkv failed with exit code {code}")
            }
            RipError::MakeMkvFailed(None) => write!(f, "makemkv failed (no exit code)"),
        }
    }
}
