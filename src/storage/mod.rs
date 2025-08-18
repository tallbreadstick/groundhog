use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::groundhog::{GroundHogConfig, TreeNode};

pub fn init_at(target: &Path) -> Result<()> {
    let root = target;
    let gh_dir = root.join(".groundhog");
    if gh_dir.exists() {
        return Err(anyhow!(".groundhog already exists at {}", gh_dir.display()));
    }

    fs::create_dir_all(gh_dir.join("store"))?;

    let mut cfg = GroundHogConfig::new();
    cfg.hash_tree = TreeNode { hash: String::from(""), children: None };
    save_config(root, &cfg)?;

    Ok(())
}

pub fn find_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".groundhog").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    Err(anyhow!("not inside a groundhog workspace (.groundhog not found)"))
}

pub fn store_dir(root: &Path) -> PathBuf {
    root.join(".groundhog").join("store")
}

pub fn meta_path(root: &Path) -> PathBuf {
    root.join(".groundhog").join("meta.json")
}

pub fn snapshot_dir_for(store_dir: &Path, name: &str) -> PathBuf {
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
    store_dir.join(format!("{}_{}", ts, sanitize(name)))
}

pub fn load_config(root: &Path) -> Result<GroundHogConfig> {
    let meta = meta_path(root);
    let content = fs::read_to_string(&meta)?;
    let cfg: GroundHogConfig = serde_json::from_str(&content)?;
    Ok(cfg)
}

pub fn save_config(root: &Path, cfg: &GroundHogConfig) -> Result<()> {
    let meta = meta_path(root);
    let content = serde_json::to_string_pretty(cfg)?;
    fs::write(meta, content)?;
    Ok(())
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}


