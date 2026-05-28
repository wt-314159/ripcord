# ripcord

A CLI tool for automating Blu-ray and DVD ripping. Insert a disc, enter the title, and ripcord handles the rest: it rips with [MakeMKV](https://www.makemkv.com/), encodes with [HandBrake](https://handbrake.fr/), and copies the result to a NAS.

The main `run` command loops continuously — as soon as ripping finishes and the disc can be swapped, encoding runs in the background so the two stages overlap. Alternatively, `encode` and `upload` are available as one-shot utilities for files you already have.

---

## Prerequisites

- **[`makemkvcon`](https://www.makemkv.com/)** — MakeMKV's command-line interface, must be on `PATH`
- **[`HandBrakeCLI`](https://handbrake.fr/downloads2.php)** — HandBrake's command-line interface, must be on `PATH`
- A NAS or other network share mounted as a local directory (only needed for uploads)

## Installation

```sh
cargo install --path .
```

Or build without installing:

```sh
cargo build --release
# binary at target/release/ripcord
```

---

## Quick start

```sh
# 1. Generate a config file
ripcord init-config ~/.config/ripcord/config.toml

# 2. Edit it — at minimum, set paths.nas_mount
$EDITOR ~/.config/ripcord/config.toml

# 3. Start ripping
ripcord --config ~/.config/ripcord/config.toml run
```

On each iteration you'll see:

```
Enter movie title (or press Enter to quit): The Matrix
Ripping 'The Matrix'...
Reading disc info...
Found 8 title(s) on disc; using smart min-length of 8340s.
Rip complete — encoding is happening in the background, you can swap the disc.
Enter movie title (or press Enter to quit): _
```

Press Enter with no title to finish. ripcord waits for the background worker to finish encoding and uploading before exiting.

---

## Commands

### `run`

The main ripping loop. Prompts for a movie title, rips the disc, then immediately frees the drive while encoding and uploading happen in a background thread.

```sh
ripcord run [OPTIONS]
```

| Flag | Description |
|---|---|
| `--disc-device <dev>` | Drive to rip from (default: `disc:0`) |
| `--preset <name>` | HandBrake preset name |
| `--preset-file <path>` | HandBrake preset export file |
| `--output-dir <path>` | Staging directory for ripped MKVs |
| `--output-format <fmt>` | Output container: `mkv`, `mp4`, `m4v` |
| `--mkv-args <args>` | Extra arguments for makemkvcon (space-separated) |
| `--hb-args <args>` | Extra arguments for HandBrakeCLI (space-separated) |
| `--mkv-log-file <path>` | Write makemkvcon output to this file for this run |
| `--hb-log-file <path>` | Write HandBrakeCLI output to this file for this run |
| `--no-upload` | Skip uploading; leave encoded files in `output_dir` |

### `encode`

Encode a single MKV file and optionally upload it.

```sh
ripcord encode [OPTIONS] <INPUT>
```

```sh
# Encode and upload, inferring the title from the filename
ripcord encode /tmp/rips/The_Matrix/The_Matrix.mkv

# Encode only, no upload, with an explicit title and output path
ripcord encode --no-upload --title "The Matrix" --output ~/The_Matrix/The_Matrix.mkv
```

| Flag | Description |
|---|---|
| `--title <title>` | Movie title for the NAS path (default: input filename stem) |
| `--output <path>` | Output path for the encoded file |
| `--preset <name>` | HandBrake preset name |
| `--preset-file <path>` | HandBrake preset export file |
| `--output-format <fmt>` | Output container: `mkv`, `mp4`, `m4v` |
| `--hb-args <args>` | Extra arguments for HandBrakeCLI (space-separated) |
| `--hb-log-file <path>` | Write HandBrakeCLI output to this file for this run |
| `--no-upload` | Skip uploading |

### `upload`

Upload a file to the NAS without encoding.

```sh
ripcord upload [OPTIONS] <INPUT>
```

```sh
# Upload as the main feature
ripcord upload --title "The Matrix" The_Matrix.mkv

# Upload as an extra (goes into <title>/extras/)
ripcord upload --extra --title "The Matrix" behind_the_scenes.mkv
```

### `init-config`

Write a default config file to disk. All values are set to their defaults; edit the file to taste. See [`config.example.toml`](config.example.toml) for the same file with explanatory comments.

```sh
ripcord init-config                                    # writes config.toml in the current directory
ripcord init-config ~/.config/ripcord/config.toml
```

---

## Configuration

Pass the config file with `--config` on any command, or bake it into a shell alias. Most config values can be overridden per-run with the flags listed above.

### Minimal config

The only value with no sensible default is `paths.nas_mount`. Everything else works out of the box.

```toml
[paths]
nas_mount = "/mnt/nas/Movies"
```

### Full reference

See [`config.example.toml`](config.example.toml) for every option with inline documentation.

### NAS path layout

Files land at:

```
<nas_mount>/
  <Title>/
    <Title>.<ext>          ← main feature
    extras/
      title_t01.<ext>      ← extras (when extras.mode = "keep")
      title_t02.<ext>
```

The title is the string you enter at the prompt (or pass with `--title`), with characters unsafe in filenames replaced by `_`.

### Key settings

**`makemkv.disc_device`** — which optical drive to use. `disc:0` is the first drive, `disc:1` the second, and so on.

**`handbrake.preset`** — any built-in HandBrake preset name. Run `HandBrakeCLI --preset-list` to see all options. To use a custom preset exported from the HandBrake GUI (File → Export Presets), point `handbrake.preset_file` at the `.json` file and set `handbrake.preset` to the preset name within it.

**`upload.direct_to_nas`** — when `true`, HandBrakeCLI writes output directly to the NAS path instead of encoding locally and then copying. Saves one full file copy, but a failed or interrupted encode leaves a partial file on the NAS. The safe default is `false`.

---

## Extras handling

Controlled by the `[extras]` section.

| `extras.mode` | Behaviour |
|---|---|
| `skip` (default) | Only the longest title is ripped. ripcord reads disc info first and sets makemkvcon's `--minlength` just below the main feature's duration, so shorter titles never touch disk. |
| `keep` | Every title above `makemkv.min_length_seconds` is ripped. The longest becomes the main feature; all others are extras. |
| `ask` | On each disc, ripcord asks the user whether to keep extras for that disc before ripping. Answer "y" to keep (behaves like "keep") or anything else to skip (behaves like "skip"). Useful when some discs have extras worth keeping and others don't. |

When `mode = "keep"` (or `ask` and the user answers "y"), two additional flags control what happens to extras:

```toml
[extras]
mode = "keep"
encode = true   # encode extras with the same HandBrake preset
upload = true   # upload extras to <title>/extras/ on the NAS
```

The subfolder name (`extras` by default) is configurable with `extras.folder_name`.

---

## Logging

Both makemkvcon and HandBrakeCLI have independent logging config under `[makemkv.logging]` and `[handbrake.logging]`.

**`show_progress`** — show a live progress indicator in the terminal (default: `true`). Non-progress output is unaffected by this setting.

**Non-progress output destination** — two mutually exclusive options; `log_dir` takes precedence if both are set:

```toml
# Append to a fixed file across all runs:
log_file = "/var/log/ripcord/handbrake.log"

# Write a new timestamped file per run, named <title>_<unix_timestamp>.log.
# The directory must already exist.
log_dir = "/var/log/ripcord"
```

If neither is set, output goes to stdout.

The `--mkv-log-file` and `--hb-log-file` flags override `log_file` for a single run without touching the config.

---

## Tips

### Hardware-accelerated encoding

Pass encoder flags via `handbrake.extra_args` in the config, or with `--hb-args` for a single run:

```toml
# NVIDIA GPU (NVENC) — in config
[handbrake]
extra_args = ["--encoder", "nvenc_h265"]
```

```sh
# Apple VideoToolbox — for one run only
ripcord run --hb-args "--encoder vt_h265"
```

### Multiple drives

Run two instances pointing at different drives, either with separate config files or just `--disc-device`:

```sh
ripcord --config config.toml run --disc-device disc:0 &
ripcord --config config.toml run --disc-device disc:1
```

### Encoding without a NAS

Set `upload.no_upload = true` (or pass `--no-upload`) to skip all uploads. Encoded files stay in `paths.output_dir`, and you can copy them manually or run `ripcord upload` later.
