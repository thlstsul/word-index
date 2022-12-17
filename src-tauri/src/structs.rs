use std::{ffi::OsStr, path::PathBuf, time::SystemTime};

use async_walkdir::DirEntry;
use encoding::all::GBK;
use encoding::{DecoderTrap, Encoding};
use serde::{Deserialize, Serialize};
use snafu::prelude::*;
use tokio::{fs::File, io::AsyncReadExt, process::Command};
use tracing::{error, info, instrument};
use word_index::CommandError;

const PLAIN_FILE_TYPE: [&str; 2] = ["txt", "sql"];
const HYPER_FILE_TYPE: [&str; 2] = ["docx", "md"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Docx {
    id: String,
    name: String,
    path: String,
    content: String,
    timestamp: u64,
}

impl Docx {
    pub async fn new(dir_entry: &DirEntry) -> Result<Docx> {
        let path = dir_entry.path();

        ensure!(
            is_support(dir_entry).await,
            UnsupportedDocument {
                path: path.to_str().unwrap().to_string()
            }
        );

        let name = path.file_name().and_then(|s| s.to_str()).unwrap();
        let path_name = path.to_str().unwrap();
        let timestamp = get_file_timestamp(dir_entry).await?;
        let md5 = md5::compute(path_name.as_bytes());
        let id = format!("{:x}", md5);
        Ok(Self {
            id,
            name: name.to_string(),
            path: path_name.to_string(),
            content: String::new(),
            timestamp,
        })
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    pub async fn set_content(&mut self) -> Result<()> {
        let path = PathBuf::from(&self.path);
        let extension = path.extension().unwrap();
        if is_plain(extension) {
            self.content = read_plain_file(&self.path).await?;
        } else {
            self.content = read_docx_file(&self.path).await?;
        }
        Ok(())
    }
}

pub async fn is_support(dir_entry: &DirEntry) -> bool {
    if let Ok(file_type) = dir_entry.file_type().await {
        let file_name = dir_entry.file_name();
        let file_name = file_name.to_str();
        if file_type.is_file() && file_name.is_some() && !file_name.unwrap().starts_with("~$") {
            let path = dir_entry.path();
            let extension = path.extension();
            if let Some(e) = extension {
                return is_plain(e) || is_hyper(e);
            }
        }
    }
    false
}

fn is_plain(extension: &OsStr) -> bool {
    let extension = extension.to_ascii_lowercase();
    for e in PLAIN_FILE_TYPE {
        if e == extension {
            return true;
        }
    }
    false
}

fn is_hyper(extension: &OsStr) -> bool {
    let extension = extension.to_ascii_lowercase();
    for e in HYPER_FILE_TYPE {
        if e == extension {
            return true;
        }
    }
    false
}

/// 文件时间戳
async fn get_file_timestamp(dir_entry: &DirEntry) -> Result<u64> {
    let path = dir_entry.path();
    let io_error = OpenOrReadDocument {
        path: path.to_str().map(|s| s.to_string()).unwrap(),
    };
    let timestamp = dir_entry
        .metadata()
        .await
        .context(io_error.clone())?
        .modified()
        .context(io_error.clone())?
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| {
            error!("取文件时间戳失败：{}", e);
            Error::ComputeSystemTime
        })?
        .as_secs();
    Ok(timestamp)
}

/// 调用pandoc，读取docx文件，返回文件内容
/// 需要提前安装pandoc
#[instrument]
async fn read_docx_file(path: &str) -> Result<String> {
    info!("read_docx_file");
    let output = Command::new("pandoc")
        .args([
            "-t",
            "plain",
            "--wrap=none",
            "--markdown-headings=atx",
            path,
        ])
        .output()
        .await
        .context(PandocConvert {
            path: path.to_string(),
        })?;

    String::from_utf8(output.stdout).map_err(|e| {
        error!("读取文件{}失败：{}", path, e);
        Error::UnsupportedEncoding {
            path: path.to_string(),
        }
    })
}

#[instrument]
async fn read_plain_file(path: &str) -> Result<String> {
    match tokio::fs::read_to_string(path).await {
        Err(e) => {
            error!("读取文件{}失败：{}", path, e);
            let io_error = OpenOrReadDocument {
                path: path.to_string(),
            };
            let mut file = File::open(path).await.context(io_error.clone())?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).await.context(io_error.clone())?;
            GBK.decode(&buf, DecoderTrap::Strict).map_err(|e| {
                error!("读取文件{}失败：{}", path, e);
                Error::UnsupportedEncoding {
                    path: path.to_string(),
                }
            })
        }
        Ok(s) => Ok(s),
    }
}

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Snafu, CommandError)]
pub enum Error {
    #[snafu(display("未支持的文档类型：{path}"), context(suffix(false)))]
    UnsupportedDocument { path: String },

    #[snafu(
        display("该文档非 UTF-8 或者 GBK 编码：{path}"),
        context(suffix(false))
    )]
    UnsupportedEncoding { path: String },

    #[snafu(display("无法打开或读取文件：{path}"), context(suffix(false)))]
    OpenOrReadDocument {
        path: String,
        source: std::io::Error,
    },

    #[snafu(
        display("Pandoc 无法将 word 文件转换成普通文本：{path}"),
        context(suffix(false))
    )]
    PandocConvert {
        path: String,
        source: std::io::Error,
    },

    #[snafu(display("系统时间错误"), context(suffix(false)))]
    ComputeSystemTime,
}
