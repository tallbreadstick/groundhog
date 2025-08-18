use anyhow::{anyhow, Result};
use std::path::Path;

use super::BackendDriver;

pub struct MySqlDriver;

impl BackendDriver for MySqlDriver {
    fn name(&self) -> &'static str { "mysql" }

    fn snapshot(&self, _scope_target: &str, _snapshot_dir: &Path, _password: Option<&str>) -> Result<()> {
        // TODO: Implement locating MySQL data dir (if local) or perform logical dump for remote
        // For now, act as a stub
        Err(anyhow!("MySQL driver not yet implemented"))
    }

    fn rollback(&self, _scope_target: &str, _snapshot_dir: &Path) -> Result<()> {
        // TODO: Implement rollback from physical/logical snapshot
        Err(anyhow!("MySQL driver not yet implemented"))
    }
}


