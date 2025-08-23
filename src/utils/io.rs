use anyhow::Result;
use indicatif::ProgressBar;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// Load a .groundhogignore matcher from a scope root, if present.
fn load_groundhogignore(root: &Path) -> Option<Gitignore> {
    let ignore_file = root.join(".groundhogignore");
    if ignore_file.exists() {
        let mut builder = GitignoreBuilder::new(root);
        let _ = builder.add(ignore_file);
        if let Ok(gi) = builder.build() {
            return Some(gi);
        }
    }
    None
}

/// Recursively copy directory `from` → `to`, excluding files matched by .groundhogignore
/// plus any names in `exclude_names`.
pub fn copy_dir_recursive_excluding(
    from: &Path,
    to: &Path,
    bar: &ProgressBar,
    exclude_names: &[&str],
) -> Result<()> {
    if !to.exists() {
        fs::create_dir_all(to)?;
    }

    let gitignore = load_groundhogignore(from);
    let root = from.to_path_buf();

    let should_include = |e: &walkdir::DirEntry| {
        let name = match e.file_name().to_str() {
            Some(n) => n,
            None => return true,
        };

        // Always exclude explicit names (e.g. ".groundhog")
        if exclude_names.iter().any(|ex| name.eq_ignore_ascii_case(ex)) {
            return false;
        }

        // Special case: exclude `manifest.json` only at the root
        if name.eq_ignore_ascii_case("manifest.json") {
            if let Ok(rel) = e.path().strip_prefix(&root) {
                if rel.components().count() == 1 {
                    return false; // only root-level manifest.json
                }
            }
        }

        // Apply .groundhogignore matcher if present
        if let Some(ref gi) = gitignore {
            let path = e.path();
            if gi.matched_path_or_any_parents(path, e.file_type().is_dir()).is_ignore() {
                return false;
            }
        }

        true
    };

    for entry in WalkDir::new(from).into_iter().filter_entry(|e| should_include(e)) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let rel = match path.strip_prefix(from) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let dest = to.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest)?;
            bar.inc(1);
        }
    }
    Ok(())
}

/// Clean a directory except for explicitly listed names.
pub fn clean_dir_except(dir: &Path, except: &[&str]) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let keep = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| except.contains(&n))
            .unwrap_or(false);

        if keep {
            continue;
        }

        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}

/// Special case: copy everything except the .groundhog directory itself.
/// Still respects .groundhogignore.
/// Note: manifest.json is excluded only at root.
pub fn copy_dir_excluding_groundhog(from: &Path, to: &Path, bar: &ProgressBar) -> Result<()> {
    copy_dir_recursive_excluding(from, to, bar, &[".groundhog", ".groundhogignore"])
}

/// Special case: clean directory except .groundhog folder (rollback safety).
pub fn clean_dir_except_groundhog(from: &Path) -> Result<()> {
    clean_dir_except(from, &[".groundhog", ".groundhogignore"])
}

pub fn make_skipper(root: &Path) -> impl FnMut(&Path, bool) -> bool {
    let gitignore = super::io::load_groundhogignore(root);
    let root = root.to_path_buf();

    move |path: &Path, is_dir: bool| {
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            // Skip `.groundhog` and `.groundhogignore` everywhere
            if name.eq_ignore_ascii_case(".groundhog") || name.eq_ignore_ascii_case(".groundhogignore") {
                return true;
            }

            // Skip `manifest.json` only at root
            if name.eq_ignore_ascii_case("manifest.json") {
                if let Ok(rel) = path.strip_prefix(&root) {
                    if rel.components().count() == 1 {
                        return true;
                    }
                }
            }
        }

        // Apply ignore patterns if available
        if let Some(ref gi) = gitignore {
            if gi.matched_path_or_any_parents(path, is_dir).is_ignore() {
                return true;
            }
        }

        false
    }
}

/// Copy only selected files (relative paths from `flatten_tree`) from `from` → `to`.
pub fn copy_selected_files(from: &Path, to: &Path, files: &[String], bar: &ProgressBar) -> Result<()> {
    for rel in files {
        let src = from.join(rel);
        let dest = to.join(rel);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        if src.is_file() {
            fs::copy(&src, &dest)?;
            bar.inc(1);
        } else if src.is_dir() {
            fs::create_dir_all(&dest)?;
        }
    }
    Ok(())
}

/// Delete only the selected paths (relative) inside `root`.
pub fn delete_selected_paths(root: &Path, paths: &[String]) -> Result<()> {
    for rel in paths {
        let p = root.join(rel);
        if p.is_file() {
            let _ = fs::remove_file(&p);
        } else if p.is_dir() {
            let _ = fs::remove_dir_all(&p);
        }
    }
    Ok(())
}
