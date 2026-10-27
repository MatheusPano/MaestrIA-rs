use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::persist::config::Config;
use crate::persist::read::Skipped;

#[derive(Debug)]
pub enum LoadOutcome {
    Missing,
    Loaded {
        config: Box<Config>,
        skipped: Vec<String>,
        backed_up: bool,
    },
    Unreadable {
        error: String,
        backed_up: bool,
    },
}

pub fn backup_path(config_path: &Path) -> PathBuf {
    let mut name = config_path.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

pub fn temp_path(config_path: &Path, pid: u32) -> PathBuf {
    let mut name = config_path.as_os_str().to_owned();
    name.push(format!(".{pid}.tmp"));
    PathBuf::from(name)
}

fn back_up(config_path: &Path) -> bool {
    fs::copy(config_path, backup_path(config_path)).is_ok()
}

pub fn load(config_path: &Path) -> LoadOutcome {
    if !config_path.exists() {
        return LoadOutcome::Missing;
    }
    let parsed = fs::read_to_string(config_path)
        .map_err(|e| e.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()));
    let object = match parsed {
        Ok(Value::Object(m)) => m,
        Ok(_) => {
            return LoadOutcome::Unreadable {
                error: "config.json is not a JSON object".into(),
                backed_up: back_up(config_path),
            };
        }
        Err(error) => {
            return LoadOutcome::Unreadable {
                error,
                backed_up: back_up(config_path),
            };
        }
    };
    let (config, Skipped(skipped)) = Config::from_json(&object);
    let backed_up = !skipped.is_empty() && back_up(config_path);
    LoadOutcome::Loaded {
        config: Box::new(config),
        skipped,
        backed_up,
    }
}

pub fn write_atomic(config_path: &Path, contents: &str, pid: u32) -> io::Result<()> {
    if let Some(dir) = config_path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temp = temp_path(config_path, pid);
    fs::write(&temp, contents)?;
    fs::rename(&temp, config_path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

pub fn save(config_path: &Path, config: &Config) -> io::Result<()> {
    write_atomic(config_path, &config.to_json_string(), std::process::id())
}
