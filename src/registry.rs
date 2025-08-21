use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};
use crate::config::groundhog::Scope;

fn config_dir() -> Result<PathBuf> {
    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Ok(PathBuf::from(appdata).join("groundhog"));
        }
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
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Vec<Scope>>(&content) {
        Ok(list) => Ok(list),
        Err(_) => Ok(Vec::new()),
    }
}

pub fn save_registry(scopes: &[Scope]) -> Result<()> {
    let path = registry_path()?;
    let json = serde_json::to_string_pretty(scopes)?;
    let dir = path.parent().ok_or_else(|| anyhow!("invalid registry path"))?;
    let tmp = dir.join("registry.json.tmp");
    fs::write(&tmp, json)?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn register_scope(new_scope: Scope) -> Result<()> {
    let mut scopes = cleanup_invalid_scopes()?;

    if scopes.iter().any(|s| s.name == new_scope.name) {
        return Err(anyhow!("scope '{}' already exists", new_scope.name));
    }
    if scopes.iter().any(|s| s.target == new_scope.target) {
        return Err(anyhow!("a scope is already registered at target '{}'", new_scope.target));
    }

    scopes.push(new_scope);
    save_registry(&scopes)
}

pub fn cleanup_invalid_scopes() -> Result<Vec<Scope>> {
    let scopes = load_registry()?;
    let mut valid: Vec<Scope> = Vec::new();
    for s in scopes {
        let root = Path::new(&s.target);
        // Keep scope if directory exists (even if .groundhog missing — recovery allowed)
        if root.exists() {
            valid.push(s);
        }
    }
    save_registry(&valid)?;
    Ok(valid)
}

pub fn resolve_scope(global_scope: &Option<String>) -> Result<Scope> {
    if let Some(name) = global_scope {
        let all = load_registry()?;
        if let Some(s) = all.into_iter().find(|s| &s.name == name) {
            return Ok(s);
        } else {
            return Err(anyhow!("scope '{}' not found", name));
        }
    }

    let cwd = std::env::current_dir()?;
    let mut cur = cwd.as_path();
    while cur.parent().is_some() {
        let gh_path = cur.join(".groundhog");
        if gh_path.exists() {
            let cfg = crate::storage::load_config(cur)?;
            if let Some(snap) = cfg.snapshots.first() {
                let all = load_registry()?;
                if let Some(s) = all.into_iter().find(|x| x.name == snap.scope) {
                    return Ok(s);
                }
            } else {
                // Empty snapshot list but workspace exists → match by target
                let all = load_registry()?;
                if let Some(s) = all.into_iter().find(|x| x.target == cur.display().to_string()) {
                    return Ok(s);
                }
            }
        }
        cur = cur.parent().unwrap();
    }

    Err(anyhow!("no scope specified and no .groundhog found in current directory"))
}
