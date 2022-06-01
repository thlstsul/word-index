use meilisearch_sdk::{
    client::Client,
    errors::Error,
    indexes::Index,
    search::{Query, SearchResults},
    tasks::Task,
};
use serde::{de::DeserializeOwned, Serialize};
use tauri::api::process::Command;
use tracing::info;

pub struct MeiliClient {
    client: Client,
}

impl MeiliClient {
    pub fn new(host: &str, api_key: &str) -> Self {
        let http_addr = host.replace("http://", "");
        restart_meilisearch(&http_addr, api_key);
        let client = Client::new(host, api_key);
        Self { client }
    }

    pub async fn add_or_replace<T: Serialize>(
        &self,
        index_name: &str,
        documents: &[T],
        primary_key: Option<&str>,
    ) -> Result<Task, Error> {
        self.client
            .index(index_name)
            .add_or_replace(documents, primary_key)
            .await
    }

    pub async fn search<T: 'static + DeserializeOwned>(
        &self,
        index_name: &str,
        keyword: &str,
        offset: usize,
        limit: usize,
    ) -> Result<SearchResults<T>, Error> {
        info!("search: {}", keyword);
        let index = self.client.index(index_name);
        let mut qry = Query::new(&index);

        let result = qry
            .with_query(&keyword)
            .with_offset(offset)
            .with_limit(limit)
            // .with_attributes_to_highlight(Selectors::Some(&attributes_to_highlight))
            .execute()
            .await?;
        Ok(result)
    }

    pub async fn get_index(&self, index_name: &str) -> Result<Index, Error> {
        info!("get_index: {}", index_name);
        self.client.get_index(index_name).await
    }
}

impl Drop for MeiliClient {
    fn drop(&mut self) {
        stop_meilisearch();
    }
}

fn restart_meilisearch(host: &str, api_key: &str) {
    info!("restart_meilisearch");
    if had_running_meilisearch() {
        stop_meilisearch();
    }
    let db_path = std::env::current_dir().unwrap().join("data.ms");
    // let log_path = std::env::current_dir().unwrap().join("meilisearch.log");
    if cfg!(target_os = "windows") {
        let cmd = format!(
            "Start-Process -WindowStyle Hidden -FilePath meilisearch -ArgumentList \"--max-indexing-memory=1024Mb --db-path={} --http-addr={} --master-key={}\"",
            db_path.to_str().unwrap(),
            host,
            api_key
        );
        info!("start_meilisearch: {}", cmd);
        Command::new("powershell")
            .args(["-c", &cmd])
            .output()
            .expect("failed to start meilisearch");
    } else {
        Command::new("nohup")
            .args([
                "meilisearch",
                "--db-path",
                db_path.to_str().unwrap(),
                "--http-addr",
                host,
                "--master-key",
                api_key,
                "&",
            ])
            .output()
            .expect("failed to start meilisearch");
    }
}

pub fn stop_meilisearch() {
    info!("stop_meilisearch");
    if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args(["/C", "Stop-Process -Name meilisearch"])
            .output()
            .expect("failed to stop meilisearch");
    } else {
        Command::new("killall")
            .args(["meilisearch"])
            .output()
            .expect("failed to stop meilisearch");
    }
}

fn had_running_meilisearch() -> bool {
    if cfg!(target_os = "windows") {
        let output = Command::new("powershell")
            .args(["/C", "Get-Process -Name meilisearch"])
            .output()
            .expect("failed to monitor meilisearch");
        output.stdout.len() > 0
    } else {
        let output = Command::new("pgrep")
            .args(["meilisearch"])
            .output()
            .expect("failed to monitor meilisearch");
        output.stdout.len() > 0
    }
}

#[cfg(test)]
mod tests {
    use tauri::async_runtime::block_on;

    use super::*;

    #[test]
    fn test_start_meilisearch() {
        tracing_subscriber::fmt::init();
        let host = "http://localhost:7700";
        let api_key = "thlstsul";
        restart_meilisearch(host, api_key);
        assert!(had_running_meilisearch());
        stop_meilisearch();
        assert!(!had_running_meilisearch());
    }

    #[test]
    fn test_client() {
        tracing_subscriber::fmt::init();
        let host = "http://localhost:7700";
        let api_key = "thlstsul";
        let client = MeiliClient::new(host, api_key);
        block_on(async {
            let index = client
                .add_or_replace("test", &["{test:'test'}"], None)
                .await
                .unwrap();
        });
    }
}
