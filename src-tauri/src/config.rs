use serde::{Deserialize, Serialize};
use snafu::prelude::*;
use std::fs::File;
use std::io::{BufReader, Write};
use word_index::CommandError;

const DB: &str = "word-index.db";

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let file = File::open(DB);
        match file {
            Ok(f) => {
                let reader = BufReader::new(f);
                serde_json::from_reader(reader).context(DecodeConfigFile { path: DB })
            }
            Err(_) => Ok(Self::default()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(DB)
            .context(ReadConfigFile { path: DB })?;
        file.write_all(
            serde_json::to_string(self)
                .context(EncodeConfig)?
                .as_bytes(),
        )
        .context(SaveConfigFile { path: DB })?;
        Ok(())
    }
}

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Snafu, CommandError)]
pub enum Error {
    #[snafu(display("无法解编配置文件：{path}"), context(suffix(false)))]
    DecodeConfigFile {
        source: serde_json::error::Error,
        path: String,
    },
    #[snafu(display("无法编码配置"), context(suffix(false)))]
    EncodeConfig { source: serde_json::error::Error },
    #[snafu(display("无法读取配置文件：{path}"), context(suffix(false)))]
    ReadConfigFile {
        source: std::io::Error,
        path: String,
    },
    #[snafu(display("无法保存配置文件：{path}"), context(suffix(false)))]
    SaveConfigFile {
        source: std::io::Error,
        path: String,
    },
}
