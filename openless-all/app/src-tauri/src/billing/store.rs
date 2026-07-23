//! BillingStore — quota tracking and usage enforcement.
//!
//! Stores data in `zerotype-billing.json` in the app data directory.

use std::path::PathBuf;

use anyhow::{Context, Result};
use parking_lot::Mutex;

use super::types::*;
use crate::persistence::{atomic_write, data_dir, ensure_dir, read_or_default};

const BILLING_FILE: &str = "zerotype-billing.json";

/// Billing data file format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BillingFile {
    quotas: Vec<WeeklyQuota>,
}

impl Default for BillingFile {
    fn default() -> Self {
        Self {
            quotas: Vec::new(),
        }
    }
}

use serde::{Deserialize, Serialize};

pub struct BillingStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl BillingStore {
    pub fn new() -> Result<Self> {
        let dir = data_dir()?;
        ensure_dir(&dir)?;
        Ok(Self {
            path: dir.join(BILLING_FILE),
            lock: Mutex::new(()),
        })
    }

    /// Check if a user has remaining quota for a given number of characters.
    /// Returns QuotaAction::Allow or QuotaAction::Block.
    pub fn check_quota(&self, user_id: &str, chars_to_add: u64) -> Result<QuotaAction, String> {
        let week_id = current_week_id();
        let _guard = self.lock.lock();
        let file = self.read()?;

        let quota = file
            .quotas
            .iter()
            .find(|q| q.user_id == user_id && q.week_id == week_id);

        let current_usage = quota.map(|q| q.chars_used).unwrap_or(0);
        let limit = quota
            .and_then(|q| if q.override_limit > 0 { Some(q.override_limit) } else { None })
            .unwrap_or(WEEKLY_FREE_CHARS);

        if current_usage + chars_to_add > limit {
            Ok(QuotaAction::Block)
        } else {
            Ok(QuotaAction::Allow)
        }
    }

    /// Record character usage after a successful transcription.
    pub fn record_usage(&self, user_id: &str, char_count: u64) -> Result<QuotaStatus, String> {
        let week_id = current_week_id();
        let _guard = self.lock.lock();
        let mut file = self.read()?;

        // Find or create quota record
        let quota_idx = file
            .quotas
            .iter()
            .position(|q| q.user_id == user_id && q.week_id == week_id);

        let quota = if let Some(idx) = quota_idx {
            &mut file.quotas[idx]
        } else {
            file.quotas.push(WeeklyQuota {
                user_id: user_id.to_string(),
                week_id: week_id.clone(),
                chars_used: 0,
                override_limit: 0,
                is_premium: false,
            });
            file.quotas.last_mut().unwrap()
        };

        quota.chars_used = quota.chars_used.saturating_add(char_count);
        let limit = if quota.override_limit > 0 {
            quota.override_limit
        } else {
            WEEKLY_FREE_CHARS
        };
        let chars_used = quota.chars_used;

        // Release borrow before writing
        drop(quota);
        self.write(&file)?;

        let remaining = limit as i64 - chars_used as i64;
        let percent = (chars_used as f64 / limit as f64 * 100.0).min(100.0);

        Ok(QuotaStatus {
            chars_used,
            chars_limit: limit,
            chars_remaining: remaining.max(0),
            week_id: week_id.clone(),
            is_over_limit: chars_used > limit,
            percent_used: percent,
        })
    }

    /// Get current quota status for a user.
    pub fn get_quota_status(&self, user_id: &str) -> Result<QuotaStatus, String> {
        let week_id = current_week_id();
        let _guard = self.lock.lock();
        let file = self.read()?;

        let quota = file
            .quotas
            .iter()
            .find(|q| q.user_id == user_id && q.week_id == week_id);

        let chars_used = quota.map(|q| q.chars_used).unwrap_or(0);
        let limit = quota
            .and_then(|q| if q.override_limit > 0 {
                Some(q.override_limit)
            } else {
                None
            })
            .unwrap_or(WEEKLY_FREE_CHARS);

        let remaining = limit as i64 - chars_used as i64;

        Ok(QuotaStatus {
            chars_used,
            chars_limit: limit,
            chars_remaining: remaining.max(0),
            week_id: week_id.clone(),
            is_over_limit: chars_used > limit,
            percent_used: (chars_used as f64 / limit as f64 * 100.0).min(100.0),
        })
    }

    /// Admin: set override limit for a user.
    pub fn set_override_limit(&self, user_id: &str, limit: u64) -> Result<(), String> {
        let week_id = current_week_id();
        let _guard = self.lock.lock();
        let mut file = self.read()?;

        if let Some(quota) = file
            .quotas
            .iter_mut()
            .find(|q| q.user_id == user_id && q.week_id == week_id)
        {
            quota.override_limit = limit;
        } else {
            file.quotas.push(WeeklyQuota {
                user_id: user_id.to_string(),
                week_id,
                chars_used: 0,
                override_limit: limit,
                is_premium: false,
            });
        }

        self.write(&file).map_err(|e| e.to_string())
    }

    /// Clean up quota records older than 12 weeks.
    pub fn cleanup(&self) -> Result<usize, String> {
        let _guard = self.lock.lock();
        let mut file = self.read()?;
        let before = file.quotas.len();

        // Keep records from the last 12 weeks
        let current = current_week_id();
        file.quotas
            .retain(|q| is_week_within_range(&q.week_id, &current, 12));

        let removed = before - file.quotas.len();
        if removed > 0 {
            self.write(&file).map_err(|e| e.to_string())?;
        }
        Ok(removed)
    }

    fn read(&self) -> Result<BillingFile, String> {
        read_or_default::<BillingFile>(&self.path).map_err(|e| e.to_string())
    }

    fn write(&self, file: &BillingFile) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(file).map_err(|e| format!("encode billing failed: {e}"))?;
        atomic_write(&self.path, &json).map_err(|e| e.to_string())
    }
}

/// Check if a week_id is within `range` weeks of `current`.
fn is_week_within_range(week_id: &str, current: &str, range: i64) -> bool {
    // Parse "YYYY-Www" → (year, week)
    let parse = |s: &str| -> Option<(i32, u32)> {
        let (year_str, week_str) = s.split_once("-W")?;
        Some((year_str.parse().ok()?, week_str.parse().ok()?))
    };

    let (cy, cw) = match parse(current) {
        Some(v) => v,
        None => return true, // Keep if can't parse
    };
    let (wy, ww) = match parse(week_id) {
        Some(v) => v,
        None => return false,
    };

    let current_abs = cy as i64 * 52 + cw as i64;
    let week_abs = wy as i64 * 52 + ww as i64;
    current_abs - week_abs <= range
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_check_allow() {
        // Unit test for quota logic
        let store = BillingStore::new_for_test();
        let status = store.record_usage("test-user", 1000).unwrap();
        assert!(!status.is_over_limit);
        assert_eq!(status.chars_used, 1000);
        assert_eq!(status.chars_limit, WEEKLY_FREE_CHARS);
    }

    #[test]
    fn test_quota_check_block() {
        let store = BillingStore::new_for_test();
        // Simulate heavy usage
        let status = store.record_usage("test-user", WEEKLY_FREE_CHARS + 1).unwrap();
        assert!(status.is_over_limit);
    }

    // Helper for testing
    impl BillingStore {
        fn new_for_test() -> Self {
            let tmp = std::env::temp_dir().join(format!("zerotype-billing-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&tmp).ok();
            Self {
                path: tmp.join(BILLING_FILE),
                lock: Mutex::new(()),
            }
        }
    }
}
