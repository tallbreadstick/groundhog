use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::utils::hash::hash_password;

#[derive(Serialize, Deserialize)]
pub struct GroundHogConfig {
    pub date_created: DateTime<Local>,
    pub last_updated: DateTime<Local>,
    pub snapshots: Vec<Snapshot>,
    pub hash_tree: TreeNode,
    pub password_hash: Option<String>, // NEW: workspace password
}

impl GroundHogConfig {
    pub fn new(password: Option<String>) -> Self {
        let now = Local::now();
        Self {
            date_created: now,
            last_updated: now,
            snapshots: Vec::new(),
            hash_tree: TreeNode { name: "".into(), hash: String::new(), is_dir: true, children: Some(Vec::new()) },
            password_hash: password.as_ref().map(|p| hash_password(p)),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub name: String,
    pub directory: String,
    pub kind: SnapshotKind,
    pub locked: bool,
    pub created_at: DateTime<Local>,
    pub scope: String,
    pub password_hash: Option<String>, // NEW: optional snapshot-level lock
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum SnapshotKind {
    Filesystem,
    Database,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Scope {
    pub name: String,
    pub target: String, // filesystem path or DB address
    pub kind: SnapshotKind,
    pub created_at: DateTime<Local>,
}

#[derive(Serialize, Deserialize)]
pub struct TreeNode {
    /// Entry name (file or directory). Root can be "".
    pub name: String,
    /// Content hash for files; for directories, hash over sorted child entries.
    pub hash: String,
    /// true = directory, false = file
    pub is_dir: bool,
    /// Children for directories (sorted by name for stability).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreeNode>>,
}