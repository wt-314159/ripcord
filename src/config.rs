use anyhow::Result;
use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::{Cli, Command, EncodeArgs, RunArgs};
use crate::types::{ExtrasMode, OutputFormat};

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    pub paths: PathsConfig,
    pub makemkv: MakeMkvConfig,
    pub handbrake: HandBrakeConfig,
    pub upload: UploadConfig,
    pub extras: ExtrasConfig,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct PathsConfig {
    pub output_dir: PathBuf,
    pub nas_mount: Option<PathBuf>,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            output_dir: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("rips"),
            nas_mount: None,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct MakeMkvConfig {
    pub min_length_seconds: u64,
    pub disc_device: String,
    pub extra_args: Vec<String>,
    pub logging: LoggingConfig,
}

impl Default for MakeMkvConfig {
    fn default() -> Self {
        Self {
            min_length_seconds: 120,
            disc_device: String::from("disc:0"),
            extra_args: Vec::new(),
            logging: LoggingConfig::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct HandBrakeConfig {
    pub preset: String,
    pub preset_file: Option<PathBuf>,
    pub extra_args: Vec<String>,
    pub output_format: OutputFormat,
    pub logging: LoggingConfig,
}

impl Default for HandBrakeConfig {
    fn default() -> Self {
        Self {
            preset: String::from("H.265 MKV 1080p30"),
            preset_file: None,
            extra_args: Vec::new(),
            output_format: OutputFormat::default(),
            logging: LoggingConfig::default(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    pub log_file: Option<PathBuf>,
    pub show_progress: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_file: None,
            show_progress: true,
        }
    }
}

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct UploadConfig {
    pub no_upload: bool,
    pub direct_to_nas: bool,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ExtrasConfig {
    pub mode: ExtrasMode,
    pub encode: bool,
    pub upload: bool,
    pub folder_name: String,
}

impl Default for ExtrasConfig {
    fn default() -> Self {
        Self {
            mode: ExtrasMode::Skip,
            encode: false,
            upload: false,
            folder_name: String::from("extras"),
        }
    }
}

impl Config {
    pub fn load(cli: &Cli) -> Result<Self> {
        let mut cfg = match &cli.config {
            Some(p) => toml::from_str(&fs::read_to_string(p)?)?,
            None => Self::default(),
        };

        match &cli.command {
            Some(Command::Run(args)) => cfg.apply_run_overrides(args),
            Some(Command::Encode(args)) => cfg.apply_encode_overrides(args),
            _ => {}
        }

        Ok(cfg)
    }

    fn apply_run_overrides(&mut self, args: &RunArgs) {
        apply_override(&args.output_dir, |v| self.paths.output_dir = v);
        apply_override(&args.preset, |v| self.handbrake.preset = v);
        apply_override(&args.preset_file, |v| self.handbrake.preset_file = Some(v));
        if args.no_upload {
            self.upload.no_upload = true;
        }
        apply_override(&args.disc_device, |v| self.makemkv.disc_device = v);
        apply_override(&args.mkv_args, |v| {
            self.makemkv.extra_args = v.split_whitespace().map(String::from).collect();
        });
        apply_override(&args.hb_args, |v| {
            self.handbrake.extra_args = v.split_whitespace().map(String::from).collect();
        });
        apply_override(&args.output_format, |v| self.handbrake.output_format = v);
    }

    fn apply_encode_overrides(&mut self, args: &EncodeArgs) {
        apply_override(&args.preset, |v| self.handbrake.preset = v);
        apply_override(&args.preset_file, |v| self.handbrake.preset_file = Some(v));
        apply_override(&args.hb_args, |v| {
            self.handbrake.extra_args = v.split_whitespace().map(String::from).collect();
        });
        apply_override(&args.output_format, |v| self.handbrake.output_format = v);
        if args.no_upload {
            self.upload.no_upload = true;
        }
    }
}

fn apply_override<T: Clone>(value: &Option<T>, f: impl FnOnce(T)) {
    if let Some(v) = value {
        f(v.clone());
    }
}
