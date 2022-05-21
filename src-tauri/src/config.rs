use std::io::Write;
use std::{fs::File, io::Read};
use serde_json::{Value, json};
use tracing::info;

use crate::utils::union_err;

const DB: &str = "word-index.db";

pub fn save_config(key: &str, value: Value) -> Result<(), String> {
    info!("save_config: {}->{}", key, value);
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(DB)
        .map_err(union_err)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(union_err)?;
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
        .open(DB)
        .map_err(union_err)?;
    file.write_all(serde_json::to_string(&map).map_err(union_err)?.as_bytes())
        .map_err(union_err)?;
    Ok(())
}

pub fn get_configs(key: &str) -> Result<Value, String> {
    info!("get_configs: {}", key);
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(DB)
        .unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(union_err)?;
    let map: Value = serde_json::from_str(&contents).unwrap_or(json!(null));
    info!("get_configs: {}", map);
    Ok(map[key].clone())
}