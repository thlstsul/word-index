use std::{
    fs::File,
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use tauri::api::process::Command;
use tracing::info;

use crate::utils::union_err;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Docx {
    id: String,
    name: String,
    path: String,
    content: String,
    timestamp: u64,
}

impl Docx {
    pub fn new(path: &PathBuf) -> Result<Docx, String> {
        let name = path.file_name().unwrap().to_str().unwrap();
        let path_name = path.to_str().unwrap();
        let timestamp = get_file_timestamp(&path)?;
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
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_path(&self) -> &str {
        &self.path
    }
    pub fn get_content(&self) -> &str {
        &self.content
    }
    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }
    pub fn set_content(&mut self) -> Result<(), String> {
        self.content = read_docx_file(&self.path)?;
        Ok(())
    }
}

/// 文件时间戳
fn get_file_timestamp(path: &Path) -> Result<u64, String> {
    let file = File::open(path).map_err(union_err)?;
    let timestamp = file
        .metadata()
        .map_err(union_err)?
        .created()
        .map_err(union_err)?
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(union_err)?
        .as_secs();
    Ok(timestamp)
}

/// 调用pandoc，读取docx文件，返回文件内容
/// 需要提前安装pandoc
fn read_docx_file(path: &str) -> Result<String, String> {
    info!("read_docx_file: {}", path);
    Command::new("pandoc")
        .args(&[
            "-t",
            "plain",
            "--wrap=none",
            "--markdown-headings=atx",
            path,
        ])
        .output()
        .map(|output| output.stdout)
        .map_err(union_err)
}
