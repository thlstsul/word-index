#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::time::Duration;

use crate::config::{get_configs, save_config};
use crate::meilisearch::{add_documents, index_finished, search};
use async_walkdir::{DirEntry, Filtering, WalkDir};
use serde_json::{json, Value};
use structs::Docx;
use tauri::api::process::Command;
use time::{macros::format_description, UtcOffset};
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tracing::{error, info, instrument};
use tracing_subscriber::fmt::time::OffsetTime;

mod config;
mod meilisearch;
mod structs;

const INDEX_NAME: &str = "WORD-INDEX";
const INDEX_PATH: &str = "paths";

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
            meilisearch::setup();
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
async fn index_doc_file(dir_path: String) -> Result<(), String> {
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
            Some(Ok(entry)) => match index_one(&entry).await {
                Err(e) => error!("{}", e),
                Ok(_) => {},
            },
            Some(Err(e)) => return Err(e.to_string()),
            None => break,
        }
    }

    // 监控索引task，直到索引完成
    while !index_finished(INDEX_NAME.to_string()).await {
        info!("indexing...");
        sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

async fn index_one(entry: &DirEntry) -> anyhow::Result<()> {
    if entry.file_type().await?.is_dir() {
        return Ok(());
    }
    let file_name = entry
        .file_name()
        .into_string()
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    if file_name.starts_with("~$") || !(file_name.ends_with(".docx") || file_name.ends_with(".doc"))
    {
        return Ok(());
    }

    let mut docx = Docx::new(&entry.path())?;
    if !docx.get_name().starts_with("~$") && !existed(&docx).await {
        docx.set_content()?;
        info!("indexing: {}", docx.get_path());
        add_documents(INDEX_NAME.to_string(), &[docx], None).await?;
    }
    Ok(())
}

/// 搜索文件，支持分页
#[tauri::command]
#[instrument]
async fn search_doc_file(keyword: String, offset: usize, limit: usize) -> Result<Value, String> {
    let results = search(INDEX_NAME.to_string(), keyword, offset, limit)
        .await
        .map_err(|e| format!("检索失败：{}", e))?;

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
async fn save_path(path: String) -> Result<(), String> {
    info!("save_path");
    let mut value = get_configs(INDEX_PATH).map_err(|e| format!("读取索引路径失败：{}", e))?;
    if value.is_null() {
        value = json!([]);
    }
    value
        .as_array_mut()
        .unwrap()
        .push(serde_json::Value::String(path));
    save_config(INDEX_PATH, value).map_err(|e| format!("保存索引路径失败：{}", e))?;
    Ok(())
}

/// 读取索引路径
#[tauri::command]
#[instrument]
async fn get_paths() -> Result<Value, String> {
    info!("get_paths");
    let value = get_configs(INDEX_PATH).map_err(|e| format!("读取索引路径失败：{}", e))?;
    if value == json!(null) {
        Ok(json!([]))
    } else {
        Ok(value)
    }
}

#[tauri::command]
#[instrument]
fn open_file(path: String) -> anyhow::Result<(), String> {
    info!("open_file");
    open_file_by_default_program(&path).map_err(|e| format!("打开原文件失败：{}", e))
}

fn open_file_by_default_program(path: &str) -> anyhow::Result<()> {
    Command::new("rundll32")
        .args(["url.dll", "FileProtocolHandler", &path])
        .output()?;
    Ok(())
}

/// 文件是否已经存在、是否过期
async fn existed(docx: &Docx) -> bool {
    let id = docx.get_id();
    let file_timestamp = docx.get_timestamp();
    let exist_docxs = search_doc_file(format!("\"{}\"", id), 0, 1).await;
    if let Ok(exist_docxs) = exist_docxs {
        for exist_docx in exist_docxs["results"].as_array().unwrap() {
            if exist_docx["id"] == id && exist_docx["timestamp"] == file_timestamp {
                return true;
            }
        }
    }
    false
}
