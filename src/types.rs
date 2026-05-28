use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, ValueEnum, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Mkv,
    Mp4,
    M4v,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Mkv => "mkv",
            OutputFormat::Mp4 => "mp4",
            OutputFormat::M4v => "m4v",
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExtrasMode {
    #[default]
    Skip,
    Keep,
}
