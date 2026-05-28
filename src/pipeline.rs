use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use crate::{config::Config, encoder, ripper, types::ExtrasMode, uploader};
use anyhow::Result;

pub struct EncodeJob {
    pub title: String,
    pub mkv_path: PathBuf,
    pub is_extra: bool,
    /// false means skip encoding and upload the raw MKV directly (extras only).
    pub encode: bool,
}

pub fn run_loop(cfg: &Config) -> Result<()> {
    if !cfg.upload.no_upload {
        uploader::check_nas_accessible(cfg)?;
    }

    let (tx, rx) = mpsc::channel::<EncodeJob>();

    let cfg_clone = cfg.clone();
    let worker = thread::spawn(move || {
        encode_upload_worker(rx, &cfg_clone);
    });

    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let mut input = String::new();

    loop {
        print!("Enter movie title (or press Enter to quit): ");
        io::stdout().flush()?;

        input.clear();
        stdin_lock.read_line(&mut input)?;
        let title = input.trim().to_string();

        if title.is_empty() {
            break;
        }

        println!("Ripping '{title}'...");

        let min_length = resolve_min_length(&title, cfg);

        match ripper::rip_disc(&title, min_length, cfg) {
            Err(e) => eprintln!("Rip failed for '{title}': {e}"),
            Ok(main_feature) => {
                println!(
                    "Rip complete — encoding is happening in the background, you can swap the disc."
                );

                tx.send(EncodeJob {
                    title: title.clone(),
                    mkv_path: main_feature.clone(),
                    is_extra: false,
                    encode: true,
                })
                .expect("encode/upload worker stopped unexpectedly");

                if should_queue_extras(cfg) {
                    if let Some(parent) = main_feature.parent() {
                        queue_extras(parent, &main_feature, &title, &tx, cfg);
                    }
                }
            }
        }
    }

    // Dropping tx closes the channel, which causes the worker's `for job in rx` loop to exit.
    drop(tx);
    worker.join().expect("encode/upload worker panicked");
    println!("All done.");
    Ok(())
}

/// Determines the min-length to pass to makemkvcon.
/// In Skip mode: reads disc info and returns (longest title duration - 60s).
/// Falls back to the configured value if disc info fails or Skip mode is not active.
fn resolve_min_length(title: &str, cfg: &Config) -> u64 {
    if cfg.extras.mode == ExtrasMode::Skip {
        println!("Reading disc info...");
        match ripper::get_disc_info(cfg) {
            Ok(titles) => {
                if let Some(smart) = ripper::smart_min_length_seconds(&titles) {
                    println!(
                        "Found {} title(s) on disc; using smart min-length of {smart}s.",
                        titles.len()
                    );
                    return smart;
                }
            }
            Err(e) => eprintln!(
                "Warning: could not read disc info for '{title}': {e}. Using configured min-length."
            ),
        }
    }
    cfg.makemkv.min_length_seconds
}

/// Returns true when extras should be identified and queued after ripping.
fn should_queue_extras(cfg: &Config) -> bool {
    cfg.extras.mode == ExtrasMode::Keep
        && (cfg.extras.encode || (cfg.extras.upload && !cfg.upload.no_upload))
}

fn queue_extras(
    output_dir: &Path,
    main_feature: &Path,
    title: &str,
    tx: &Sender<EncodeJob>,
    cfg: &Config,
) {
    match find_extras(output_dir, main_feature) {
        Err(e) => eprintln!("Warning: could not list extras for '{title}': {e}"),
        Ok(extras) if extras.is_empty() => {}
        Ok(extras) => {
            println!("Queuing {} extra(s) for '{title}'.", extras.len());
            for extra in extras {
                if let Err(e) = tx.send(EncodeJob {
                    title: title.to_string(),
                    mkv_path: extra,
                    is_extra: true,
                    encode: cfg.extras.encode,
                }) {
                    eprintln!("Warning: could not queue extra: {e}");
                    break;
                }
            }
        }
    }
}

fn encode_upload_worker(rx: Receiver<EncodeJob>, cfg: &Config) {
    for job in rx {
        let label = if job.is_extra {
            format!("'{}' (extra)", job.title)
        } else {
            format!("'{}'", job.title)
        };

        println!("[worker] Processing {label}...");

        if let Err(e) = process_job(&job, cfg) {
            eprintln!("[worker] Failed to process {label}: {e}");
        } else {
            println!("[worker] Finished {label}.");
        }
    }
}

fn process_job(job: &EncodeJob, cfg: &Config) -> Result<()> {
    let should_upload =
        !cfg.upload.no_upload && (!job.is_extra || cfg.extras.upload);
    // Direct mode: HandBrakeCLI writes the output straight to the NAS path.
    let use_direct = job.encode && should_upload && cfg.upload.direct_to_nas;

    let encoded_path = if job.encode {
        let output = if use_direct {
            uploader::prepare_encode_dest(&job.mkv_path, &job.title, job.is_extra, cfg)?
        } else {
            local_encode_path(&job.mkv_path, &job.title, job.is_extra, cfg)
        };
        encoder::encode(&job.mkv_path, &output, cfg)?;
        output
    } else {
        // No encoding requested — pass the raw MKV through to the upload step.
        job.mkv_path.clone()
    };

    if should_upload && !use_direct {
        let dest = uploader::upload_file(&encoded_path, &job.title, job.is_extra, cfg)?;
        println!("[worker] Uploaded: {}", dest.display());
    }

    Ok(())
}

/// Local path HandBrakeCLI writes to when not using direct-to-NAS mode.
/// Main feature: `<output_dir>/<SanitizedTitle>.<ext>`
/// Extras: same directory as the source MKV, stem unchanged, extension updated.
fn local_encode_path(mkv_path: &Path, title: &str, is_extra: bool, cfg: &Config) -> PathBuf {
    let ext = cfg.handbrake.output_format.extension();
    let parent = mkv_path.parent().unwrap_or(mkv_path);

    if is_extra {
        let stem = mkv_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from("extra"));
        parent.join(format!("{stem}.{ext}"))
    } else {
        let sanitized = ripper::sanitize_filename(title);
        parent.join(format!("{sanitized}.{ext}"))
    }
}

fn find_extras(output_dir: &Path, main_feature: &Path) -> Result<Vec<PathBuf>> {
    let extras = fs::read_dir(output_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|ext| ext.to_str()) == Some("mkv"))
        .filter(|p| p != main_feature)
        .collect();
    Ok(extras)
}
