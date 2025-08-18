use anyhow::Result;
use indicatif::ProgressBar;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn copy_dir_recursive_excluding(from: &Path, to: &Path, bar: &ProgressBar, exclude_names: &[&str]) -> Result<()> {
    if !to.exists() {
        fs::create_dir_all(to)?;
    }

    let should_include = |e: &walkdir::DirEntry| {
        let name = match e.file_name().to_str() { Some(n) => n, None => return true };
        !exclude_names.iter().any(|ex| name.eq_ignore_ascii_case(ex))
    };

    for entry in WalkDir::new(from).into_iter().filter_entry(|e| should_include(e)) {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let path = entry.path();
        let rel = match path.strip_prefix(from) { Ok(r) => r, Err(_) => continue };
        let dest = to.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() { fs::create_dir_all(parent)?; }
            fs::copy(path, &dest)?;
            bar.inc(1);
        }
    }
    Ok(())
}

pub fn copy_dir_excluding_groundhog(from: &Path, to: &Path, bar: &ProgressBar) -> Result<()> {
    copy_dir_recursive_excluding(from, to, bar, &[".groundhog"])
}


