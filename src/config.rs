use anyhow::Result;
use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::cli::{Cli, Command, EncodeArgs, RunArgs};

#[derive(Deserialize, Default, Clone)]
pub struct Config {
    pub paths: PathsConfig,
    pub makemkv: MakeMkvConfig,
    pub encoding: EncodingConfig,
    pub upload: UploadConfig,
}

#[derive(Deserialize, Default, Clone)]
pub struct PathsConfig {
    pub output_dir: PathBuf,        // Fill with sensible default
    pub nas_mount: Option<PathBuf>, // Make sure user has set this
}

#[derive(Deserialize, Default, Clone)]
pub struct MakeMkvConfig {
    pub min_length_seconds: u64,
}

#[derive(Deserialize, Default, Clone)]
pub struct EncodingConfig {
    pub preset: Option<String>,
    pub preset_file: Option<PathBuf>,
}

#[derive(Deserialize, Default, Clone)]
pub struct UploadConfig {
    pub no_upload: bool,
}

impl Config {
    pub fn load(cli: &Cli) -> Result<Self> {
        let mut cfg = match &cli.config {
            Some(p) => toml::from_str(&fs::read_to_string(p)?)?,
            None => Self::default(),
        };

        match &cli.command {
            Command::Run(args) => cfg.apply_run_overrides(&args),
            Command::Encode(args) => cfg.apply_encode_overrides(&args),
            _ => {}
        }

        Ok(cfg)
    }

    fn apply_run_overrides(&mut self, args: &RunArgs) {
        apply_override(&args.output_dir, |v| self.paths.output_dir = v);
        apply_override(&args.preset, |v| self.encoding.preset = Some(v));
        apply_override(&args.preset_file, |v| self.encoding.preset_file = Some(v));
        apply_override(&args.no_upload, |v| self.upload.no_upload = v);
    }

    fn apply_encode_overrides(&mut self, args: &EncodeArgs) {
        apply_override(&args.preset, |v| self.encoding.preset = Some(v));
        apply_override(&args.preset_file, |v| self.encoding.preset_file = Some(v));
    }
}

fn apply_override<T: Clone>(value: &Option<T>, f: impl FnOnce(T)) {
    if let Some(v) = value {
        f(v.clone());
    }
}
