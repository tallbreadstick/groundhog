use super::{filesystem::FilesystemDriver, mysql::MySqlDriver, postgres::PostgresDriver, sqlite::SqliteDriver, BackendDriver};
use std::sync::Arc;

pub fn select_drivers_for_target(target: &str) -> Vec<Arc<dyn BackendDriver>> {
    let mut drivers: Vec<Arc<dyn BackendDriver>> = Vec::new();

    if target.starts_with("mysql://") {
        drivers.push(Arc::new(MySqlDriver));
    } else if target.starts_with("postgres://") || target.starts_with("postgresql://") {
        drivers.push(Arc::new(PostgresDriver));
    } else if target.ends_with(".sqlite") || target.starts_with("sqlite://") {
        drivers.push(Arc::new(SqliteDriver));
    } else {
        drivers.push(Arc::new(FilesystemDriver));
    }

    drivers
}


