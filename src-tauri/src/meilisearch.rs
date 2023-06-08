use std::{collections::BTreeSet, mem::MaybeUninit, str::FromStr, sync::Once};

use actix_web::error::PayloadError;
use bytes::Bytes;
use clap::Parser;
use meilisearch_http::{setup_meilisearch, Opt};
use meilisearch_lib::{
    index::{
        MatchingStrategy, SearchQuery, SearchResult, Settings, DEFAULT_CROP_LENGTH,
        DEFAULT_CROP_MARKER, DEFAULT_HIGHLIGHT_POST_TAG, DEFAULT_HIGHLIGHT_PRE_TAG,
    },
    index_controller::{error::IndexControllerError, DocumentAdditionFormat, Update},
    milli::update::{IndexDocumentsMethod, Setting},
    options::MaxMemory,
    tasks::{
        task::{Task, TaskContent},
        TaskFilter,
    },
    MeiliSearch,
};
use serde::Serialize;
use serde_json::{to_string, Value};
use snafu::prelude::*;
use tokio::{runtime::Builder, sync::mpsc, task::LocalSet};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info};
use word_index::CommandError;

static mut MEILI_SEARCH: MaybeUninit<MeiliSearch> = MaybeUninit::uninit();
static INIT: Once = Once::new();

const API_KEY: &str = "thlstsul";

pub fn setup(index_name: Option<String>) {
    info!("meilisearch启动开始……");
    INIT.call_once(|| {
        let rt = Builder::new_current_thread().enable_all().build().unwrap();

        std::thread::spawn(move || {
            let local = LocalSet::new();
            // setup_meilisearch包含tokio::task::spawn_local操作，其只能运行于LocalSet设置的local context
            local.spawn_local(async move {
                let mut opt = Opt::parse();
                // 初始化 --max-indexing-memory=1024Mb --db-path={} --master-key={}
                opt.indexer_options.max_indexing_memory = MaxMemory::from_str("1024Mb").unwrap();
                opt.db_path = std::env::current_dir().unwrap().join("data.ms");
                opt.master_key = Some(API_KEY.to_string());

                unsafe {
                    MEILI_SEARCH
                        .as_mut_ptr()
                        .write(setup_meilisearch(&opt).unwrap());
                }

                // 加快第一次检索
                if let Some(meili) = get_meili() {
                    if let Ok(indexes) = meili.list_indexes().await {
                        info!("索引列表：{indexes:?}");
                        if indexes.is_empty() && index_name.is_some() {
                            let mut filters = BTreeSet::new();
                            filters.insert("class".to_string());
                            info!("创建索引 with filter: {filters:?}");
                            let settings = Settings {
                                filterable_attributes: Setting::Set(filters),
                                ..Default::default()
                            };
                            let update = Update::Settings {
                                settings,
                                is_deletion: false,
                                allow_index_creation: true,
                            };
                            meili
                                .register_update(index_name.unwrap(), update)
                                .await
                                .unwrap();
                        }
                    }
                }
            });
            rt.block_on(local);
        });
    });
    info!("meilisearch启动完成。");
}

pub async fn add_documents<T: 'static + Serialize>(
    index_name: String,
    documents: &[T],
    primary_key: Option<String>,
) -> Result<Task> {
    let update = Update::DocumentAddition {
        payload: Box::new(doc2stream(documents).await?),
        primary_key,
        method: IndexDocumentsMethod::ReplaceDocuments,
        format: DocumentAdditionFormat::Json,
        allow_index_creation: true,
    };

    get_meili()
        .unwrap()
        .register_update(index_name, update)
        .await
        .context(AddDocument)
}

pub async fn search(
    index_name: String,
    keyword: String,
    offset: usize,
    limit: usize,
    classes: Option<Vec<String>>,
) -> Result<SearchResult> {
    let filter = classes.map(|v| {
        let value: Vec<String> = v.iter().map(|c| format!("class = {c}")).collect();
        Value::String(value.join(" OR "))
    });
    let search_query = SearchQuery {
        q: Some(keyword),
        offset: Some(offset),
        limit,
        attributes_to_retrieve: None,
        attributes_to_crop: None,
        crop_length: DEFAULT_CROP_LENGTH(),
        attributes_to_highlight: None,
        show_matches_position: false,
        filter,
        sort: None,
        facets: None,
        highlight_post_tag: DEFAULT_HIGHLIGHT_POST_TAG(),
        highlight_pre_tag: DEFAULT_HIGHLIGHT_PRE_TAG(),
        crop_marker: DEFAULT_CROP_MARKER(),
        matching_strategy: MatchingStrategy::All,
    };

    get_meili()
        .unwrap()
        .search(index_name, search_query)
        .await
        .context(SearchDocument)
}

pub async fn existed(index_name: String, id: &str, timestamp: u64) -> bool {
    let search_query = SearchQuery {
        q: Some(id.to_string()),
        offset: None,
        limit: 1,
        attributes_to_retrieve: Some(BTreeSet::from(["id".to_string(), "timestamp".to_string()])),
        attributes_to_crop: None,
        crop_length: DEFAULT_CROP_LENGTH(),
        attributes_to_highlight: None,
        show_matches_position: false,
        filter: None,
        sort: None,
        facets: None,
        highlight_post_tag: DEFAULT_HIGHLIGHT_POST_TAG(),
        highlight_pre_tag: DEFAULT_HIGHLIGHT_PRE_TAG(),
        crop_marker: DEFAULT_CROP_MARKER(),
        matching_strategy: MatchingStrategy::All,
    };
    let result = get_meili().unwrap().search(index_name, search_query).await;

    if let Ok(r) = result {
        if r.hits.is_empty() {
            return false;
        }
        let doc = &r.hits[0].document;
        return id == doc["id"] && timestamp == doc["timestamp"];
    }
    false
}

/// 是否索引完成
pub async fn index_finished(index_name: String) -> bool {
    let mut filter = TaskFilter::default();
    filter.filter_index(index_name);
    filter.filter_fn(Box::new(|task| {
        if !task.is_finished() {
            matches!(task.content, TaskContent::DocumentAddition { .. })
        } else {
            false
        }
    }));

    let result = get_meili()
        .unwrap()
        .list_tasks(Some(filter), Some(1), None)
        .await;
    if let Ok(result) = result {
        result.is_empty()
    } else {
        error!("{result:?}");
        true
    }
}

fn get_meili() -> Option<&'static MeiliSearch> {
    if !INIT.is_completed() {
        return None;
    }
    unsafe { Some(&*MEILI_SEARCH.as_ptr()) }
}

async fn doc2stream<S>(
    documents: &[S],
) -> Result<ReceiverStream<core::result::Result<Bytes, PayloadError>>>
where
    S: Serialize,
{
    let (snd, recv) = mpsc::channel(1);
    snd.send(Ok(Bytes::from(
        to_string(documents).context(EncodeDocuments)?,
    )))
    .await
    .context(SendToChannel)?;
    Ok(ReceiverStream::new(recv))
}

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Snafu, CommandError)]
pub enum Error {
    #[snafu(display("无法编码索引文档"), context(suffix(false)))]
    EncodeDocuments { source: serde_json::error::Error },

    #[snafu(display("无法将待索引文档发送到队列"), context(suffix(false)))]
    SendToChannel {
        source: tokio::sync::mpsc::error::SendError<core::result::Result<Bytes, PayloadError>>,
    },

    #[snafu(display("添加索引文档失败"), context(suffix(false)))]
    AddDocument { source: IndexControllerError },

    #[snafu(display("检索文档失败"), context(suffix(false)))]
    SearchDocument { source: IndexControllerError },
}
