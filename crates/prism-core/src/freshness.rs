//! Freshness scoring for PRISM context items.
//!
//! Tracks how current a context item is based on turn-based aging.
//! The freshness ratio is: `(current_turn - last_read_turn) / ttl_turns`.
//!
//! - **Fresh**: ratio ≤ 0.7
//! - **Aging**: ratio > 0.7 and ≤ 1.0
//! - **Stale**: ratio > 1.0
//!
//! Adaptive TTL adjusts based on access frequency: items accessed more
//! frequently get longer TTLs (they're clearly useful).

use serde::{Deserialize, Serialize};

/// Discrete freshness level for a context item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FreshnessScore {
    /// Item was recently accessed and is current (ratio ≤ 0.7).
    Fresh,
    /// Item has not been accessed in a while and may need review (0.7 < ratio ≤ 1.0).
    Aging,
    /// Item is significantly out of date and should be refreshed (ratio > 1.0).
    Stale,
}

/// A freshness record for a single context item, tracking turn-based aging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessRecord {
    /// Path or ID of the context item.
    pub path: String,
    /// Turn at which this item was last read/accessed.
    pub last_read_turn: u32,
    /// Turn at which this item was last modified.
    pub last_modified_turn: u32,
    /// Time-to-live in turns before the item becomes stale.
    pub ttl_turns: u32,
    /// Number of times this item has been accessed in the current session.
    pub access_count: u32,
    /// Content hash at time of last freshness check.
    pub content_hash: Option<String>,
}

/// The freshness ratio and its classification.
#[derive(Debug, Clone)]
pub struct FreshnessResult {
    /// The raw ratio value: (current_turn - last_read_turn) / ttl_turns.
    pub ratio: f64,
    /// The classified score.
    pub score: FreshnessScore,
}

/// Thresholds for freshness classification.
#[derive(Debug, Clone)]
pub struct FreshnessThresholds {
    /// Ratio at or below which an item is Fresh (default: 0.7).
    pub aging_threshold: f64,
    /// Ratio at or below which an item is Aging; above is Stale (default: 1.0).
    pub stale_threshold: f64,
}

impl Default for FreshnessThresholds {
    fn default() -> Self {
        Self {
            aging_threshold: 0.7,
            stale_threshold: 1.0,
        }
    }
}

/// Calculate the freshness score for a context item given its last-read turn
/// and the current turn.
///
/// Returns the ratio and classified score. A TTL of 0 is treated as immediately stale.
#[tracing::instrument(skip_all, fields(last_read_turn, current_turn, ttl_turns))]
pub fn calculate_freshness(
    last_read_turn: u32,
    current_turn: u32,
    ttl_turns: u32,
    thresholds: &FreshnessThresholds,
) -> FreshnessResult {
    if ttl_turns == 0 {
        return FreshnessResult {
            ratio: f64::INFINITY,
            score: FreshnessScore::Stale,
        };
    }

    let elapsed = current_turn.saturating_sub(last_read_turn);
    let ratio = f64::from(elapsed) / f64::from(ttl_turns);

    let score = classify_ratio(ratio, thresholds);

    FreshnessResult { ratio, score }
}

/// Classify a freshness ratio into a discrete score.
#[tracing::instrument(skip_all)]
pub fn classify_ratio(ratio: f64, thresholds: &FreshnessThresholds) -> FreshnessScore {
    if ratio <= thresholds.aging_threshold {
        FreshnessScore::Fresh
    } else if ratio <= thresholds.stale_threshold {
        FreshnessScore::Aging
    } else {
        FreshnessScore::Stale
    }
}

/// Compute an adaptive TTL based on access frequency.
///
/// Items accessed more frequently get longer TTLs (they're clearly useful and
/// shouldn't be evicted). The formula is:
///
/// `adaptive_ttl = base_ttl + (access_count * boost_per_access)`
///
/// Capped at `max_ttl` to prevent unbounded growth.
#[tracing::instrument(skip_all, fields(base_ttl, access_count))]
pub fn adaptive_ttl(base_ttl: u32, access_count: u32, boost_per_access: u32, max_ttl: u32) -> u32 {
    let boosted = base_ttl.saturating_add(access_count.saturating_mul(boost_per_access));
    boosted.min(max_ttl)
}

/// Check if a freshness record should be marked stale based on content hash change.
///
/// Returns `true` if the stored hash differs from the current hash, indicating
/// the underlying content changed and the freshness record is outdated.
#[tracing::instrument(skip_all)]
pub fn content_changed(record: &FreshnessRecord, current_hash: &str) -> bool {
    match &record.content_hash {
        Some(stored) => stored != current_hash,
        None => true, // No stored hash means we can't confirm freshness.
    }
}
