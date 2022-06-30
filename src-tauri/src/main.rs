#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::config::{get_configs, save_config};
use crate::utils::union_err;
use actix_web::error::PayloadError;
use bytes::Bytes;
use clap::Parser;
use meilisearch_http::{setup_meilisearch, Opt};
use meilisearch_lib::index::{
    SearchQuery, SearchResult, DEFAULT_CROP_LENGTH, DEFAULT_CROP_MARKER,
    DEFAULT_HIGHLIGHT_POST_TAG, DEFAULT_HIGHLIGHT_PRE_TAG,
};
use meilisearch_lib::index_controller::error::IndexControllerError;
use meilisearch_lib::index_controller::{DocumentAdditionFormat, Update};
use meilisearch_lib::milli::update::IndexDocumentsMethod;
use meilisearch_lib::options::MaxMemory;
use meilisearch_lib::tasks::task::Task;
use meilisearch_lib::MeiliSearch;
use serde::Serialize;
use serde_json::{json, to_string, Value};
use structs::Docx;
use tauri::api::dir::{read_dir, DiskEntry};
use tauri::api::process::Command;
use time::{macros::format_description, UtcOffset};
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::task::LocalSet;
use tokio_stream::Stream;
use tracing::{info, instrument};
use tracing_subscriber::fmt::time::OffsetTime;

mod config;
mod structs;
mod utils;

const INDEX_NAME: &str = "WORD-INDEX";
const INDEX_PATH: &str = "paths";
const API_KEY: &str = "thlstsul";

static mut MEILI_SEARCH: MaybeUninit<MeiliSearch> = MaybeUninit::uninit();


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
            let rt = Builder::new_current_thread().enable_all().build().unwrap();

            std::thread::spawn(move || {
                let local = LocalSet::new();
                // setup_meilisearch包含tokio::task::spawn_local操作，其只能运行于LocalSet::run_until设置的local context
                local.spawn_local(async {
                    let mut opt = Opt::parse();
                    //初始化 --max-indexing-memory=1024Mb --db-path={} --master-key={}
                    opt.indexer_options.max_indexing_memory =
                        MaxMemory::from_str("1024Mb").unwrap();
                    opt.db_path = std::env::current_dir().unwrap().join("data.ms");
                    opt.master_key = Some(API_KEY.to_string());

                    unsafe {
                        MEILI_SEARCH
                            .as_mut_ptr()
                            .write(setup_meilisearch(&opt).unwrap());
                    }
                });
                rt.block_on(local);
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
#[instrument]
async fn index_doc_file(dir_path: String) -> Result<(), String> {
    info!("index_doc_file");
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
                add_documents(INDEX_NAME.to_string(), &[docx], None)
                    .await
                    .map_err(union_err)?;
            }
        }
    }

    Ok(())
}

/// 搜索文件，支持分页
#[tauri::command]
#[instrument]
async fn search_doc_file(keyword: String, offset: usize, limit: usize) -> Result<Value, String> {
    info!("search_doc_file query");

    let results = search(INDEX_NAME.to_string(), keyword, offset, limit)
        .await
        .map_err(union_err)?;

    let mut ret = json!({});
    ret["total"] = json!(results.nb_hits);
    ret["offset"] = json!(results.offset);
    ret["limit"] = json!(results.limit);
    ret["results"] = json!(results.hits);
    Ok(ret)
}

#[tauri::command]
#[instrument]
async fn save_path(path: String) -> Result<(), String> {
    info!("save_path");
    let value = get_configs(INDEX_PATH)?;
    if value.is_null() {
        let mut value = json!([]);
        value
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::String(path));
        save_config(INDEX_PATH, value)?;
    } else {
        let mut value = value.clone();
        value
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::String(path));
        save_config(INDEX_PATH, value)?;
    }
    Ok(())
}

#[tauri::command]
#[instrument]
async fn get_paths() -> Result<Value, String> {
    info!("get_paths");
    let value = get_configs(INDEX_PATH)?;
    if value == json!(null) {
        Ok(json!([]))
    } else {
        Ok(value)
    }
}

#[tauri::command]
#[instrument]
fn open_file(path: String) -> Result<(), String> {
    info!("open_file");
    Command::new("rundll32")
        .args(["url.dll", "FileProtocolHandler", &path])
        .output()
        .map_err(union_err)?;
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

async fn add_documents<T: 'static + Serialize>(
    index_name: String,
    documents: &[T],
    primary_key: Option<String>,
) -> Result<Task, IndexControllerError> {
    let update = Update::DocumentAddition {
        payload: Box::new(doc2stream(documents).await),
        primary_key,
        method: IndexDocumentsMethod::ReplaceDocuments,
        format: DocumentAdditionFormat::Json,
        allow_index_creation: true,
    };

    get_meili().register_update(index_name, update).await
}

async fn search(
    index_name: String,
    keyword: String,
    offset: usize,
    limit: usize,
) -> Result<SearchResult, IndexControllerError> {
    let search_query = SearchQuery {
        q: Some(keyword),
        offset: Some(offset),
        limit,
        attributes_to_retrieve: None,
        attributes_to_crop: None,
        crop_length: DEFAULT_CROP_LENGTH,
        attributes_to_highlight: None,
        filter: None,
        sort: None,
        highlight_post_tag: DEFAULT_HIGHLIGHT_POST_TAG.to_string(),
        highlight_pre_tag: DEFAULT_HIGHLIGHT_PRE_TAG.to_string(),
        crop_marker: DEFAULT_CROP_MARKER.to_string(),
        matches: false,
        facets_distribution: None,
    };

    get_meili().search(index_name, search_query).await
}

fn get_meili() -> &'static MeiliSearch {
    unsafe { &*MEILI_SEARCH.as_ptr() }
}

async fn doc2stream<T: Serialize>(
    documents: &[T],
) -> impl Stream<Item = Result<Bytes, PayloadError>> {
    let (snd, recv) = mpsc::channel(1);
    snd.send(Ok(Bytes::from(to_string(documents).unwrap())))
        .await
        .unwrap();
    tokio_stream::wrappers::ReceiverStream::new(recv)
}
