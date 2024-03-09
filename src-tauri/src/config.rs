use serde::{Deserialize, Serialize};
use snafu::prelude::*;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};
use word_index::CommandError;

const DB: &str = "word-index.db";

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Config {
    pub async fn load() -> Result<Self> {
        if let Ok(mut file) = File::open(DB).await {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .await
                .context(ReadConfigFile { path: DB })?;
            serde_json::from_slice(&buf).context(DecodeConfigFile { path: DB })
        } else {
            Ok(Self::default())
        }
    }

    pub async fn save(&self) -> Result<()> {
        let file = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(DB)
            .context(ReadConfigFile { path: DB })?;
        let mut file = File::from_std(file);
        file.write_all(
            serde_json::to_string(self)
                .context(EncodeConfig)?
                .as_bytes(),
        )
        .await
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
