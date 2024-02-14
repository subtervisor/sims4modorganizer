use std::{collections::HashMap, path::PathBuf};

use chrono::{DateTime, Local};

#[derive(Debug)]
pub struct Mod {
    pub id: i64,
    pub name: String,
    pub directory: PathBuf,
    pub source_url: String,
    pub version: String,
    pub updated: DateTime<Local>,
    pub file_hashes: HashMap<PathBuf, u64>,
    pub tags: Vec<String>,
}
