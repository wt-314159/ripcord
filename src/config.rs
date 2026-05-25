use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct Config {
    pub paths: PathsConfig,
    pub makemkv: MakeMkvConfig,
    pub encoding: EncodingConfig,
    pub upload: UploadConfig,
}

#[derive(Deserialize, Default)]
pub struct PathsConfig {
    pub output_dir: PathBuf,        // Fill with sensible default
    pub nas_mount: Option<PathBuf>, // Make sure user has set this
}

#[derive(Deserialize, Default)]
pub struct MakeMkvConfig {}

#[derive(Deserialize, Default)]
pub struct EncodingConfig {}

#[derive(Deserialize, Default)]
pub struct UploadConfig {}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let mut cfg = match path {
            Some(p) => toml::from_str(&fs::read_to_string(p)?)?,
            None => Self::default(),
        };

        todo!("Apply CLI overrides");
        Ok(cfg)
    }
}
