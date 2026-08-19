//! 每周免费配额（sqlx 持久化：SQLite 起步，Postgres 可切换）。

use serde::Serialize;
use sqlx::SqlitePool;

/// 每周免费字数（对标 TypeLess 8,000 词/周，ZeroType 用 10,000 字）。
pub const WEEKLY_FREE_CHARS: u64 = 10_000;

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

/// 配额存储（进程内连接池 + SQLite 表）。
#[derive(Clone)]
pub struct QuotaStore {
    pool: SqlitePool,
}

impl QuotaStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 启动时建表（幂等）。
    pub async fn init(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS quota (
                user_id TEXT PRIMARY KEY,
                week_id TEXT NOT NULL,
                chars_used INTEGER NOT NULL DEFAULT 0,
                chars_limit INTEGER NOT NULL DEFAULT 10000
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 当前周配额状态（跨周自动重置）。
    pub async fn status(&self, user_id: &str) -> WeekUsage {
        let week = current_week_id();
        let row = sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT week_id, chars_used, chars_limit FROM quota WHERE user_id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();
        match row {
            Some((stored_week, used, limit)) if stored_week == week => WeekUsage {
                week_id: week,
                chars_used: used.max(0) as u64,
                chars_limit: limit.max(0) as u64,
            },
            _ => WeekUsage {
                week_id: week,
                chars_used: 0,
                chars_limit: WEEKLY_FREE_CHARS,
            },
        }
    }

    /// 记录使用并返回最新状态。跨周时重置计数。超额不阻断（Phase 2 接入支付）。
    pub async fn record(&self, user_id: &str, chars: u64) -> WeekUsage {
        let week = current_week_id();
        let current = self.status(user_id).await;
        let used = if current.week_id == week { current.chars_used } else { 0 };
        let new_used = used.saturating_add(chars);
        let _ = sqlx::query(
            "INSERT INTO quota (user_id, week_id, chars_used, chars_limit) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(user_id) DO UPDATE SET week_id = excluded.week_id,
             chars_used = excluded.chars_used, chars_limit = excluded.chars_limit",
        )
        .bind(user_id)
        .bind(&week)
        .bind(new_used as i64)
        .bind(WEEKLY_FREE_CHARS as i64)
        .execute(&self.pool)
        .await;
        WeekUsage {
            week_id: week,
            chars_used: new_used,
            chars_limit: WEEKLY_FREE_CHARS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn weekly_quota_tracks_and_resets() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = QuotaStore::new(pool);
        store.init().await.unwrap();
        let s1 = store.status("u1").await;
        assert_eq!(s1.chars_limit, WEEKLY_FREE_CHARS);
        let s2 = store.record("u1", 2500).await;
        assert_eq!(s2.chars_used, 2500);
        assert_eq!(s2.chars_remaining(), 7500);
        let s3 = store.record("u1", WEEKLY_FREE_CHARS).await;
        assert!(s3.is_over_limit());
        // 持久化：新 store 实例读同一库仍可见。
        let store2 = QuotaStore::new(store.pool.clone());
        let s4 = store2.status("u1").await;
        assert_eq!(s4.chars_used, WEEKLY_FREE_CHARS + 2500);
    }
}
