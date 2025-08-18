use anyhow::Result;
use std::path::Path;

pub mod filesystem;
pub mod mysql;
pub mod postgres;
pub mod sqlite;
pub mod selector;

pub trait BackendDriver: Send + Sync {
    fn name(&self) -> &'static str;

    /// Capture a snapshot of the target into the provided snapshot directory.
    /// Implement minimal I/O by copying only changed files when possible.
    fn snapshot(&self, scope_target: &str, snapshot_dir: &Path, password: Option<&str>) -> Result<()>;

    /// Restore the workspace to the given snapshot. Implement minimal I/O.
    fn rollback(&self, scope_target: &str, snapshot_dir: &Path) -> Result<()>;
}


