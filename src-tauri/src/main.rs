#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::time::Duration;

use crate::config::Config;
use crate::meilisearch::{add_documents, existed, index_finished, search};
use async_walkdir::{DirEntry, Filtering, WalkDir};
use command_result::CommandError;
use command_result::Result;
use serde_json::{json, Value};
use structs::Docx;
use time::{macros::format_description, UtcOffset};
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tracing::{error, info, instrument};
use tracing_subscriber::fmt::time::OffsetTime;

mod command_result;
mod config;
mod meilisearch;
mod structs;

const INDEX_NAME: &str = "WORD-INDEX";

fn main() {
    let file_appender = tracing_appender::rolling::never(".", "word-index.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    let offset = UtcOffset::current_local_offset().expect("should get local offset!");
    let timer = OffsetTime::new(
        offset,
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
    );
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_timer(timer)
        .with_ansi(false)
        .init();

    tauri::Builder::default()
        .setup(|_app| {
            meilisearch::setup(Some(INDEX_NAME.to_string()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            index_doc_file,
            search_doc_file,
            save_path,
            get_paths,
            open_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 为指定路径的文件创建索引
#[tauri::command]
#[instrument]
async fn index_doc_file(dir_path: String) -> Result<()> {
    let mut entries = WalkDir::new(dir_path).filter(|entry| async move {
        if let Some(true) = entry
            .path()
            .file_name()
            .map(|f| f.to_string_lossy().starts_with('.'))
        {
            return Filtering::IgnoreDir;
        }
        Filtering::Continue
    });
    loop {
        match entries.next().await {
            Some(Ok(entry)) => {
                tokio::spawn(index_one(entry)).await?;
            }
            Some(Err(e)) => return Err(CommandError(e.to_string())),
            None => break,
        }
    }

    // 监控索引task，直到索引完成
    while !index_finished(INDEX_NAME.to_string()).await {
        sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

async fn index_one(entry: DirEntry) {
    let docx = Docx::new(&entry).await;
    match docx {
        Ok(mut docx) => {
            if !existed(INDEX_NAME.to_string(), docx.get_id(), docx.get_timestamp()).await {
                if let Err(e) = docx.set_content().await {
                    error!("{e}");
                } else if let Err(e) = add_documents(INDEX_NAME.to_string(), &[docx], None).await {
                    error!("{e}");
                }
            }
        }
        Err(e) => error!("{e}"),
    }
}

/// 搜索文件，支持分页
#[tauri::command]
#[instrument]
async fn search_doc_file(
    keyword: String,
    offset: usize,
    limit: usize,
    classes: Option<Vec<String>>,
) -> Result<Value> {
    let results = search(INDEX_NAME.to_string(), keyword, offset, limit, classes).await?;

    let mut ret = json!({});
    ret["total"] = json!(results.estimated_total_hits);
    ret["offset"] = json!(results.offset);
    ret["limit"] = json!(results.limit);
    ret["results"] = json!(results.hits);
    Ok(ret)
}

/// 保存索引路径
#[tauri::command]
#[instrument]
async fn save_path(path: String) -> Result<()> {
    info!("save_path");
    let mut config = Config::load().await?;
    if config.paths.contains(&path) {
        return Err(CommandError(format!("{path}\n索引路径已存在！")));
    } else {
        config.paths.push(path);
        config.save().await?;
    }

    Ok(())
}

/// 读取索引路径
#[tauri::command]
#[instrument]
async fn get_paths() -> Result<Vec<String>> {
    info!("get_paths");
    let config = Config::load().await?;
    Ok(config.paths)
}

#[tauri::command]
#[instrument]
fn open_file(path: String) -> Result<()> {
    info!("open_file");
    open_file_by_default_program(&path)
}

#[cfg(windows)]
fn open_file_by_default_program(path: &str) -> Result<()> {
    use tauri::api::process::Command;

    Command::new("rundll32")
        .args(["url.dll", "FileProtocolHandler", path])
        .output()?;
    Ok(())
}

#[cfg(not(windows))]
fn open_file_by_default_program(path: &str) -> Result<()> {
    Err(CommandError(String::from("未适配！")))
}
