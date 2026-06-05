use anyhow::Result;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::cli::{Cli, Command, EncodeArgs, RunArgs, UploadArgs};
use crate::types::{ExtrasMode, OutputFormat};

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    pub paths: PathsConfig,
    pub makemkv: MakeMkvConfig,
    pub handbrake: HandBrakeConfig,
    pub dvdread: DvdReadConfig,
    pub upload: UploadConfig,
    pub extras: ExtrasConfig,
    pub cleanup: CleanupConfig,
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
pub struct DvdReadConfig {
    pub device_path: PathBuf,
}

impl Default for DvdReadConfig {
    fn default() -> Self {
        Self {
            device_path: PathBuf::from("/dev/sr0"),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    pub log_file: Option<PathBuf>,
    pub log_dir: Option<PathBuf>,
    pub show_progress: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_file: None,
            log_dir: None,
            show_progress: true,
        }
    }
}

impl LoggingConfig {
    /// Returns the log file path for this run, or `None` if output goes to stdout.
    /// `title` is used as the filename stem when `log_dir` is set.
    pub fn get_encode_log_file(&self, title: &str, stem: &str) -> Option<PathBuf> {
        if let Some(dir) = &self.log_dir {
            let ts = self.get_timestamp();
            let folder = dir.join(title);
            return Some(dir.join(folder).join(format!("{stem}_{ts}.log")));
        }
        self.log_file.clone()
    }

    pub fn get_rip_log_file(&self, title: &str) -> Option<PathBuf> {
        if let Some(dir) = &self.log_dir {
            let ts = self.get_timestamp();
            return Some(dir.join(format!("{title}_{ts}.log")));
        }
        self.log_file.clone()
    }

    fn get_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
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

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct CleanupConfig {
    pub delete_rips: bool,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self { delete_rips: true }
    }
}

impl Config {
    pub fn load(cli: &Cli) -> Result<Self> {
        let mut cfg = match &cli.config {
            Some(p) => toml::from_str(&fs::read_to_string(p)?)?,
            _ => Self::default(),
        };

        match &cli.command {
            Some(Command::Run(args)) => cfg.apply_run_overrides(args),
            Some(Command::Encode(args)) => cfg.apply_encode_overrides(args),
            Some(Command::Upload(args)) => cfg.apply_upload_overrides(args),
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
        apply_override(&args.mkv_log_file, |v| {
            self.makemkv.logging.log_file = Some(v)
        });
        apply_override(&args.hb_log_file, |v| {
            self.handbrake.logging.log_file = Some(v)
        });
        apply_override(&args.delete_rips, |v| self.cleanup.delete_rips = v);
    }

    fn apply_encode_overrides(&mut self, args: &EncodeArgs) {
        apply_override(&args.preset, |v| self.handbrake.preset = v);
        apply_override(&args.preset_file, |v| self.handbrake.preset_file = Some(v));
        apply_override(&args.hb_args, |v| {
            self.handbrake.extra_args = v.split_whitespace().map(String::from).collect();
        });
        apply_override(&args.output_format, |v| self.handbrake.output_format = v);
        apply_override(&args.hb_log_file, |v| {
            self.handbrake.logging.log_file = Some(v)
        });
        if args.no_upload {
            self.upload.no_upload = true;
        }
        apply_override(&args.delete_rips, |v| self.cleanup.delete_rips = v);
    }

    fn apply_upload_overrides(&mut self, args: &UploadArgs) {
        apply_override(&args.delete_rips, |v| self.cleanup.delete_rips = v);
    }
}

fn apply_override<T: Clone>(value: &Option<T>, f: impl FnOnce(T)) {
    if let Some(v) = value {
        f(v.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{self, Command};
    use crate::types::{ExtrasMode, OutputFormat};

    // Helpers to build Cli structs without going through argument parsing.

    fn cli_no_command() -> Cli {
        Cli {
            config: None,
            command: None,
        }
    }

    fn cli_run(args: cli::RunArgs) -> Cli {
        Cli {
            config: None,
            command: Some(Command::Run(args)),
        }
    }

    fn cli_encode(args: cli::EncodeArgs) -> Cli {
        Cli {
            config: None,
            command: Some(Command::Encode(args)),
        }
    }

    fn bare_run_args() -> cli::RunArgs {
        cli::RunArgs {
            output_dir: None,
            preset: None,
            preset_file: None,
            no_upload: false,
            disc_device: None,
            mkv_args: None,
            hb_args: None,
            output_format: None,
            mkv_log_file: None,
            hb_log_file: None,
            delete_rips: None,
        }
    }

    fn bare_encode_args() -> cli::EncodeArgs {
        cli::EncodeArgs {
            input: "/tmp/test.mkv".into(),
            output: None,
            title: None,
            preset: None,
            preset_file: None,
            no_upload: false,
            hb_args: None,
            output_format: None,
            hb_log_file: None,
            delete_rips: None,
        }
    }

    fn bare_upload_args() -> cli::UploadArgs {
        cli::UploadArgs {
            input: "/tmp/test.mkv".into(),
            title: None,
            extra: false,
            delete_rips: None,
        }
    }

    fn cli_upload(args: cli::UploadArgs) -> Cli {
        Cli {
            config: None,
            command: Some(Command::Upload(args)),
        }
    }

    // --- Default values ---

    #[test]
    fn default_config_values() {
        let cfg = Config::default();
        assert_eq!(cfg.makemkv.min_length_seconds, 120);
        assert_eq!(cfg.makemkv.disc_device, "disc:0");
        assert!(cfg.makemkv.extra_args.is_empty());
        assert_eq!(cfg.handbrake.preset, "H.265 MKV 1080p30");
        assert_eq!(cfg.handbrake.output_format, OutputFormat::Mkv);
        assert!(!cfg.upload.no_upload);
        assert!(!cfg.upload.direct_to_nas);
        assert_eq!(cfg.extras.mode, ExtrasMode::Skip);
        assert!(!cfg.extras.encode);
        assert!(!cfg.extras.upload);
        assert_eq!(cfg.extras.folder_name, "extras");
        assert!(cfg.makemkv.logging.show_progress);
        assert!(cfg.handbrake.logging.show_progress);
        assert!(cfg.makemkv.logging.log_file.is_none());
        assert!(cfg.makemkv.logging.log_dir.is_none());
        assert!(cfg.handbrake.logging.log_dir.is_none());
        assert!(cfg.paths.nas_mount.is_none());
        assert!(cfg.cleanup.delete_rips);
    }

    // --- TOML loading ---

    #[test]
    fn partial_toml_fills_missing_fields_with_defaults() {
        let cfg: Config = toml::from_str(
            r#"
            [handbrake]
            preset = "Fast 1080p30"
        "#,
        )
        .unwrap();
        assert_eq!(cfg.handbrake.preset, "Fast 1080p30");
        assert_eq!(cfg.makemkv.min_length_seconds, 120);
        assert_eq!(cfg.makemkv.disc_device, "disc:0");
    }

    #[test]
    fn toml_loads_output_format_enum() {
        let cfg: Config = toml::from_str(
            r#"
            [handbrake]
            output_format = "mp4"
        "#,
        )
        .unwrap();
        assert_eq!(cfg.handbrake.output_format, OutputFormat::Mp4);
    }

    #[test]
    fn toml_loads_extras_mode_keep() {
        let cfg: Config = toml::from_str(
            r#"
            [extras]
            mode = "keep"
            encode = true
            upload = true
        "#,
        )
        .unwrap();
        assert_eq!(cfg.extras.mode, ExtrasMode::Keep);
        assert!(cfg.extras.encode);
        assert!(cfg.extras.upload);
    }

    #[test]
    fn extras_mode_ask_round_trips_through_toml() {
        let mut original = Config::default();
        original.extras.mode = ExtrasMode::Ask;
        let serialised = toml::to_string_pretty(&original).unwrap();
        let reloaded: Config = toml::from_str(&serialised).unwrap();
        assert_eq!(reloaded.extras.mode, ExtrasMode::Ask);
    }

    #[test]
    fn toml_loads_extras_mode_ask() {
        let cfg: Config = toml::from_str(
            r#"
            [extras]
            mode = "ask"
            encode = true
            upload = true
        "#,
        )
        .unwrap();
        assert_eq!(cfg.extras.mode, ExtrasMode::Ask);
        assert!(cfg.extras.encode);
        assert!(cfg.extras.upload);
    }

    #[test]
    fn toml_loads_nested_logging_section() {
        let cfg: Config = toml::from_str(
            r#"
            [makemkv.logging]
            show_progress = false
            log_file = "/tmp/makemkv.log"
        "#,
        )
        .unwrap();
        assert!(!cfg.makemkv.logging.show_progress);
        assert_eq!(
            cfg.makemkv.logging.log_file,
            Some(std::path::PathBuf::from("/tmp/makemkv.log"))
        );
        // Unrelated sections should still be at defaults.
        assert!(cfg.handbrake.logging.show_progress);
    }

    #[test]
    fn default_config_round_trips_through_toml() {
        let original = Config::default();
        let serialised = toml::to_string_pretty(&original).unwrap();
        let reloaded: Config = toml::from_str(&serialised).unwrap();
        assert_eq!(reloaded.handbrake.preset, original.handbrake.preset);
        assert_eq!(
            reloaded.makemkv.min_length_seconds,
            original.makemkv.min_length_seconds
        );
        assert_eq!(reloaded.extras.mode, original.extras.mode);
        assert_eq!(
            reloaded.handbrake.output_format,
            original.handbrake.output_format
        );
    }

    // --- CLI run overrides ---

    #[test]
    fn no_cli_args_leaves_defaults_intact() {
        let cfg = Config::load(&cli_no_command()).unwrap();
        assert_eq!(cfg.handbrake.preset, "H.265 MKV 1080p30");
        assert_eq!(cfg.makemkv.disc_device, "disc:0");
    }

    #[test]
    fn run_preset_overrides_config() {
        let mut args = bare_run_args();
        args.preset = Some("Custom Preset".to_string());
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert_eq!(cfg.handbrake.preset, "Custom Preset");
    }

    #[test]
    fn run_no_upload_overrides_config() {
        let mut args = bare_run_args();
        args.no_upload = true;
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert!(cfg.upload.no_upload);
    }

    #[test]
    fn run_disc_device_overrides_config() {
        let mut args = bare_run_args();
        args.disc_device = Some("disc:1".to_string());
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert_eq!(cfg.makemkv.disc_device, "disc:1");
    }

    #[test]
    fn run_mkv_args_splits_on_whitespace() {
        let mut args = bare_run_args();
        args.mkv_args = Some("--noscan --foo bar".to_string());
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert_eq!(cfg.makemkv.extra_args, vec!["--noscan", "--foo", "bar"]);
    }

    #[test]
    fn run_hb_args_splits_on_whitespace() {
        let mut args = bare_run_args();
        args.hb_args = Some("--encoder nvenc_h265".to_string());
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert_eq!(cfg.handbrake.extra_args, vec!["--encoder", "nvenc_h265"]);
    }

    #[test]
    fn run_output_format_overrides_config() {
        let mut args = bare_run_args();
        args.output_format = Some(OutputFormat::Mp4);
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert_eq!(cfg.handbrake.output_format, OutputFormat::Mp4);
    }

    // --- CLI encode overrides ---

    #[test]
    fn encode_preset_overrides_config() {
        let mut args = bare_encode_args();
        args.preset = Some("H.264 MKV 1080p30".to_string());
        let cfg = Config::load(&cli_encode(args)).unwrap();
        assert_eq!(cfg.handbrake.preset, "H.264 MKV 1080p30");
    }

    #[test]
    fn encode_no_upload_overrides_config() {
        let mut args = bare_encode_args();
        args.no_upload = true;
        let cfg = Config::load(&cli_encode(args)).unwrap();
        assert!(cfg.upload.no_upload);
    }

    #[test]
    fn encode_hb_args_splits_on_whitespace() {
        let mut args = bare_encode_args();
        args.hb_args = Some("--encoder nvenc_h265 --vb 8000".to_string());
        let cfg = Config::load(&cli_encode(args)).unwrap();
        assert_eq!(
            cfg.handbrake.extra_args,
            vec!["--encoder", "nvenc_h265", "--vb", "8000"]
        );
    }

    #[test]
    fn encode_output_format_overrides_config() {
        let mut args = bare_encode_args();
        args.output_format = Some(OutputFormat::M4v);
        let cfg = Config::load(&cli_encode(args)).unwrap();
        assert_eq!(cfg.handbrake.output_format, OutputFormat::M4v);
    }

    // --- get_encode_log_file ---

    #[test]
    fn get_encode_log_file_none_when_neither_set() {
        let log = LoggingConfig::default();
        assert!(log.get_encode_log_file("The Matrix", "A1_t00").is_none());
    }

    #[test]
    fn get_encode_log_file_returns_log_file_path() {
        let log = LoggingConfig {
            log_file: Some("/tmp/out.log".into()),
            ..Default::default()
        };
        assert_eq!(
            log.get_encode_log_file("The Matrix", "A1_t00"),
            Some(PathBuf::from("/tmp/out.log"))
        );
    }

    #[test]
    fn get_encode_log_file_dir_takes_precedence_over_file() {
        let log = LoggingConfig {
            log_file: Some("/tmp/out.log".into()),
            log_dir: Some("/var/log/ripcord".into()),
            ..Default::default()
        };
        let result = log.get_encode_log_file("The_Matrix", "A1_t00").unwrap();
        assert_eq!(
            result.parent().unwrap(),
            std::path::Path::new("/var/log/ripcord/The_Matrix")
        );
        assert!(
            result
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("A1_t00_")
        );
        assert!(result.extension().unwrap() == "log");
    }

    #[test]
    fn get_encode_log_file_dir_embeds_title_in_filename() {
        let log = LoggingConfig {
            log_dir: Some("/tmp/logs".into()),
            ..Default::default()
        };
        let result = log.get_encode_log_file("Inception", "A1_t00").unwrap();
        let name = result.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("A1_t00_"),
            "expected 'A1_t00_…', got '{name}'"
        );
    }

    // --- get_rip_log_file ---

    #[test]
    fn get_rip_log_file_none_when_neither_set() {
        let log = LoggingConfig::default();
        assert!(log.get_rip_log_file("The Matrix").is_none());
    }

    #[test]
    fn get_rip_log_file_returns_log_file_path() {
        let log = LoggingConfig {
            log_file: Some("/tmp/out.log".into()),
            ..Default::default()
        };
        assert_eq!(
            log.get_rip_log_file("The Matrix"),
            Some(PathBuf::from("/tmp/out.log"))
        );
    }

    #[test]
    fn get_rip_log_file_dir_takes_precedence_over_file() {
        let log = LoggingConfig {
            log_file: Some("/tmp/out.log".into()),
            log_dir: Some("/var/log/ripcord".into()),
            ..Default::default()
        };
        let result = log.get_rip_log_file("The_Matrix").unwrap();
        assert_eq!(
            result.parent().unwrap(),
            std::path::Path::new("/var/log/ripcord")
        );
        assert!(
            result
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("The_Matrix_")
        );
        assert!(result.extension().unwrap() == "log");
    }

    #[test]
    fn get_rip_log_file_dir_embeds_title_in_filename() {
        let log = LoggingConfig {
            log_dir: Some("/tmp/logs".into()),
            ..Default::default()
        };
        let result = log.get_rip_log_file("Inception").unwrap();
        let name = result.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with("Inception_"),
            "expected 'Inception_…', got '{name}'"
        );
    }

    #[test]
    fn cleanup_delete_rips_defaults_to_true() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.cleanup.delete_rips);
    }

    #[test]
    fn cleanup_delete_rips_can_be_disabled() {
        let cfg: Config = toml::from_str(
            r#"
            [cleanup]
            delete_rips = false
        "#,
        )
        .unwrap();
        assert!(!cfg.cleanup.delete_rips);
    }

    #[test]
    fn toml_loads_log_dir() {
        let cfg: Config = toml::from_str(
            r#"
            [makemkv.logging]
            log_dir = "/var/log/ripcord"
        "#,
        )
        .unwrap();
        assert_eq!(
            cfg.makemkv.logging.log_dir,
            Some(PathBuf::from("/var/log/ripcord"))
        );
        assert!(cfg.makemkv.logging.log_file.is_none());
    }

    // --- CLI log file overrides ---

    #[test]
    fn run_mkv_log_file_overrides_config() {
        let mut args = bare_run_args();
        args.mkv_log_file = Some("/tmp/makemkv.log".into());
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert_eq!(
            cfg.makemkv.logging.log_file,
            Some(PathBuf::from("/tmp/makemkv.log"))
        );
    }

    #[test]
    fn run_hb_log_file_overrides_config() {
        let mut args = bare_run_args();
        args.hb_log_file = Some("/tmp/handbrake.log".into());
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert_eq!(
            cfg.handbrake.logging.log_file,
            Some(PathBuf::from("/tmp/handbrake.log"))
        );
    }

    #[test]
    fn encode_hb_log_file_overrides_config() {
        let mut args = bare_encode_args();
        args.hb_log_file = Some("/tmp/handbrake.log".into());
        let cfg = Config::load(&cli_encode(args)).unwrap();
        assert_eq!(
            cfg.handbrake.logging.log_file,
            Some(PathBuf::from("/tmp/handbrake.log"))
        );
    }

    // --- CLI delete_rips overrides ---

    #[test]
    fn run_delete_rips_false_disables_cleanup() {
        let mut args = bare_run_args();
        args.delete_rips = Some(false);
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert!(!cfg.cleanup.delete_rips);
    }

    #[test]
    fn run_delete_rips_true_enables_cleanup() {
        let mut args = bare_run_args();
        args.delete_rips = Some(true);
        let cfg = Config::load(&cli_run(args)).unwrap();
        assert!(cfg.cleanup.delete_rips);
    }

    #[test]
    fn run_delete_rips_none_leaves_config_unchanged() {
        // No CLI flag — config default (true) should be respected.
        let cfg = Config::load(&cli_run(bare_run_args())).unwrap();
        assert!(cfg.cleanup.delete_rips);
    }

    #[test]
    fn encode_delete_rips_false_disables_cleanup() {
        let mut args = bare_encode_args();
        args.delete_rips = Some(false);
        let cfg = Config::load(&cli_encode(args)).unwrap();
        assert!(!cfg.cleanup.delete_rips);
    }

    #[test]
    fn encode_delete_rips_true_enables_cleanup() {
        let mut args = bare_encode_args();
        args.delete_rips = Some(true);
        let cfg = Config::load(&cli_encode(args)).unwrap();
        assert!(cfg.cleanup.delete_rips);
    }

    #[test]
    fn upload_delete_rips_false_disables_cleanup() {
        let mut args = bare_upload_args();
        args.delete_rips = Some(false);
        let cfg = Config::load(&cli_upload(args)).unwrap();
        assert!(!cfg.cleanup.delete_rips);
    }

    #[test]
    fn upload_delete_rips_true_enables_cleanup() {
        let mut args = bare_upload_args();
        args.delete_rips = Some(true);
        let cfg = Config::load(&cli_upload(args)).unwrap();
        assert!(cfg.cleanup.delete_rips);
    }
}
