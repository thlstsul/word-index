use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Write};

const DB: &str = "word-index.db";

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub paths: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let file = File::open(DB);
        match file {
            Ok(f) => {
                let reader = BufReader::new(f);
                serde_json::from_reader(reader).map_err(|e| anyhow!(e))
            }
            Err(_) => Ok(Self::default()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(DB)?;
        file.write_all(serde_json::to_string(self)?.as_bytes())?;
        Ok(())
    }
}