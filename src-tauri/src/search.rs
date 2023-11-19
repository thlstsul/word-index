use std::{fs::create_dir, path::Path};

use async_walkdir::{Filtering, WalkDir};
use log_error::LogError;
use snafu::ResultExt;
use snafu::Snafu;
use tantivy::{
    collector::{Count, MultiCollector, TopDocs},
    directory::MmapDirectory,
    query::{QueryParser, QueryParserError},
    schema::{Field, Schema},
    tokenizer::TokenizerManager,
    Document, Index, IndexReader, IndexSettings, IndexSortByField, IndexWriter, Order, Searcher,
    TantivyError, Term,
};
use tokio_stream::StreamExt;
use tracing::error;

use crate::structs::{Docx, SearchFruit};
use word_index::CommandError;

#[derive(Clone)]
pub struct SearchState {
    pub schema: Schema,
    pub index: Index,
    pub reader: IndexReader,
    pub parser: QueryParser,
}

impl SearchState {
    pub fn new() -> Self {
        let schema = Docx::schema();
        let tokenizer = tantivy_jieba::JiebaTokenizer {};
        let tokenizers = TokenizerManager::default();
        tokenizers.register("default", tokenizer);
        let settings = IndexSettings {
            sort_by_field: Some(IndexSortByField {
                field: "timestamp".to_string(),
                order: Order::Desc,
            }),
            ..Default::default()
        };
        let data_path = Path::new("./data");
        if !data_path.exists() || !data_path.is_dir() {
            create_dir(data_path).unwrap();
        }
        let dir = MmapDirectory::open(data_path).unwrap();
        let index = Index::builder()
            .schema(schema.clone())
            .tokenizers(tokenizers)
            .settings(settings)
            .open_or_create(dir)
            .expect("创建索引失败");
        let reader = index.reader().expect("创建Reader失败");
        let parser = QueryParser::for_index(&index, vec![]);
        Self {
            schema,
            index,
            reader,
            parser,
        }
    }

    pub async fn index(&self, dir_path: String) -> Result<()> {
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

        let mut writer = self.index.writer(100_000_000).context(CreateWriter)?;
        let searcher = self.reader.searcher();

        loop {
            match entries.next().await {
                Some(Ok(entry)) => {
                    let docx = Docx::new(&entry).await;
                    match docx {
                        Ok(docx) if !Self::exists(&searcher, &self.parser, &docx) => {
                            Self::add_document(&mut writer, docx)
                                .await
                                .log_error("add document fault");
                        }
                        Ok(_) => (),
                        Err(e) => error!("{e}"),
                    }
                }
                Some(e) => {
                    e.context(ReadDir)?;
                }
                None => break,
            }
        }

        Ok(())
    }

    pub fn search(
        &self,
        keyword: String,
        offset: usize,
        limit: usize,
        classes: Option<Vec<String>>,
    ) -> Result<SearchFruit> {
        let filter = if !keyword.is_empty() {
            let mut filter = format!("name:{} OR content:{}", keyword, keyword);
            if let Some(classes) = classes {
                if !classes.is_empty() {
                    filter = format!("({}) AND class:IN [{}]", filter, classes.join(" "));
                }
            }
            filter
        } else {
            String::from("*")
        };

        let searcher = self.reader.searcher();
        let query = self.parser.parse_query(&filter).context(SearchParser)?;
        let mut collectors = MultiCollector::new();
        let top_docs_handle =
            collectors.add_collector(TopDocs::with_limit(limit).and_offset(offset));
        let count_handle = collectors.add_collector(Count);
        let mut multi_fruit = searcher
            .search(&query, &collectors)
            .context(SearchDocument)?;
        let total = count_handle.extract(&mut multi_fruit);
        let top_docs = top_docs_handle.extract(&mut multi_fruit);

        let mut docs = Vec::new();
        for (_score, doc_address) in top_docs {
            // Retrieve the actual content of documents given its `doc_address`.
            let retrieved_doc = searcher.doc(doc_address).context(SearchDocument)?;
            let doc = Docx {
                name: Self::get_field_value(&retrieved_doc, &self.schema, "name"),
                content: Self::get_field_value(&retrieved_doc, &self.schema, "content"),
                path: Self::get_field_value(&retrieved_doc, &self.schema, "path"),
                ..Default::default()
            };
            docs.push(doc);
        }

        Ok(SearchFruit {
            results: docs,
            total,
            limit,
            offset,
        })
    }

    fn get_field_value(doc: &Document, schema: &Schema, name: &str) -> String {
        let field = schema.get_field(name);
        if let Ok(field) = field {
            let field_value = doc.get_first(field).and_then(|v| v.as_text());
            if let Some(value) = field_value {
                return value.to_string();
            }
        }
        String::new()
    }

    async fn add_document(writer: &mut IndexWriter, mut docx: Docx) -> Result<()> {
        // id field must 0
        let field = Field::from_field_id(0);
        let term = Term::from_field_text(field, docx.get_id());
        // 先删
        writer.delete_term(term);
        docx.set_content().await.context(OpenOrReadDocument)?;
        writer.add_document(docx.into()).context(AddDocument)?;
        writer.commit().context(Commit)?;
        Ok(())
    }

    fn exists(searcher: &Searcher, parser: &QueryParser, docx: &Docx) -> bool {
        let query = parser.parse_query(&format!(
            "id:{} AND timestamp:{}",
            docx.get_id(),
            docx.get_timestamp()
        ));
        if let Ok(query) = query {
            let rs = searcher.search(&query, &Count);
            if let Ok(rs) = rs {
                return rs > 0;
            } else {
                error!("{rs:?}")
            }
        } else {
            error!("{query:?}");
        }

        false
    }
}

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Snafu, CommandError)]
pub enum Error {
    #[snafu(display("遍历文档失败"), context(suffix(false)))]
    ReadDir { source: std::io::Error },

    #[snafu(display("无法打开或读取文件"), context(suffix(false)))]
    OpenOrReadDocument { source: crate::structs::Error },

    #[snafu(display("创建 WRITER 失败"), context(suffix(false)))]
    CreateWriter { source: TantivyError },

    #[snafu(display("添加索引文档失败"), context(suffix(false)))]
    AddDocument { source: TantivyError },

    #[snafu(display("提交索引文档失败"), context(suffix(false)))]
    Commit { source: TantivyError },

    #[snafu(display("解析检索语句失败"), context(suffix(false)))]
    SearchParser { source: QueryParserError },

    #[snafu(display("检索文档失败"), context(suffix(false)))]
    SearchDocument { source: TantivyError },
}
