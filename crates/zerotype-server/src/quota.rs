//! 每周免费配额（进程内存储，Postgres 待接入）。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

/// 每周免费字数（对标 TypeLess 8,000 词/周，ZeroType 用 10,000 字）。
pub const WEEKLY_FREE_CHARS: u64 = 10_000;

#[derive(Default)]
pub struct QuotaStore {
    inner: Mutex<HashMap<String, WeekUsage>>, // key: user_id
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WeekUsage {
    pub week_id: String,
    pub chars_used: u64,
    pub chars_limit: u64,
}

impl WeekUsage {
    pub fn chars_remaining(&self) -> u64 {
        self.chars_limit.saturating_sub(self.chars_used)
    }
    pub fn is_over_limit(&self) -> bool {
        self.chars_used >= self.chars_limit
    }
}

fn current_week_id() -> String {
    chrono::Utc::now().format("%G-W%V").to_string()
}

impl QuotaStore {
    /// 当前周配额状态（跨周自动重置）。
    pub fn status(&self, user_id: &str) -> WeekUsage {
        let mut store = self.inner.lock().unwrap();
        let week = current_week_id();
        let usage = store.entry(user_id.to_string()).or_insert_with(|| WeekUsage {
            week_id: week.clone(),
            chars_used: 0,
            chars_limit: WEEKLY_FREE_CHARS,
        });
        if usage.week_id != week {
            usage.week_id = week;
            usage.chars_used = 0;
        }
        usage.clone()
    }

    /// 记录使用并返回是否超额。超额不阻断（Phase 2 再接入支付升级）。
    pub fn record(&self, user_id: &str, chars: u64) -> WeekUsage {
        let mut store = self.inner.lock().unwrap();
        let week = current_week_id();
        let usage = store.entry(user_id.to_string()).or_insert_with(|| WeekUsage {
            week_id: week.clone(),
            chars_used: 0,
            chars_limit: WEEKLY_FREE_CHARS,
        });
        if usage.week_id != week {
            usage.week_id = week;
            usage.chars_used = 0;
        }
        usage.chars_used = usage.chars_used.saturating_add(chars);
        usage.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_quota_tracks_and_resets() {
        let store = QuotaStore::default();
        let s1 = store.status("u1");
        assert_eq!(s1.chars_limit, WEEKLY_FREE_CHARS);
        let s2 = store.record("u1", 2500);
        assert_eq!(s2.chars_used, 2500);
        assert_eq!(s2.chars_remaining(), 7500);
        let s3 = store.record("u1", WEEKLY_FREE_CHARS);
        assert!(s3.is_over_limit());
    }
}
