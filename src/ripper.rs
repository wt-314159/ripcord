use std::{error::Error, fs, path::PathBuf, process::Command};

use crate::config::Config;
use anyhow::Result;

pub fn rip_disc(title: &str, cfg: &Config) -> Result<PathBuf> {
    let output_path = cfg.paths.output_dir.join(sanitize_filename(title));
    fs::create_dir_all(&output_path)?;

    let status = Command::new("makemkvcon")
        .args([
            "mkv",
            "disc:0",
            "all",
            output_path.to_str().unwrap(),
            &format!("--minlength={}", cfg.makemkv.min_length_seconds),
        ])
        .status()?;

    if !status.success() {
        return Err(RipError::MakeMkvFailed(status.code()).into());
    }

    find_main_feature(&output_path)
}

fn sanitize_filename(title: &str) -> String {
    todo!("sanitize filename")
}

fn find_main_feature(output_path: &PathBuf) -> Result<PathBuf> {
    todo!("find main feature")
}

#[derive(Debug, Clone)]
enum RipError {
    MakeMkvFailed(Option<i32>),
}

impl std::error::Error for RipError {
    fn description(&self) -> &str {
        match self {
            RipError::MakeMkvFailed(_) => "makemkv failed",
        }
    }
}

impl std::fmt::Display for RipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
