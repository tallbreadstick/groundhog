use anyhow::{Result, anyhow};
use indicatif::{ProgressBar, ProgressStyle};
use rpassword::read_password;
use std::path::Path;

use crate::config::groundhog::{Scope, Snapshot, SnapshotKind};
use crate::drivers::selector::select_drivers_for_target;
use crate::registry;
use crate::storage;
use crate::utils::hash::{build_merkle_tree, diff_trees, hash_password, verify_password};
use crate::utils::io::{copy_selected_files, delete_selected_paths, make_skipper};
use colored::*;
use comfy_table::{Attribute, Cell, ContentArrangement, Table, presets::UTF8_FULL};

// Help is provided by clap; keep no-op or remove custom help.

pub fn do_init(
    target: Option<String>,
    name: Option<String>,
    password: Option<String>,
) -> Result<()> {
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
            storage::init_at(&target_path, password)?;
            println!(
                "{} {}",
                "i".yellow().bold(),
                "Recovered missing .groundhog workspace".yellow()
            );

            match name {
                Some(new_name) if new_name != existing.name => {
                    if prompt_confirm(&format!(
                        "Scope '{}' exists. Keep old name '{}' instead of new name '{}'? [y/N] ",
                        existing.target, existing.name, new_name
                    ))? {
                        println!(
                            "{} {}",
                            "✔".green().bold(),
                            format!("Recovered scope '{}'", existing.name).green()
                        );
                    } else {
                        do_rename(&Some(existing.name.clone()), &new_name)?;
                    }
                }
                _ => {
                    println!(
                        "{} {}",
                        "✔".green().bold(),
                        format!("Recovered scope '{}'", existing.name).green()
                    );
                }
            }
            return Ok(());
        }
    }

    // If .groundhog doesn’t exist, create
    let created_workspace = if gh_dir.exists() {
        false
    } else {
        storage::init_at(&target_path, password)?;
        true
    };

    // Register new scope globally
    let kind = SnapshotKind::Filesystem;
    let scope_name = name.unwrap_or_else(|| generate_scope_name(&target_str));
    let scope = Scope {
        name: scope_name.clone(),
        target: target_str,
        kind,
        created_at: chrono::Local::now(),
    };

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
        println!(
            "{} {}",
            "✔".green().bold(),
            format!("Initialized .groundhog at {}", target_path.display()).green()
        );
    } else {
        println!(
            "{} {}",
            "i".yellow().bold(),
            format!("Using existing .groundhog at {}", target_path.display()).yellow()
        );
    }
    println!(
        "{} {} {}",
        "✔".green().bold(),
        "Initialized scope:".green(),
        scope_name.green()
    );
    Ok(())
}

pub fn do_snapshot(
    global_scope: &Option<String>,
    name: &str,
    password: Option<String>,
) -> Result<()> {
    let scope = registry::resolve_scope(global_scope)?;
    let root = std::path::Path::new(&scope.target).to_path_buf();
    let mut config = storage::load_config(&root)?;

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
    if snapshot_dir.exists() {
        eprintln!(
            "{} {}: {}",
            "!".yellow().bold(),
            "Warning".yellow(),
            format!(
                "snapshot directory already exists at '{}'",
                snapshot_dir.display()
            )
        );
        return Ok(());
    }
    std::fs::create_dir_all(&snapshot_dir)?;
    let bar = create_progress_bar("Creating snapshot");

    // 1) Build current Merkle tree (ignoring .groundhog / .groundhogignore)
    let skip = make_skipper(&root);
    let current_tree = build_merkle_tree(&root, "".into(), skip)
        .map_err(|e| anyhow!("failed to build merkle tree: {}", e))?;

    // 2) Find baseline tree (last snapshot in this scope) if any
    let baseline_tree = config
        .snapshots
        .iter()
        .filter(|s| s.scope == scope.name)
        .last()
        .map(|last| storage::load_manifest(&root.join(&last.directory)))
        .transpose()
        .unwrap_or(None);

    // 3) Minimal copy based on diff
    // If no baseline, do a full copy via diff add-all.
    let (to_copy, _to_delete) = if let Some(base) = &baseline_tree {
        let d = diff_trees(&current_tree, base);
        let mut copy_list = d.added.clone();
        copy_list.extend(d.modified.iter().cloned());
        (copy_list, d.deleted)
    } else {
        // copy all files in current_tree
        let mut all_files = Vec::new();
        for (path, (_h, is_dir)) in crate::utils::hash::flatten_tree(&current_tree) {
            if !is_dir {
                all_files.push(path);
            }
        }
        (all_files, Vec::new())
    };

    // 4) Copy only necessary files into the snapshot dir
    copy_selected_files(&root, &snapshot_dir, &to_copy, &bar)?;

    // 5) Save manifest in snapshot folder and update meta.json hash_tree
    storage::save_manifest(&snapshot_dir, &current_tree)?;
    let now = chrono::Local::now();
    config.snapshots.push(Snapshot {
        name: name.to_string(),
        directory: relative_path(&snapshot_dir, &root)?,
        kind: scope.kind,
        locked: password.as_deref().map(|p| !p.is_empty()).unwrap_or(false),
        created_at: now,
        scope: scope.name.clone(),
        password_hash: password.clone().map(|p| hash_password(&p)),
    });
    config.last_updated = chrono::Local::now();
    config.hash_tree = current_tree;
    storage::save_config(&root, &config)?;

    // 6) (Optional) delegate to drivers for DB etc.
    let drivers = select_drivers_for_target(&scope.target);
    for driver in drivers {
        bar.set_message(format!("Capturing {}", scope.name));
        if let Err(err) = driver.snapshot(&scope.target, &snapshot_dir, password.as_deref()) {
            eprintln!("{} {}: {}", "!".yellow().bold(), "Warning".yellow(), err);
        }
        bar.inc(1);
    }

    bar.finish_with_message("Snapshot created");
    println!(
        "{} {}",
        "✔".green().bold(),
        format!("Snapshot '{}' created", name).green()
    );
    Ok(())
}

pub fn do_rollback(
    global_scope: &Option<String>,
    name: Option<String>,
    latest: bool,
) -> Result<()> {
    let scope = registry::resolve_scope(global_scope)?;
    let root = std::path::Path::new(&scope.target).to_path_buf();
    let config = storage::load_config(&root)?;

    let snap = if latest {
        config
            .snapshots
            .iter()
            .filter(|s| s.scope == scope.name)
            .last()
            .ok_or_else(|| anyhow!("no snapshots available"))?
    } else {
        let name = name.ok_or_else(|| anyhow!("snapshot name required unless --latest"))?;
        config
            .snapshots
            .iter()
            .find(|s| s.name == name && s.scope == scope.name)
            .ok_or_else(|| anyhow!("snapshot '{}' not found", name))?
    };

    let snapshot_path = root.join(&snap.directory);
    let bar = create_progress_bar("Rolling back");

    // 1) Load snapshot manifest
    let snap_tree = storage::load_manifest(&snapshot_path)
        .map_err(|e| anyhow!("missing or invalid snapshot manifest: {}", e))?;

    // 2) Build current tree to compute minimal changes
    let skip = crate::utils::io::make_skipper(&root);
    let current_tree = build_merkle_tree(&root, "".into(), skip)
        .map_err(|e| anyhow!("failed to build current merkle tree: {}", e))?;

    // 3) Diff (we want to transform current → snapshot)
    let d = diff_trees(&snap_tree, &current_tree);
    // - For files added/modified in snapshot (relative to current), copy from snapshot to root
    // - For files deleted in snapshot (relative to current), delete from root
    let mut to_copy = d.added.clone();
    to_copy.extend(d.modified.iter().cloned());

    // 4) Perform minimal I/O
    // Copy from snapshot folder (which contains only changed files for that snapshot) *if present*,
    // otherwise fall back to the full snapshot path.
    copy_selected_files(&snapshot_path, &root, &to_copy, &bar)?;
    delete_selected_paths(&root, &d.deleted)?;

    // 5) (Optional) delegate to drivers, e.g., databases
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

    if let Some(hash) = snap
        .password_hash
        .as_ref()
        .or(config.password_hash.as_ref())
    {
        let password = &prompt_password(&format!("Enter password for snapshot '{}': ", name))?;
        if !verify_password(password, hash) {
            eprintln!(
                "{} {}",
                "!".yellow().bold(),
                "Password is incorrect; unable to delete snapshot".yellow()
            );
            return Ok(());
        }
    } else {
        if !prompt_confirm(&format!("Delete snapshot '{}'? [y/N] ", name))? {
            println!("Aborted.");
            return Ok(());
        }
    }

    let bar = create_progress_bar("Deleting snapshot");
    bar.set_message(name.to_string());
    if snap_path.exists() {
        std::fs::remove_dir_all(&snap_path)?;
    }

    config.snapshots.remove(index);
    config.last_updated = chrono::Local::now();
    storage::save_config(&root, &config)?;

    let _ = registry::cleanup_invalid_scopes();

    bar.finish_with_message("Snapshot deleted");
    println!(
        "{} {}",
        "✔".green().bold(),
        format!("Deleted snapshot '{}'", name).green()
    );
    Ok(())
}

pub fn do_drop(global_scope: &Option<String>) -> Result<()> {
    let scope = registry::resolve_scope(global_scope)?;
    let root = std::path::Path::new(&scope.target).to_path_buf();
    let config = storage::load_config(&root)?;

    println!(
        "{} {}",
        "!".yellow().bold(),
        format!(
            "WARNING: dropping scope '{}' will permanently delete all snapshots under it. This action cannot be undone.",
            scope.name
        ).yellow()
    );

    // Check if scope has a password guard
    if let Some(hash) = config.password_hash.as_ref() {
        let password = &prompt_password(&format!("Enter password for scope '{}': ", scope.name))?;
        if !verify_password(password, hash) {
            eprintln!(
                "{} {}",
                "!".yellow().bold(),
                "Password is incorrect; scope not dropped".yellow()
            );
            return Ok(());
        }
    } else {
        if !prompt_confirm(&format!("Drop scope '{}'? [y/N] ", scope.name))? {
            println!("Aborted.");
            return Ok(());
        }
    }

    let bar = create_progress_bar("Dropping scope");
    bar.set_message(scope.name.clone());

    // Delete .groundhog folder
    let gh_dir = root.join(".groundhog");
    if gh_dir.exists() {
        std::fs::remove_dir_all(&gh_dir)?;
    }

    // Delete .groundhogignore if it exists
    let gh_ignore = root.join(".groundhogignore");
    if gh_ignore.exists() {
        std::fs::remove_file(&gh_ignore)?;
    }

    // Remove from global registry
    let mut all = registry::cleanup_invalid_scopes()?;
    all.retain(|s| s.name != scope.name);
    registry::save_registry(&all)?;

    bar.finish_with_message("Scope dropped");
    println!(
        "{} {}",
        "✔".green().bold(),
        format!("Dropped scope '{}'", scope.name).green()
    );
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
        let kind = match s.kind {
            SnapshotKind::Filesystem => "filesystem",
            SnapshotKind::Database => "database",
        };
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

fn prompt_password(message: &str) -> Result<String> {
    print!("{} {}", "?".cyan().bold(), message.cyan());
    std::io::Write::flush(&mut std::io::stdout())?;
    let password = read_password()?; // input hidden
    Ok(password)
}

fn generate_scope_name(target: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(target.as_bytes());
    let digest = hasher.finalize();
    format!("scope-{}", &hex::encode(digest)[..8])
}

pub fn do_scopes() -> Result<()> {
    let scopes = registry::cleanup_invalid_scopes()?;
    let mut table = comfy_table::Table::new();
    table
        .load_preset(comfy_table::presets::UTF8_FULL)
        .set_header(vec!["Name", "Type", "Target", "Created"]);
    for s in &scopes {
        let kind = match s.kind {
            SnapshotKind::Filesystem => "filesystem",
            SnapshotKind::Database => "database",
        };
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
    for snap in cfg
        .snapshots
        .iter_mut()
        .filter(|s| s.scope == old_scope_obj.name)
    {
        snap.scope = new_name.to_string();
    }
    storage::save_config(&scope_root, &cfg)?;
    // Update global registry (clean + rename)
    let mut all = registry::cleanup_invalid_scopes()?;
    if let Some(s) = all.iter_mut().find(|s| s.name == old_scope_obj.name) {
        s.name = new_name.to_string();
    }
    registry::save_registry(&all)?;
    println!(
        "{} {} -> {}",
        "✔".green().bold(),
        old_scope_obj.name.green(),
        new_name.green()
    );
    Ok(())
}

pub fn do_version() {
    println!("{} {}", "groundhog".bold(), "0.1-alpha".cyan());
}
