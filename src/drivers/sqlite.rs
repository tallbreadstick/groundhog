use anyhow::{anyhow, Result};
use std::path::Path;

use super::BackendDriver;

pub struct SqliteDriver;

impl BackendDriver for SqliteDriver {
    fn name(&self) -> &'static str { "sqlite" }

    fn snapshot(&self, _scope_target: &str, _snapshot_dir: &Path, _password: Option<&str>) -> Result<()> {
        // TODO: Implement SQLite file snapshot (copy .sqlite file and any WAL/SHM files)
        Err(anyhow!("SQLite driver not yet implemented"))
    }

    fn rollback(&self, _scope_target: &str, _snapshot_dir: &Path) -> Result<()> {
        // TODO: Implement rollback by restoring DB files
        Err(anyhow!("SQLite driver not yet implemented"))
    }
}


