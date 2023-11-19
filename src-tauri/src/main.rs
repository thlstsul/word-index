#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use crate::config::Config;
use command_result::CommandError;
use command_result::Result;
use search::SearchState;
use structs::SearchFruit;
use tauri::Manager;
use tauri::State;
use time::{macros::format_description, UtcOffset};
use tracing::{info, instrument};
use tracing_subscriber::fmt::time::OffsetTime;

mod command_result;
mod config;
mod search;
mod structs;

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
            let state = SearchState::new();
            app.manage(state);
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
async fn index_doc_file(dir_path: String, state: State<'_, SearchState>) -> Result<()> {
    state.index(dir_path).await?;
    Ok(())
}

/// 搜索文件，支持分页
#[tauri::command]
async fn search_doc_file(
    keyword: String,
    offset: usize,
    limit: usize,
    classes: Option<Vec<String>>,
    state: State<'_, SearchState>,
) -> Result<SearchFruit> {
    let fruit = state.search(keyword, offset, limit, classes)?;
    Ok(fruit)
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
