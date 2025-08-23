use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File};
use std::io::{Read, Result as IoResult};
use std::path::Path;

use crate::config::groundhog::TreeNode;

pub fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)  // hex crate: converts bytes → hex string
}

/// Verify a password against a stored hash
pub fn verify_password(password: &str, expected_hash: &str) -> bool {
    let hash = hash_password(password);
    hash == expected_hash
}

pub fn sha256_file(path: &Path) -> IoResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Hash arbitrary bytes → hex
fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Compute a stable directory hash from a map of (name -> child_hash, kind).
/// Format: "tree\0{name1}:{hash1}:{k1}\n{name2}:{hash2}:{k2}\n..."
/// where k= "d" for dir, "f" for file. Sorted by name.
fn hash_dir_index(index: &BTreeMap<String, (String, char)>) -> String {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"tree\0");
    for (name, (h, k)) in index {
        buf.extend_from_slice(name.as_bytes());
        buf.push(b':');
        buf.extend_from_slice(h.as_bytes());
        buf.push(b':');
        buf.push(*k as u8);
        buf.push(b'\n');
    }
    sha256_bytes(&buf)
}

/// Build a Merkle tree for `root`, excluding anything for which `should_skip(path, is_dir)` returns true.
/// `display_name` is the node name to record (usually "" for root).
pub fn build_merkle_tree<F>(root: &Path, display_name: String, mut should_skip: F) -> IoResult<TreeNode>
where
    F: FnMut(&Path, bool) -> bool
{
    fn build<F>(abs: &Path, name: String, should_skip: &mut F) -> IoResult<TreeNode>
    where
        F: FnMut(&Path, bool) -> bool
    {
        let md = fs::metadata(abs)?;
        let is_dir = md.is_dir();

        if should_skip(abs, is_dir) {
            // Represent skipped paths by an empty node with empty hash, so parents can still compute.
            return Ok(TreeNode { name, hash: String::new(), is_dir, children: if is_dir { Some(Vec::new()) } else { None }});
        }

        if !is_dir {
            let h = sha256_file(abs)?;
            Ok(TreeNode { name, hash: h, is_dir: false, children: None })
        } else {
            // Build children, skip empty nodes
            let mut entries: Vec<(String, TreeNode)> = Vec::new();
            for entry in fs::read_dir(abs)? {
                let entry = entry?;
                let p = entry.path();
                let n = entry.file_name().to_string_lossy().to_string();
                let node = build(&p, n.clone(), should_skip)?;
                // skip nodes with empty hash only if they are empty-skip placeholders
                if !(node.hash.is_empty() && node.is_dir && node.children.as_ref().map(|c| c.is_empty()).unwrap_or(true)) {
                    entries.push((n, node));
                }
            }
            // sort by name for stable hashing
            entries.sort_by(|a, b| a.0.cmp(&b.0));

            // build index for hashing
            let mut index: BTreeMap<String, (String, char)> = BTreeMap::new();
            let mut kids: Vec<TreeNode> = Vec::with_capacity(entries.len());
            for (n, node) in entries {
                index.insert(n.clone(), (node.hash.clone(), if node.is_dir { 'd' } else { 'f' }));
                kids.push(node);
            }
            let h = hash_dir_index(&index);
            Ok(TreeNode { name, hash: h, is_dir: true, children: Some(kids) })
        }
    }

    build(root, display_name, &mut should_skip)
}

/// Flatten a tree into a map path -> (hash, is_dir). Paths are slash-separated relative paths (no leading slash).
pub fn flatten_tree(tree: &TreeNode) -> BTreeMap<String, (String, bool)> {
    let mut out = BTreeMap::new();
    let mut q: VecDeque<(String, &TreeNode)> = VecDeque::new();
    q.push_back(("".into(), tree));

    while let Some((prefix, node)) = q.pop_front() {
        let path = if prefix.is_empty() || node.name.is_empty() {
            // root
            "".to_string()
        } else if prefix.is_empty() {
            node.name.clone()
        } else {
            format!("{}/{}", prefix, node.name)
        };

        if !(path.is_empty()) {
            out.insert(path.clone(), (node.hash.clone(), node.is_dir));
        }
        if node.is_dir {
            if let Some(children) = node.children.as_ref() {
                let child_prefix = path;
                for child in children {
                    q.push_back((child_prefix.clone(), child));
                }
            }
        }
    }
    out
}

#[derive(Debug, Default)]
pub struct Diff {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
}

/// Compare two trees (left = current, right = baseline) and return changes to turn baseline→current.
/// - added: present in current, absent in baseline
/// - deleted: present in baseline, absent in current
/// - modified: present in both but hashes differ (file content or subtree changed)
pub fn diff_trees(current: &TreeNode, baseline: &TreeNode) -> Diff {
    let mut d = Diff::default();
    let a = flatten_tree(current);
    let b = flatten_tree(baseline);

    // additions + modifications
    for (path, (h, _is_dir)) in &a {
        match b.get(path) {
            None => d.added.push(path.clone()),
            Some((h_old, _)) => {
                if h_old != h {
                    d.modified.push(path.clone());
                }
            }
        }
    }
    // deletions
    for (path, _) in &b {
        if !a.contains_key(path) {
            d.deleted.push(path.clone());
        }
    }
    d
}