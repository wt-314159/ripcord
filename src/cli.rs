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
    Run {
        /// Output directory for ripped files (before encoding)
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        preset_file: Option<PathBuf>,
        #[arg(long)]
        no_upload: bool,
    },
    /// Encode a single existing MKV file
    Encode {
        /// Path to the MKV file to encode
        input: PathBuf,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        preset_file: Option<PathBuf>,
    },
    /// Upload a file to the NAS without encoding
    Upload { input: PathBuf },
    /// Generate a default config file
    InitConfig {
        /// Path to save the config file
        #[arg(default_value = "config.toml")]
        path: PathBuf,
    },
}
