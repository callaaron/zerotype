//! Hotword learning engine — extracts frequent phrases from dictation history
//! and suggests them as dictionary entries. Implements the "越用越聪明" (gets smarter
//! with use) promise.
//!
//! ## Architecture
//!
//! ```
//! DictationSession ──→ extract_ngrams() ──→ score_candidates()
//!                                              │
//!                        ┌─────────────────────┘
//!                        ▼
//!                  HotwordStore (JSON, separate from DictionaryStore)
//!                        │
//!                        ├── threshold met → auto-suggest to Dictionary
//!                        └── below threshold → accumulate score
//! ```
//!
//! ## Decay
//! Unconfirmed candidates lose 10% score every 7 days. Confirmed dictionary entries
//! that drop below 30% of their peak monthly hits get flagged for review.

pub mod learn;
pub mod scoring;
pub mod store;

use learn::CandidateTracker;
use store::HotwordStore;
