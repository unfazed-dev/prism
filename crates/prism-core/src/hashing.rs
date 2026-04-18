//! Content hashing and Merkle tree construction.
//!
//! Provides SHA-256 based content hashing for files and strings, along with
//! a [`MerkleTree`] for efficient change detection across document hierarchies.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A content hash computed from file or string data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileHash {
    /// The hex-encoded hash value.
    pub hex: String,
    /// The algorithm used (e.g. "sha256").
    pub algorithm: String,
}

/// A Merkle tree built from a collection of content hashes.
///
/// Used for efficient comparison of document hierarchies — if the root hash
/// matches, the entire subtree is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    /// Root hash of the tree.
    pub root: String,
    /// Ordered leaf hashes that were combined to produce the root.
    pub leaves: Vec<String>,
}

impl MerkleTree {
    /// Build a Merkle tree from a list of hex-encoded leaf hashes.
    ///
    /// For odd numbers of nodes at any level, the last node is duplicated
    /// before pairing. An empty input produces an empty root.
    pub fn from_hashes(leaves: Vec<String>) -> Self {
        let root = compute_merkle_root(&leaves);
        MerkleTree { root, leaves }
    }

    /// Return the root hash of the tree.
    pub fn root_hash(&self) -> &str {
        &self.root
    }
}

/// Compute a SHA-256 hash of the given file contents and return a [`FileHash`].
///
/// # Examples
///
/// ```
/// use prism_core::hashing::hash_file;
///
/// let hash = hash_file(b"hello world");
/// assert_eq!(hash.hex.len(), 64); // SHA-256 produces 64 hex chars
/// assert_eq!(hash.hex, hash_file(b"hello world").hex); // deterministic
/// ```
#[tracing::instrument(skip(contents), fields(size = contents.len()))]
pub fn hash_file(contents: &[u8]) -> FileHash {
    let mut hasher = Sha256::new();
    hasher.update(contents);
    let result = hasher.finalize();
    FileHash {
        hex: hex_encode(&result),
        algorithm: "sha256".to_string(),
    }
}

/// Compute a SHA-256 hash of a string and return the hex-encoded result.
#[tracing::instrument(skip(input), fields(len = input.len()))]
pub fn hash_string(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Encode a byte slice as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Iteratively compute the Merkle root from a slice of hex-encoded hashes.
fn compute_merkle_root(leaves: &[String]) -> String {
    if leaves.is_empty() {
        return String::new();
    }

    let mut current_level: Vec<String> = leaves.to_vec();

    while current_level.len() > 1 {
        // Duplicate the last element when the count is odd.
        if !current_level.len().is_multiple_of(2) {
            let last = current_level
                .last()
                .expect("non-empty: while guard ensures len > 1")
                .clone();
            current_level.push(last);
        }

        let mut next_level = Vec::with_capacity(current_level.len() / 2);
        for pair in current_level.chunks(2) {
            let combined = format!("{}{}", pair[0], pair[1]);
            next_level.push(hash_string(&combined));
        }
        current_level = next_level;
    }

    current_level
        .into_iter()
        .next()
        .expect("non-empty: while loop exits when len == 1")
}
