use anyhow::{anyhow, Result};
use std::path::Path;

use super::BackendDriver;

pub struct PostgresDriver;

impl BackendDriver for PostgresDriver {
    fn name(&self) -> &'static str { "postgres" }

    fn snapshot(&self, _scope_target: &str, _snapshot_dir: &Path, _password: Option<&str>) -> Result<()> {
        // TODO: Implement locating Postgres data dir (if local) or perform logical dump for remote
        Err(anyhow!("PostgreSQL driver not yet implemented"))
    }

    fn rollback(&self, _scope_target: &str, _snapshot_dir: &Path) -> Result<()> {
        // TODO: Implement rollback from physical/logical snapshot
        Err(anyhow!("PostgreSQL driver not yet implemented"))
    }
}


