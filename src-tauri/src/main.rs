#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::path::{Path, PathBuf};

use crate::config::{get_configs, save_config};
use crate::meili_client::{stop_meilisearch, MeiliClient};
use crate::utils::union_err;
use lazy_static::*;
use meilisearch_sdk::search::SearchResults;
use serde_json::{json, Value};
use structs::Docx;
use tauri::api::dir::{read_dir, DiskEntry};
use tauri::api::process::Command;
use tauri::{Manager, WindowEvent};
use time::{macros::format_description, UtcOffset};
use tracing::info;
use tracing_subscriber::fmt::time::OffsetTime;

mod config;
mod meili_client;
mod structs;
mod utils;

lazy_static! {
    static ref HOST: String = "http://localhost:7700".to_string();
    static ref API_KEY: String = "thlstsul".to_string();
    static ref CLIENT: MeiliClient = MeiliClient::new(&HOST, &API_KEY);
    static ref INDEX_NAME: String = "WORD-INDEX".to_string();
    static ref INDEX_PATH: String = "paths".to_string();
}

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
        .setup(|app| {
            let main_window = app.get_window("main").unwrap();
            main_window.on_window_event(|event| match event {
                WindowEvent::Destroyed => {
                    info!("Destroyed");
                    stop_meilisearch();
                }
                _ => {}
            });
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
async fn index_doc_file(dir_path: String) -> Result<(), String> {
    info!("index_doc_file: {}", dir_path);
    let file_paths = plat_dir(&dir_path)?;
    for file_path in file_paths.iter().rev() {
        if file_path.is_file()
            && (file_path.extension() == Some(std::ffi::OsStr::new("docx"))
                || file_path.extension() == Some(std::ffi::OsStr::new("doc")))
        {
            let mut docx = Docx::new(&file_path)?;

            if !docx.get_name().starts_with("~$") && !existed(&docx).await {
                docx.set_content()?;
                info!("indexing: {}", docx.get_path());
                CLIENT
                    .add_or_replace(&INDEX_NAME, &[docx], None)
                    .await
                    .map_err(union_err)?;
            }
        }
    }

    Ok(())
}

///扁平化文件夹
fn plat_dir(dir: &str) -> Result<Vec<PathBuf>, String> {
    let dir_path = Path::new(&dir);
    if dir_path.is_file() {
        return Ok(vec![dir_path.to_path_buf()]);
    }
    let dir_entry = read_dir(dir_path, true).map_err(union_err)?;
    let mut paths = disk_entry_recursive(dir_entry);
    paths.sort();
    Ok(paths)
}

fn disk_entry_recursive(disk_entrys: Vec<DiskEntry>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for disk_entry in disk_entrys {
        if let Some(children) = disk_entry.children {
            paths.extend(disk_entry_recursive(children));
        } else {
            paths.push(disk_entry.path);
        }
    }
    paths
}

/// 搜索文件，支持分页
#[tauri::command]
async fn search_doc_file(keyword: String, offset: usize, limit: usize) -> Result<Value, String> {
    info!(
        "search_doc_file query: {}, offset: {}, limit: {}",
        keyword, offset, limit
    );

    let results: SearchResults<Docx> = CLIENT
        .search(&INDEX_NAME, &keyword, offset, limit)
        .await
        .map_err(union_err)?;

    let mut ret = json!({});
    ret["total"] = json!(results.nb_hits);
    ret["offset"] = json!(results.offset);
    ret["limit"] = json!(results.limit);
    ret["results"] = json!(results
        .hits
        .iter()
        .map(|hit| hit.result.clone())
        .collect::<Vec<Docx>>());
    Ok(ret)
}

#[tauri::command]
async fn save_path(path: String) -> Result<(), String> {
    info!("save_path: {}", path);
    let value = get_configs(INDEX_PATH.as_str())?;
    if value.is_null() {
        let mut value = json!([]);
        value
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::String(path));
        save_config(INDEX_PATH.as_str(), value)?;
    } else {
        let mut value = value.clone();
        value
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::String(path));
        save_config(INDEX_PATH.as_str(), value)?;
    }
    Ok(())
}

#[tauri::command]
async fn get_paths() -> Result<Value, String> {
    info!("get_paths");
    let value = get_configs(INDEX_PATH.as_str())?;
    if value == json!(null) {
        Ok(json!([]))
    } else {
        Ok(value)
    }
}

#[tauri::command]
fn open_file(path: String) -> Result<(), String> {
    info!("open_file: {}", path);
    Command::new("rundll32")
        .args(["url.dll", "FileProtocolHandler", &path])
        .output()
        .map_err(union_err)?;
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
