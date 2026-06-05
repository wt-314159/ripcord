use std::{fs, path::Path, sync::Arc};

use anyhow::Result;
use clap::Parser;

use crate::{config::Config, ui::Ui};

pub mod cli;
pub mod config;
pub mod encoder;
pub mod pipeline;
pub mod ripper;
pub mod types;
pub mod ui;
pub mod uploader;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let mut cfg = config::Config::load(&cli)?;

    match &cli.command {
        Some(cli::Command::Run(_)) => pipeline::run_loop(&mut cfg)?,
        Some(cli::Command::Encode(args)) => handle_encode(args, &cfg)?,
        Some(cli::Command::Upload(args)) => handle_upload(args, &cfg)?,
        Some(cli::Command::InitConfig { path }) => handle_init_config(path)?,
        None => eprintln!("No command specified. Use --help for usage."),
    }

    Ok(())
}

fn handle_encode(args: &cli::EncodeArgs, cfg: &config::Config) -> Result<()> {
    let ui = Arc::new(Ui::new());

    if args.input.is_dir() {
        let files = std::fs::read_dir(&args.input)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|ext| ext.to_str()) == Some("mkv"));

        for file in files {
            let title = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            let ext = cfg.handbrake.output_format.extension();
            let output_dir = args.output.clone().unwrap_or(args.input.clone());
            let output = output_dir.join(format!("{}.{ext}", title));

            encode_file(&file, &output, cfg, &title, &ui)?;
        }
        Ok(())
    } else {
        let title = args
            .title
            .clone()
            .or_else(|| stem_str(&args.input))
            .unwrap_or_else(|| String::from("unknown"));

        let output = args.output.clone().unwrap_or_else(|| {
            // TODO check that having same name for output as input doesn't cause issues in handbrake
            let ext = cfg.handbrake.output_format.extension();
            let stem = args.input.file_stem().unwrap_or_default();
            args.input
                .parent()
                .unwrap_or(Path::new("."))
                .join(format!("{}.{ext}", stem.to_string_lossy()))
        });

        encode_file(&args.input, &output, cfg, &title, &ui)
    }
}

fn encode_file(input: &Path, output: &Path, cfg: &Config, title: &str, ui: &Arc<Ui>) -> Result<()> {
    encoder::encode(input, output, cfg, title, ui)?;

    if !cfg.upload.no_upload {
        uploader::check_nas_accessible(cfg)?;
        let dest = uploader::upload_file(output, title, false, cfg)?;
        println!("Uploaded to: {}", dest.display());
    }

    if cfg.cleanup.delete_rips {
        pipeline::delete_rip_file(input);
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

    if cfg.cleanup.delete_rips {
        pipeline::delete_rip_file(&args.input);
    }

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
