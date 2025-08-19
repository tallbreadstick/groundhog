use anyhow::Result;
use std::path::Path;

use super::BackendDriver;
use crate::utils::io;

pub struct FilesystemDriver;

impl BackendDriver for FilesystemDriver {
    fn name(&self) -> &'static str { "filesystem" }

    fn snapshot(&self, scope_target: &str, snapshot_dir: &Path, _password: Option<&str>) -> Result<()> {
        let src = std::path::Path::new(scope_target);
        io::copy_dir_excluding_groundhog(src, snapshot_dir, &indicatif::ProgressBar::hidden())
    }

    fn rollback(&self, scope_target: &str, snapshot_dir: &Path) -> Result<()> {
        let dst = std::path::Path::new(scope_target);
        io::clean_dir_except_groundhog(dst)?;
        io::copy_dir_excluding_groundhog(snapshot_dir, dst, &indicatif::ProgressBar::hidden())
    }
}


