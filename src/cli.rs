use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::types::OutputFormat;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    /// Path to the configuration file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the continuous ripping process
    Run(RunArgs),
    /// Encode a single existing MKV file
    Encode(EncodeArgs),
    /// Upload a file to the NAS without encoding
    Upload(UploadArgs),
    /// Generate a default config file
    InitConfig {
        /// Path to save the config file
        #[arg(default_value = "config.toml")]
        path: PathBuf,
    },
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Output directory for ripped files (before encoding)
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,
    /// HandBrakeCLI preset name to use
    #[arg(long)]
    pub preset: Option<String>,
    /// Path to a HandBrakeCLI preset file
    #[arg(long)]
    pub preset_file: Option<PathBuf>,
    /// Skip uploading to the NAS
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub no_upload: bool,
    /// DVD drive to rip from (e.g. disc:0, disc:1)
    #[arg(long)]
    pub disc_device: Option<String>,
    /// Extra arguments to pass to makemkvcon (space-separated)
    #[arg(long)]
    pub mkv_args: Option<String>,
    /// Extra arguments to pass to HandBrakeCLI (space-separated)
    #[arg(long)]
    pub hb_args: Option<String>,
    /// Output container format
    #[arg(long)]
    pub output_format: Option<OutputFormat>,
    /// Write makemkvcon output to this file for this run (overrides config)
    #[arg(long)]
    pub mkv_log_file: Option<PathBuf>,
    /// Write HandBrakeCLI output to this file for this run (overrides config)
    #[arg(long)]
    pub hb_log_file: Option<PathBuf>,
    /// Delete the original ripped MKV after successful encoding/upload (overrides config)
    #[arg(long)]
    pub delete_rips: Option<bool>,
}

#[derive(Parser, Debug)]
pub struct EncodeArgs {
    /// Path to the MKV file to encode
    pub input: PathBuf,
    /// Explicit output path for the encoded file (default: same directory as input)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Movie title used when uploading to the NAS (default: input filename stem)
    #[arg(long)]
    pub title: Option<String>,
    /// HandBrakeCLI preset name to use
    #[arg(long)]
    pub preset: Option<String>,
    /// Path to a HandBrakeCLI preset file
    #[arg(long)]
    pub preset_file: Option<PathBuf>,
    /// Skip uploading to the NAS
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub no_upload: bool,
    /// Extra arguments to pass to HandBrakeCLI (space-separated)
    #[arg(long)]
    pub hb_args: Option<String>,
    /// Output container format
    #[arg(long)]
    pub output_format: Option<OutputFormat>,
    /// Write HandBrakeCLI output to this file for this run (overrides config)
    #[arg(long)]
    pub hb_log_file: Option<PathBuf>,
    /// Delete the original MKV after successful encoding/upload (overrides config)
    #[arg(long)]
    pub delete_rips: Option<bool>,
}

#[derive(Parser, Debug)]
pub struct UploadArgs {
    /// Path to the file to upload
    pub input: PathBuf,
    /// Movie title used for the NAS path (default: input filename stem)
    #[arg(long)]
    pub title: Option<String>,
    /// Treat this file as an extra (uploads to <title>/extras/ instead of <title>/)
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub extra: bool,
    /// Delete the source file after successful upload (overrides config)
    #[arg(long)]
    pub delete_rips: Option<bool>,
}
