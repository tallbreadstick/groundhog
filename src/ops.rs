use anyhow::{anyhow, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

use colored::*;
use crate::config::groundhog::{Snapshot, SnapshotKind, TreeNode, Scope};
use comfy_table::{Table, presets::UTF8_FULL, Cell, Attribute, ContentArrangement};
use crate::drivers::selector::select_drivers_for_target;
use crate::registry;
use crate::storage;
// use crate::utils::io; // kept for future diff/copy utilities

// Help is provided by clap; keep no-op or remove custom help.

pub fn do_init(target: Option<String>, name: Option<String>) -> Result<()> {
    let target_path = target
        .as_deref()
        .map(Path::new)
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir()?);

    let gh_dir = target_path.join(".groundhog");
    let target_str = target_path.display().to_string();

    // Check for recovery case: scope in registry but no .groundhog
    let all = registry::load_registry()?;
    if !gh_dir.exists() {
        if let Some(existing) = all.iter().find(|s| s.target == target_str) {
            storage::init_at(&target_path)?;
            println!("{} {}", "i".yellow().bold(), "Recovered missing .groundhog workspace".yellow());

            if let Some(new_name) = name {
                if prompt_confirm(&format!(
                    "Scope '{}' exists. Keep old name '{}' instead of new name '{}'? [y/N] ",
                    existing.target, existing.name, new_name
                ))? {
                    println!("{} {}", "✔".green().bold(), format!("Recovered scope '{}'", existing.name).green());
                } else {
                    do_rename(&Some(existing.name.clone()), &new_name)?;
                }
            } else {
                println!("{} {}", "✔".green().bold(), format!("Recovered scope '{}'", existing.name).green());
            }
            return Ok(());
        }
    }

    // If .groundhog doesn’t exist, create
    let created_workspace = if gh_dir.exists() {
        false
    } else {
        storage::init_at(&target_path)?;
        true
    };

    // Register new scope globally
    let kind = SnapshotKind::Filesystem;
    let scope_name = name.unwrap_or_else(|| generate_scope_name(&target_str));
    let scope = Scope { name: scope_name.clone(), target: target_str, kind, created_at: chrono::Local::now() };

    match registry::register_scope(scope.clone()) {
        Ok(()) => {}
        Err(e) => {
            if format!("{}", e).contains("already exists") {
                return Err(anyhow!(e.to_string())); // hard error for duplicate target or name
            } else {
                return Err(e);
            }
        }
    }

    if created_workspace {
        println!("{} {}", "✔".green().bold(), format!("Initialized .groundhog at {}", target_path.display()).green());
    } else {
        println!("{} {}", "i".yellow().bold(), format!("Using existing .groundhog at {}", target_path.display()).yellow());
    }
    println!("{} {} {}", "✔".green().bold(), "Initialized scope:".green(), scope_name.green());
    Ok(())
}

pub fn do_snapshot(global_scope: &Option<String>, name: &str, password: Option<String>) -> Result<()> {
    let scope = registry::resolve_scope(global_scope)?;
    let root = std::path::Path::new(&scope.target).to_path_buf();
    let mut config = storage::load_config(&root)?;

    // --- NEW: check local config for duplicate snapshot name within this scope ---
    if config
        .snapshots
        .iter()
        .any(|s| s.name == name && s.scope == scope.name)
    {
        eprintln!(
            "{} {}: {}",
            "!".yellow().bold(),
            "Warning".yellow(),
            format!(
                "snapshot '{}' already exists in scope '{}'; skipping",
                name, scope.name
            )
        );
        return Ok(());
    }

    let store_dir = storage::store_dir(&root);
    let snapshot_dir = storage::snapshot_dir_for(&store_dir, name);

    // Keep an extra safeguard: if a directory with the same name already exists (orphaned),
    // warn and skip to avoid clobbering.
    if snapshot_dir.exists() {
        eprintln!(
            "{} {}: {}",
            "!".yellow().bold(),
            "Warning".yellow(),
            format!(
                "snapshot directory already exists at '{}'!",
                snapshot_dir.display()
            )
        );
        return Ok(());
    }

    std::fs::create_dir_all(&snapshot_dir)?;

    let bar = create_progress_bar("Creating snapshot");

    // Placeholder: choose driver and capture state for the scope target
    let now = chrono::Local::now();
    let drivers = select_drivers_for_target(&scope.target);
    for driver in drivers {
        bar.set_message(format!("Capturing {}", scope.name));
        if let Err(err) = driver.snapshot(&scope.target, &snapshot_dir, password.as_deref()) {
            eprintln!("{} {}: {}", "!".yellow().bold(), "Warning".yellow(), err);
        }
        bar.inc(1);
    }

    // Placeholder: compute Merkle-like tree for changed content and store
    // Implement optimized diffing in `utils::hash` and use it here to minimize I/O
    let tree = TreeNode { hash: String::from("TODO:root_hash"), children: None };

    // Track metadata
    config.snapshots.push(Snapshot {
        name: name.to_string(),
        directory: relative_path(&snapshot_dir, &root)?,
        kind: scope.kind,
        locked: password.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
        created_at: now,
        scope: scope.name.clone(),
    });
    config.last_updated = chrono::Local::now();
    config.hash_tree = tree;
    storage::save_config(&root, &config)?;

    bar.finish_with_message("Snapshot created");
    println!("{} {}", "✔".green().bold(), format!("Snapshot '{}' created", name).green());
    Ok(())
}

pub fn do_rollback(global_scope: &Option<String>, name: Option<String>, latest: bool) -> Result<()> {
    let scope = registry::resolve_scope(global_scope)?;
    let root = std::path::Path::new(&scope.target).to_path_buf();
    let config = storage::load_config(&root)?;

    let snapshot_path = if latest {
        let last = config
            .snapshots
            .iter()
            .filter(|s| s.scope == scope.name)
            .last()
            .ok_or_else(|| anyhow!("no snapshots available"))?;
        root.join(&last.directory)
    } else {
        let name = name.ok_or_else(|| anyhow!("snapshot name required unless --latest"))?;
        let snap = config
            .snapshots
            .iter()
            .find(|s| s.name == name && s.scope == scope.name)
            .ok_or_else(|| anyhow!("snapshot '{}' not found", name))?;
        root.join(&snap.directory)
    };

    let bar = create_progress_bar("Rolling back");

    // Delegate to driver rollback for the scope target
    let drivers = select_drivers_for_target(&scope.target);
    for driver in drivers {
        if let Err(err) = driver.rollback(&scope.target, &snapshot_path) {
            eprintln!("{} {}: {}", "!".yellow().bold(), "Warning".yellow(), err);
        }
    }

    bar.finish_with_message("Rollback complete");
    println!("{} {}", "✔".green().bold(), "Rollback complete".green());
    Ok(())
}

pub fn do_delete(global_scope: &Option<String>, name: &str) -> Result<()> {
    let scope = registry::resolve_scope(global_scope)?;
    let root = std::path::Path::new(&scope.target).to_path_buf();
    let mut config = storage::load_config(&root)?;

    let index = config
        .snapshots
        .iter()
        .position(|s| s.name == name && s.scope == scope.name)
        .ok_or_else(|| anyhow!("snapshot '{}' not found", name))?;

    let snap = &config.snapshots[index];
    let snap_path = root.join(&snap.directory);

    // TODO: prompt for password if locked (when encryption is implemented)
    if !prompt_confirm(&format!("Delete snapshot '{}'? [y/N] ", name))? { 
        println!("Aborted.");
        return Ok(());
    }

    let bar = create_progress_bar("Deleting snapshot");
    bar.set_message(name.to_string());
    if snap_path.exists() {
        std::fs::remove_dir_all(&snap_path)?;
    }

    config.snapshots.remove(index);
    config.last_updated = chrono::Local::now();
    storage::save_config(&root, &config)?;

    // If this deletion results in no snapshots and the workspace removed, optionally cleanup registry here.
    let _ = registry::cleanup_invalid_scopes();

    bar.finish_with_message("Snapshot deleted");
    println!("{} {}", "✔".green().bold(), format!("Deleted snapshot '{}'", name).green());
    Ok(())
}

pub fn do_list(global_scope: &Option<String>) -> Result<()> {
    let scope = registry::resolve_scope(global_scope)?;
    let root = std::path::Path::new(&scope.target).to_path_buf();
    let config = storage::load_config(&root)?;

    if config.snapshots.is_empty() {
        println!("{} {}", "i".yellow().bold(), "No snapshots found".yellow());
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Name").add_attribute(Attribute::Bold),
            Cell::new("Type").add_attribute(Attribute::Bold),
            Cell::new("Timestamp").add_attribute(Attribute::Bold),
            Cell::new("Locked").add_attribute(Attribute::Bold),
        ]);

    for s in &config.snapshots {
        let kind = match s.kind { SnapshotKind::Filesystem => "filesystem", SnapshotKind::Database => "database" };
        let ts = s.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
        table.add_row(vec![
            Cell::new(&s.name),
            Cell::new(kind),
            Cell::new(ts),
            Cell::new(if s.locked { "yes" } else { "no" }),
        ]);
    }

    println!("{}", table);
    Ok(())
}

fn create_progress_bar(prefix: &str) -> ProgressBar {
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
    );
    bar.set_message(prefix.to_string());
    bar.enable_steady_tick(std::time::Duration::from_millis(80));
    bar
}

// determine_targets removed in favor of named scopes

fn relative_path(path: &Path, base: &Path) -> Result<String> {
    let rel = path
        .strip_prefix(base)
        .map_err(|_| anyhow!("failed to compute relative path"))?;
    Ok(rel.to_string_lossy().to_string())
}

fn prompt_confirm(message: &str) -> Result<bool> {
    use std::io::{self, Write};
    print!("{} {}", "?".cyan().bold(), message.cyan());
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let ans = input.trim().to_lowercase();
    Ok(ans == "y" || ans == "yes")
}

fn generate_scope_name(target: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(target.as_bytes());
    let digest = hasher.finalize();
    format!("scope-{}", &hex::encode(digest)[..8])
}

// Local resolve_scope removed; global registry-based resolver is used instead.

pub fn do_scopes() -> Result<()> {
    let scopes = registry::cleanup_invalid_scopes()?;
    let mut table = comfy_table::Table::new();
    table
        .load_preset(comfy_table::presets::UTF8_FULL)
        .set_header(vec!["Name", "Type", "Target", "Created"]);
    for s in &scopes {
        let kind = match s.kind { SnapshotKind::Filesystem => "filesystem", SnapshotKind::Database => "database" };
        table.add_row(vec![
            s.name.clone(),
            kind.to_string(),
            s.target.clone(),
            s.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        ]);
    }
    if scopes.is_empty() {
        println!("{} {}", "i".yellow().bold(), "No scopes defined".yellow());
    } else {
        println!("{}", table);
    }
    Ok(())
}

pub fn do_rename(global_scope: &Option<String>, new_name: &str) -> Result<()> {
    let old_scope_obj = registry::resolve_scope(global_scope)?;
    // Check global name collision
    let all = registry::load_registry()?;
    if all.iter().any(|s| s.name == new_name) {
        return Err(anyhow!("scope '{}' already exists", new_name));
    }
    // Update local snapshots' recorded scope name in that workspace
    let scope_root = std::path::Path::new(&old_scope_obj.target).to_path_buf();
    let mut cfg = storage::load_config(&scope_root)?;
    for snap in cfg.snapshots.iter_mut().filter(|s| s.scope == old_scope_obj.name) {
        snap.scope = new_name.to_string();
    }
    storage::save_config(&scope_root, &cfg)?;
    // Update global registry (clean + rename)
    let mut all = registry::cleanup_invalid_scopes()?;
    if let Some(s) = all.iter_mut().find(|s| s.name == old_scope_obj.name) {
        s.name = new_name.to_string();
    }
    registry::save_registry(&all)?;
    println!("{} {} -> {}", "✔".green().bold(), old_scope_obj.name.green(), new_name.green());
    Ok(())
}

pub fn do_version() {
    println!("{} {}", "groundhog".bold(), "0.1-alpha".cyan());
}
