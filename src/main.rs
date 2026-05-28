use std::{fs, path::Path};

use anyhow::Result;
use clap::Parser;

pub mod cli;
pub mod config;
pub mod encoder;
pub mod pipeline;
pub mod ripper;
pub mod types;
pub mod uploader;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let cfg = config::Config::load(&cli)?;

    match &cli.command {
        Some(cli::Command::Run(_)) => pipeline::run_loop(&cfg)?,
        Some(cli::Command::Encode(args)) => handle_encode(args, &cfg)?,
        Some(cli::Command::Upload(args)) => handle_upload(args, &cfg)?,
        Some(cli::Command::InitConfig { path }) => handle_init_config(path)?,
        None => eprintln!("No command specified. Use --help for usage."),
    }

    Ok(())
}

fn handle_encode(args: &cli::EncodeArgs, cfg: &config::Config) -> Result<()> {
    let title = args
        .title
        .clone()
        .or_else(|| stem_str(&args.input))
        .unwrap_or_else(|| String::from("unknown"));

    let output = args.output.clone().unwrap_or_else(|| {
        let ext = cfg.handbrake.output_format.extension();
        let stem = args.input.file_stem().unwrap_or_default();
        args.input
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("{}.{ext}", stem.to_string_lossy()))
    });

    encoder::encode(&args.input, &output, cfg)?;

    if !cfg.upload.no_upload {
        uploader::check_nas_accessible(cfg)?;
        let dest = uploader::upload_file(&output, &title, false, cfg)?;
        println!("Uploaded to: {}", dest.display());
    }

    Ok(())
}

fn handle_upload(args: &cli::UploadArgs, cfg: &config::Config) -> Result<()> {
    let title = args
        .title
        .clone()
        .or_else(|| stem_str(&args.input))
        .unwrap_or_else(|| String::from("unknown"));

    uploader::check_nas_accessible(cfg)?;
    let dest = uploader::upload_file(&args.input, &title, args.extra, cfg)?;
    println!("Uploaded to: {}", dest.display());
    Ok(())
}

fn handle_init_config(path: &std::path::PathBuf) -> Result<()> {
    if path.exists() {
        eprintln!("Warning: '{}' already exists, overwriting.", path.display());
    }
    let toml = toml::to_string_pretty(&config::Config::default())?;
    fs::write(path, toml)?;
    println!("Default config written to '{}'.", path.display());
    Ok(())
}

/// Returns the file stem of `path` as an owned `String`, or `None` if it isn't valid UTF-8.
fn stem_str(path: &std::path::Path) -> Option<String> {
    path.file_stem()?.to_str().map(str::to_owned)
}
