// src/storage/mod.rs

use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::groundhog::{GroundHogConfig, TreeNode};

pub fn init_at(target: &Path, password: Option<String>) -> Result<()> {
    let root = target;
    let gh_dir = root.join(".groundhog");
    if gh_dir.exists() {
        return Err(anyhow!(".groundhog already exists at {}", gh_dir.display()));
    }
    fs::create_dir_all(gh_dir.join("store"))?;

    #[cfg(target_os = "windows")]
    {
        use winapi::um::fileapi::SetFileAttributesW;
        use winapi::um::winnt::FILE_ATTRIBUTE_HIDDEN;
        use std::os::windows::ffi::OsStrExt;

        let mut wide: Vec<u16> = gh_dir.as_os_str().encode_wide().collect();
        wide.push(0);
        unsafe { SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN); }
    }

    let cfg = GroundHogConfig::new(password);
    save_config(root, &cfg)?;
    Ok(())
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

pub fn manifest_path(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join("manifest.json")
}

pub fn save_manifest(snapshot_dir: &Path, tree: &TreeNode) -> Result<()> {
    let p = manifest_path(snapshot_dir);
    let json = serde_json::to_string_pretty(tree)?;
    if let Some(parent) = p.parent() { fs::create_dir_all(parent)?; }
    fs::write(p, json)?;
    Ok(())
}

pub fn load_manifest(snapshot_dir: &Path) -> Result<TreeNode> {
    let p = manifest_path(snapshot_dir);
    let content = fs::read_to_string(&p)?;
    let t: TreeNode = serde_json::from_str(&content)?;
    Ok(t)
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
