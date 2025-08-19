use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use crate::config::groundhog::Scope;

fn config_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Ok(PathBuf::from(appdata).join("groundhog"));
        }
        // Fallback to HOME if APPDATA is missing
        if let Ok(home) = std::env::var("USERPROFILE") {
            return Ok(PathBuf::from(home).join("AppData\\Roaming").join("groundhog"));
        }
        Err(anyhow!("APPDATA not set; cannot determine config directory"))
    } else {
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home).join(".groundhog"));
        }
        Err(anyhow!("HOME not set; cannot determine config directory"))
    }
}

fn old_registry_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| anyhow!("unable to determine executable directory"))?;
    Ok(dir.join("registry.json"))
}

pub fn registry_path() -> Result<PathBuf> {
    let cfg_dir = config_dir()?;
    if !cfg_dir.exists() {
        fs::create_dir_all(&cfg_dir)?;
    }
    let new_path = cfg_dir.join("registry.json");

    // Migrate from old binary directory if present
    let old_path = old_registry_path()?;
    if old_path.exists() && !new_path.exists() {
        if fs::rename(&old_path, &new_path).is_err() {
            if let Ok(content) = fs::read(&old_path) {
                let _ = fs::write(&new_path, &content);
                let _ = fs::remove_file(&old_path);
            }
        }
    }

    Ok(new_path)
}

pub fn load_registry() -> Result<Vec<Scope>> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        // Treat empty file as empty registry
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Vec<Scope>>(&content) {
        Ok(list) => Ok(list),
        Err(_) => {
            // Corrupt registry; reset to empty to recover
            Ok(Vec::new())
        }
    }
}

pub fn save_registry(scopes: &[Scope]) -> Result<()> {
    let path = registry_path()?;
    let json = serde_json::to_string_pretty(scopes)?;
    // Write atomically: write to temp file then rename
    let dir = path.parent().ok_or_else(|| anyhow!("invalid registry path"))?;
    let tmp = dir.join("registry.json.tmp");
    fs::write(&tmp, json)?;
    // On Windows, replace by removing first if necessary
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn register_scope(new_scope: Scope) -> Result<()> {
    // Always start from a cleaned registry
    let mut scopes = cleanup_invalid_scopes()?;
    if scopes.iter().any(|s| s.name == new_scope.name) {
        return Err(anyhow!("scope '{}' already exists", new_scope.name));
    }
    scopes.push(new_scope);
    save_registry(&scopes)
}

pub fn cleanup_invalid_scopes() -> Result<Vec<Scope>> {
    let scopes = load_registry()?;
    let mut valid: Vec<Scope> = Vec::new();
    for s in scopes {
        let root = Path::new(&s.target);
        if root.join(".groundhog").is_dir() {
            valid.push(s);
        }
    }
    save_registry(&valid)?;
    Ok(valid)
}

pub fn resolve_scope(global_scope: &Option<String>) -> Result<Scope> {
    if let Some(name) = global_scope {
        // Explicit scope requested
        let all = load_registry()?;
        if let Some(s) = all.into_iter().find(|s| &s.name == name) {
            return Ok(s);
        } else {
            return Err(anyhow!("scope '{}' not found", name));
        }
    }

    // No -s: look for a local .groundhog in cwd or parents
    let cwd = std::env::current_dir()?;
    let mut cur = cwd.as_path();
    while cur.parent().is_some() {
        let gh_path = cur.join(".groundhog");
        if gh_path.exists() {
            let cfg = crate::storage::load_config(cur)?;
            // Use the scope tied to this workspace
            if let Some(snap) = cfg.snapshots.first() {
                // lookup scope by snap.scope in global registry
                let all = load_registry()?;
                if let Some(s) = all.into_iter().find(|x| x.name == snap.scope) {
                    return Ok(s);
                }
            }
        }
        cur = cur.parent().unwrap();
    }

    // Nothing local found
    Err(anyhow!("no scope specified and no .groundhog found in current directory"))
}
