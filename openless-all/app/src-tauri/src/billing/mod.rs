//! ZeroType billing and quota system.
//!
//! ## Quota Model
//!
//! - **Free tier**: 10,000 characters per week (configurable via `WEEKLY_FREE_CHARS`)
//! - **Paid tier**: buy tokens for additional usage (Phase 2)
//! - **Admin override**: admin can adjust individual user quotas
//!
//! ## Weekly Reset
//!
//! Weeks start on Monday 00:00 UTC+8. When a new week begins, usage resets to 0.
//!
//! ## Integration
//!
//! Hooked into `end_session` in coordinator/dictation.rs: after successful transcription,
//! the char count of `final_text` is deducted from the user's quota.

pub mod store;
pub mod types;

pub use store::BillingStore;
pub use types::{QuotaAction, QuotaStatus, WeeklyQuota, WEEKLY_FREE_CHARS};
