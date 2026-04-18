//! SHA-256 content hashing for drift detection.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A content hash computed from file data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileHash {
    /// The hex-encoded hash value.
    pub hex: String,
    /// The algorithm used (e.g. "sha256").
    pub algorithm: String,
}

/// Compute a SHA-256 hash of the given file contents.
///
/// # Examples
///
/// ```
/// use prism_core::hashing::hash_file;
///
/// let hash = hash_file(b"hello world");
/// assert_eq!(hash.hex.len(), 64);
/// assert_eq!(hash.hex, hash_file(b"hello world").hex);
/// ```
pub fn hash_file(contents: &[u8]) -> FileHash {
    let mut hasher = Sha256::new();
    hasher.update(contents);
    FileHash {
        hex: hex_encode(&hasher.finalize()),
        algorithm: "sha256".to_string(),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
