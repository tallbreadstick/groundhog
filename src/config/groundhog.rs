use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct GroundHogConfig {
    pub date_created: DateTime<Local>,
    pub last_updated: DateTime<Local>,
    pub snapshots: Vec<Snapshot>,
    pub hash_tree: TreeNode
}

impl GroundHogConfig {
    pub fn new() -> Self {
        let now = Local::now();
        Self {
            date_created: now,
            last_updated: now,
            snapshots: Vec::new(),
            hash_tree: TreeNode { hash: String::new(), children: None },
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
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreeNode>>,
}
