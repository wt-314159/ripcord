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

    if let Some(log_file) = cfg.makemkv.logging.get_log_file(&sanitize_filename(title)) {
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
                || matches!(
                    c,
                    ' ' | '.' | '_' | '-' | '(' | ')' | '\'' | '!' | '[' | ']'
                )
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
        let (Some(id_s), Some(attr_s), Some(_), Some(val_s)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };

        let Ok(title_id) = id_s.parse::<u32>() else {
            continue;
        };
        let Ok(attr_id) = attr_s.parse::<u32>() else {
            continue;
        };
        if attr_id != ATTR_DURATION {
            continue;
        }

        if let Some(secs) = parse_duration(val_s.trim_matches('"')) {
            durations.insert(title_id, secs);
        }
    }

    let mut titles: Vec<DiscTitle> = durations
        .into_iter()
        .map(|(id, duration_seconds)| DiscTitle {
            id,
            duration_seconds,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- sanitize_filename ---

    #[test]
    fn sanitize_passes_through_allowed_chars() {
        assert_eq!(
            sanitize_filename("Schindler's List (1993)"),
            "Schindler's List (1993)"
        );
    }

    #[test]
    fn sanitize_replaces_colon() {
        assert_eq!(
            sanitize_filename("Batman: The Dark Knight"),
            "Batman_ The Dark Knight"
        );
    }

    #[test]
    fn sanitize_preserves_square_brackets() {
        assert_eq!(
            sanitize_filename("Meet the Robinsons (2007) [imdbid-tt0396555]"),
            "Meet the Robinsons (2007) [imdbid-tt0396555]"
        );
    }

    #[test]
    fn sanitize_replaces_slashes() {
        assert_eq!(sanitize_filename("A/B\\C"), "A_B_C");
    }

    #[test]
    fn sanitize_trims_leading_trailing_spaces() {
        assert_eq!(sanitize_filename("  Movie  "), "Movie");
    }

    #[test]
    fn sanitize_trims_leading_trailing_underscores() {
        assert_eq!(sanitize_filename("___title___"), "title");
    }

    #[test]
    fn sanitize_all_unsafe_chars_returns_placeholder() {
        assert_eq!(sanitize_filename("@#$%"), "_");
    }

    #[test]
    fn sanitize_empty_string_returns_placeholder() {
        assert_eq!(sanitize_filename(""), "_");
    }

    #[test]
    fn sanitize_preserves_numbers_and_dots() {
        assert_eq!(
            sanitize_filename("2001. A Space Odyssey"),
            "2001. A Space Odyssey"
        );
    }

    // --- parse_duration ---

    #[test]
    fn parse_duration_standard() {
        assert_eq!(parse_duration("2:01:45"), Some(7305));
    }

    #[test]
    fn parse_duration_zero() {
        assert_eq!(parse_duration("0:00:00"), Some(0));
    }

    #[test]
    fn parse_duration_multi_hour() {
        assert_eq!(parse_duration("3:30:00"), Some(12600));
    }

    #[test]
    fn parse_duration_rejects_too_few_parts() {
        assert_eq!(parse_duration("1:30"), None);
    }

    #[test]
    fn parse_duration_rejects_too_many_parts() {
        assert_eq!(parse_duration("1:2:3:4"), None);
    }

    #[test]
    fn parse_duration_rejects_non_numeric() {
        assert_eq!(parse_duration("not_a_time"), None);
    }

    #[test]
    fn parse_duration_rejects_empty() {
        assert_eq!(parse_duration(""), None);
    }

    // --- smart_min_length_seconds ---

    #[test]
    fn smart_min_length_subtracts_60_from_max() {
        let titles = vec![
            DiscTitle {
                id: 0,
                duration_seconds: 300,
            },
            DiscTitle {
                id: 1,
                duration_seconds: 7305,
            },
            DiscTitle {
                id: 2,
                duration_seconds: 600,
            },
        ];
        assert_eq!(smart_min_length_seconds(&titles), Some(7245));
    }

    #[test]
    fn smart_min_length_empty_slice_returns_none() {
        assert_eq!(smart_min_length_seconds(&[]), None);
    }

    #[test]
    fn smart_min_length_saturates_at_zero() {
        // A very short disc shouldn't produce an underflowing value.
        let titles = vec![DiscTitle {
            id: 0,
            duration_seconds: 30,
        }];
        assert_eq!(smart_min_length_seconds(&titles), Some(0));
    }

    #[test]
    fn smart_min_length_single_title() {
        let titles = vec![DiscTitle {
            id: 0,
            duration_seconds: 7200,
        }];
        assert_eq!(smart_min_length_seconds(&titles), Some(7140));
    }

    // --- find_main_feature ---

    #[test]
    fn find_main_feature_picks_largest_mkv() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("title_t00.mkv"), vec![0u8; 1_000]).unwrap();
        fs::write(dir.path().join("title_t01.mkv"), vec![0u8; 5_000]).unwrap();
        fs::write(dir.path().join("title_t02.mkv"), vec![0u8; 2_000]).unwrap();

        let result = find_main_feature(dir.path()).unwrap();
        // Returns the full path, not just the filename.
        assert_eq!(result, dir.path().join("title_t01.mkv"));
    }

    #[test]
    fn find_main_feature_ignores_non_mkv_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("notes.txt"), vec![0u8; 9_999]).unwrap();
        fs::write(dir.path().join("title_t00.mkv"), vec![0u8; 100]).unwrap();

        let result = find_main_feature(dir.path()).unwrap();
        assert_eq!(result, dir.path().join("title_t00.mkv"));
    }

    #[test]
    fn find_main_feature_errors_when_no_mkv_present() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("somefile.txt"), b"content").unwrap();

        assert!(find_main_feature(dir.path()).is_err());
    }

    #[test]
    fn find_main_feature_errors_on_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_main_feature(dir.path()).is_err());
    }
}
