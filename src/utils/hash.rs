use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Result as IoResult};
use std::path::Path;

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

// Placeholder: In the future, build a Merkle-like tree over directories where each node
// contains the hash of its children and file contents. Use this to compute diffs quickly
// and decide minimal I/O for snapshot/rollback.


