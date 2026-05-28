use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::config::Config;
use crate::ripper::sanitize_filename;
use anyhow::Result;

/// Verifies the NAS mount point exists and is a directory.
/// Call this at startup when upload is enabled, before any ripping begins.
pub fn check_nas_accessible(cfg: &Config) -> Result<()> {
    let mount = nas_mount(cfg)?;
    if !mount.is_dir() {
        return Err(anyhow::anyhow!(
            "NAS mount point '{}' is not accessible — is the NAS mounted?",
            mount.display()
        ));
    }
    Ok(())
}

/// Copies `source` to the NAS under `<nas_mount>/<title>/` for a main feature,
/// or `<nas_mount>/<title>/<extras_folder>/` for extras.
/// Creates destination directories as needed.
/// Returns the destination path.
pub fn upload_file(source: &Path, title: &str, is_extra: bool, cfg: &Config) -> Result<PathBuf> {
    let filename = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("source path '{}' has no valid filename", source.display()))?;

    let dest = build_dest_path(title, filename, is_extra, cfg)?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(source, &dest)?;
    Ok(dest)
}

/// Returns the NAS path for `direct_to_nas` mode — the path HandBrakeCLI should write to
/// directly as its output file. Creates the destination directory so HandBrakeCLI can write
/// immediately without the directory missing.
/// For main features the output filename is `<SanitizedTitle>.<ext>`.
/// For extras it uses the source file's stem so each extra keeps a distinct name.
pub fn prepare_encode_dest(source: &Path, title: &str, is_extra: bool, cfg: &Config) -> Result<PathBuf> {
    let ext = cfg.handbrake.output_format.extension();
    let filename = if is_extra {
        let stem = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extra");
        format!("{stem}.{ext}")
    } else {
        let sanitized = sanitize_filename(title);
        format!("{sanitized}.{ext}")
    };

    let dest = build_dest_path(title, &filename, is_extra, cfg)?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    Ok(dest)
}

fn build_dest_path(title: &str, filename: &str, is_extra: bool, cfg: &Config) -> Result<PathBuf> {
    let mount = nas_mount(cfg)?;
    let sanitized_title = sanitize_filename(title);

    let path = if is_extra {
        mount
            .join(&sanitized_title)
            .join(&cfg.extras.folder_name)
            .join(filename)
    } else {
        mount.join(&sanitized_title).join(filename)
    };

    Ok(path)
}

fn nas_mount(cfg: &Config) -> Result<&Path> {
    cfg.paths
        .nas_mount
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("nas_mount is not configured in paths.nas_mount"))
}
