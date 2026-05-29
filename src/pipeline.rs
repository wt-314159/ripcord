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

pub fn run_loop(cfg: &mut Config) -> Result<()> {
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
    let mut keep_extras = String::new();
    let ask_extras = cfg.extras.mode == ExtrasMode::Ask;

    loop {
        print!("Enter movie title (or press Enter to quit): ");
        io::stdout().flush()?;

        input.clear();
        stdin_lock.read_line(&mut input)?;
        let title = input.trim().to_string();

        if title.is_empty() {
            break;
        }

        if ask_extras {
            keep_extras.clear();
            print!("Keep extras? (y/N): ");
            io::stdout().flush()?;
            stdin_lock.read_line(&mut keep_extras)?;
            cfg.extras.mode = if keep_extras.trim().eq_ignore_ascii_case("y") {
                ExtrasMode::Keep
            } else {
                ExtrasMode::Skip
            };
        }

        match cfg.extras.mode {
            ExtrasMode::Keep => println!("Extras will be kept."),
            ExtrasMode::Skip => println!("Extras will be skipped."),
            ExtrasMode::Ask => unreachable!("Ask mode is resolved to Keep/Skip before this point"),
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
    let should_upload = !cfg.upload.no_upload && (!job.is_extra || cfg.extras.upload);
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

    if cfg.cleanup.delete_rips && (job.encode || should_upload) {
        delete_rip_file(&job.mkv_path);
    }

    Ok(())
}

pub(crate) fn delete_rip_file(mkv_path: &Path) {
    match fs::remove_file(mkv_path) {
        Err(e) => eprintln!(
            "[worker] Warning: could not delete rip '{}': {e}",
            mkv_path.display()
        ),
        Ok(()) => {
            println!("[worker] Deleted rip: {}", mkv_path.display());
            if let Some(parent) = mkv_path.parent() {
                // Best-effort: remove the directory only if it is now empty.
                let _ = fs::remove_dir(parent);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ExtrasConfig, UploadConfig};
    use crate::types::{ExtrasMode, OutputFormat};

    fn cfg_with_extras(mode: ExtrasMode, encode: bool, upload: bool, no_upload: bool) -> Config {
        let mut cfg = Config::default();
        cfg.extras = ExtrasConfig {
            mode,
            encode,
            upload,
            ..Default::default()
        };
        cfg.upload = UploadConfig {
            no_upload,
            ..Default::default()
        };
        cfg
    }

    // --- should_queue_extras ---

    #[test]
    fn should_queue_extras_keep_with_encode() {
        let cfg = cfg_with_extras(ExtrasMode::Keep, true, false, false);
        assert!(should_queue_extras(&cfg));
    }

    #[test]
    fn should_queue_extras_keep_with_upload() {
        let cfg = cfg_with_extras(ExtrasMode::Keep, false, true, false);
        assert!(should_queue_extras(&cfg));
    }

    #[test]
    fn should_queue_extras_keep_upload_suppressed_by_no_upload() {
        let cfg = cfg_with_extras(ExtrasMode::Keep, false, true, true);
        assert!(!should_queue_extras(&cfg));
    }

    #[test]
    fn should_queue_extras_keep_neither_encode_nor_upload() {
        let cfg = cfg_with_extras(ExtrasMode::Keep, false, false, false);
        assert!(!should_queue_extras(&cfg));
    }

    #[test]
    fn should_queue_extras_skip_always_false() {
        let cfg = cfg_with_extras(ExtrasMode::Skip, true, true, false);
        assert!(!should_queue_extras(&cfg));
    }

    #[test]
    fn should_queue_extras_ask_always_false() {
        // Ask is resolved to Keep/Skip before should_queue_extras is ever called,
        // but if somehow called with Ask it should safely return false.
        let cfg = cfg_with_extras(ExtrasMode::Ask, true, true, false);
        assert!(!should_queue_extras(&cfg));
    }

    // --- local_encode_path ---

    fn cfg_with_format(fmt: OutputFormat) -> Config {
        let mut cfg = Config::default();
        cfg.handbrake.output_format = fmt;
        cfg
    }

    #[test]
    fn local_encode_path_main_feature_uses_sanitized_title() {
        let cfg = cfg_with_format(OutputFormat::Mkv);
        let mkv = std::path::Path::new("/tmp/rips/Batman__The_Dark_Knight/title_t00.mkv");
        let result = local_encode_path(mkv, "Batman: The Dark Knight", false, &cfg);
        assert_eq!(
            result,
            std::path::Path::new("/tmp/rips/Batman__The_Dark_Knight/Batman_ The Dark Knight.mkv")
        );
    }

    #[test]
    fn local_encode_path_main_feature_respects_output_format() {
        let cfg = cfg_with_format(OutputFormat::Mp4);
        let mkv = std::path::Path::new("/tmp/rips/The_Matrix/title_t00.mkv");
        let result = local_encode_path(mkv, "The Matrix", false, &cfg);
        // sanitize_filename preserves spaces, so "The Matrix" stays "The Matrix"
        assert_eq!(
            result,
            std::path::Path::new("/tmp/rips/The_Matrix/The Matrix.mp4")
        );
    }

    #[test]
    fn local_encode_path_extra_uses_source_stem() {
        let cfg = cfg_with_format(OutputFormat::Mkv);
        let mkv = std::path::Path::new("/tmp/rips/The_Matrix/title_t02.mkv");
        let result = local_encode_path(mkv, "The Matrix", true, &cfg);
        assert_eq!(
            result,
            std::path::Path::new("/tmp/rips/The_Matrix/title_t02.mkv")
        );
    }

    #[test]
    fn local_encode_path_extra_updates_extension() {
        let cfg = cfg_with_format(OutputFormat::Mp4);
        let mkv = std::path::Path::new("/tmp/rips/The_Matrix/title_t02.mkv");
        let result = local_encode_path(mkv, "The Matrix", true, &cfg);
        assert_eq!(
            result,
            std::path::Path::new("/tmp/rips/The_Matrix/title_t02.mp4")
        );
    }

    // --- delete_rip_file ---

    #[test]
    fn delete_rip_file_removes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mkv = dir.path().join("title_t00.mkv");
        fs::write(&mkv, b"data").unwrap();

        delete_rip_file(&mkv);
        assert!(!mkv.exists());
    }

    #[test]
    fn delete_rip_file_removes_parent_dir_when_empty() {
        let root = tempfile::tempdir().unwrap();
        let sub = root.path().join("The Matrix");
        fs::create_dir(&sub).unwrap();
        let mkv = sub.join("title_t00.mkv");
        fs::write(&mkv, b"data").unwrap();

        delete_rip_file(&mkv);
        assert!(!mkv.exists());
        assert!(!sub.exists(), "empty rip directory should be removed");
    }

    #[test]
    fn delete_rip_file_leaves_parent_dir_when_not_empty() {
        let root = tempfile::tempdir().unwrap();
        let sub = root.path().join("The Matrix");
        fs::create_dir(&sub).unwrap();
        let mkv = sub.join("title_t00.mkv");
        let extra = sub.join("title_t01.mkv");
        fs::write(&mkv, b"main").unwrap();
        fs::write(&extra, b"extra").unwrap();

        delete_rip_file(&mkv);
        assert!(!mkv.exists());
        assert!(sub.exists(), "non-empty rip directory should remain");
        assert!(extra.exists());
    }

    // --- find_extras ---

    #[test]
    fn find_extras_excludes_main_feature() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("title_t00.mkv");
        fs::write(&main, b"main").unwrap();
        fs::write(dir.path().join("title_t01.mkv"), b"extra1").unwrap();
        fs::write(dir.path().join("title_t02.mkv"), b"extra2").unwrap();

        let mut extras = find_extras(dir.path(), &main).unwrap();
        extras.sort();
        // Returns full paths, not just filenames.
        assert_eq!(extras.len(), 2);
        assert!(extras.contains(&dir.path().join("title_t01.mkv")));
        assert!(extras.contains(&dir.path().join("title_t02.mkv")));
        assert!(!extras.contains(&main));
    }

    #[test]
    fn find_extras_returns_empty_when_only_main() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("title_t00.mkv");
        fs::write(&main, b"main").unwrap();

        let extras = find_extras(dir.path(), &main).unwrap();
        assert!(extras.is_empty());
    }

    #[test]
    fn find_extras_ignores_non_mkv_files() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("title_t00.mkv");
        fs::write(&main, b"main").unwrap();
        fs::write(dir.path().join("notes.txt"), b"text").unwrap();
        fs::write(dir.path().join("thumb.jpg"), b"img").unwrap();

        let extras = find_extras(dir.path(), &main).unwrap();
        assert!(extras.is_empty());
    }
}
