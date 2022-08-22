use serde_json::{json, Value};
use std::io::Write;
use std::{fs::File, io::Read};
use tracing::info;
use anyhow::Result;

const DB: &str = "word-index.db";

pub fn save_config(key: &str, value: Value) -> Result<()> {
    info!("save_config: {}->{}", key, value);
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(DB)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let mut map: Value = serde_json::from_str(&contents).unwrap_or(json!({}));
    match map.get_mut(key) {
        Some(v) => {
            *v = value;
        }
        None => {
            map = json!({ key: value });
        }
    }
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(DB)?;
    file.write_all(serde_json::to_string(&map)?.as_bytes())?;
    Ok(())
}

pub fn get_configs(key: &str) -> Result<Value> {
    info!("get_configs: {}", key);
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(DB)
        .unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let map: Value = serde_json::from_str(&contents).unwrap_or(json!(null));
    info!("get_configs: {}", map);
    Ok(map[key].clone())
}
