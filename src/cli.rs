use std::path::PathBuf;

use clap::{Parser, Subcommand, arg, command};

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    /// Path to the configuration file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
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
    #[arg(long)]
    pub preset: Option<String>,
    #[arg(long)]
    pub preset_file: Option<PathBuf>,
    #[arg(long)]
    pub no_upload: Option<bool>,
}

#[derive(Parser, Debug)]
pub struct EncodeArgs {
    /// Path to the MKV file to encode
    pub input: PathBuf,
    #[arg(long)]
    pub preset: Option<String>,
    #[arg(long)]
    pub preset_file: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct UploadArgs {
    /// Path to the file to upload
    pub input: PathBuf,
}
