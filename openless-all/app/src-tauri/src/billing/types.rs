//! Billing types.

use serde::{Deserialize, Serialize};
use chrono::Datelike;

/// Default weekly free char quota.
pub const WEEKLY_FREE_CHARS: u64 = 10_000;

/// A user's quota record for a specific week.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyQuota {
    pub user_id: String,
    /// ISO week identifier: "2026-W30"
    pub week_id: String,
    /// Characters used this week.
    pub chars_used: u64,
    /// Override limit (e.g. admin grant). 0 = use default free tier.
    pub override_limit: u64,
    /// Whether the user has purchased premium.
    pub is_premium: bool,
}

/// Quota status returned to frontend.
#[derive(Debug, Clone, Serialize)]
pub struct QuotaStatus {
    pub chars_used: u64,
    pub chars_limit: u64,
    pub chars_remaining: i64,
    pub week_id: String,
    pub is_over_limit: bool,
    pub percent_used: f64,
}

/// What to do after quota check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaAction {
    /// Allow the transcription.
    Allow,
    /// Block: quota exceeded.
    Block,
}

/// Get the current ISO week identifier (e.g. "2026-W30")
pub fn current_week_id() -> String {
    let now = chrono::Local::now();
    let iso = now.iso_week();
    format!("{}-W{:02}", iso.year(), iso.week())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_week_id_format() {
        let id = current_week_id();
        assert!(id.contains("-W"), "Expected -W in week id: {}", id);
    }
}
