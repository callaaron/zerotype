//! Scoring and decay logic for hotword candidates.
//!
//! ## Scoring Formula
//!
//! ```text
//! score = frequency_weight * 0.6 + recency_weight * 0.3 + uniqueness_weight * 0.1
//!
//! frequency_weight = min(occurrences / 5.0, 1.0)
//! recency_weight   = exp(-days_since_last_seen / 7.0)
//! uniqueness_weight = 1.0 - common_word_penalty
//! ```
//!
//! ## Decay
//!
//! - Unconfirmed candidates: `score *= 0.9` every 7 days without a hit
//! - Confirmed (promoted to dictionary): no decay, use DictionaryEntry.hits instead
//!
//! ## Thresholds
//!
//! - `AUTO_PROMOTE`: score ≥ 0.75 → auto-add to dictionary
//! - `SUGGEST`: 0.45 ≤ score < 0.75 → show in suggestions UI
//! - `DISCARD`: score < 0.10 AND age > 30 days → remove
//!
//! ## Common Words (Stoplist)
//!
//! Words that should never become hotword candidates even if frequent:
//! 的、了、是、在、我、你、他、她、它、们、这、那、不、就、也、都、还、很、要、能、会、
//! 可以、应该、因为、所以、但是、虽然、如果、而且、或者、然后、不过、the, a, an, is, are,
//! was, were, be, been, have, has, had, do, does, did, will, would, can, could,
//! to, of, in, on, at, for, with, by, from, this, that, it, and, or, but, so

use serde::{Deserialize, Serialize};

/// Threshold above which a candidate is automatically promoted to dictionary.
pub const AUTO_PROMOTE_THRESHOLD: f64 = 0.75;

/// Threshold above which a candidate is shown in suggestion UI.
pub const SUGGEST_THRESHOLD: f64 = 0.45;

/// Threshold below which a candidate is discarded (if aged > 30 days).
pub const DISCARD_THRESHOLD: f64 = 0.10;

/// Maximum age (days) of a low-scoring candidate before cleanup.
pub const MAX_CANDIDATE_AGE_DAYS: i64 = 30;

/// Decay factor for unconfirmed candidates (applied every 7 days).
pub const DECAY_FACTOR: f64 = 0.9;

/// Minimum occurrences required before a candidate is even considered.
pub const MIN_OCCURRENCES: u64 = 3;

/// Minimum phrase length (chars) for extraction.
pub const MIN_PHRASE_LEN: usize = 2;

/// Maximum phrase length (chars) for extraction.
pub const MAX_PHRASE_LEN: usize = 12;

/// Maximum n-gram size for extraction.
pub const MAX_NGRAM: usize = 3;

/// Score weights.
pub const FREQUENCY_WEIGHT: f64 = 0.6;
pub const RECENCY_WEIGHT: f64 = 0.3;
pub const UNIQUENESS_WEIGHT: f64 = 0.1;

/// A scored hotword candidate waiting for promotion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotwordCandidate {
    /// The extracted phrase.
    pub phrase: String,
    /// Current composite score [0.0, 1.0].
    pub score: f64,
    /// Total occurrence count across all sessions.
    pub occurrences: u64,
    /// Days since first seen (approximate).
    pub days_active: i64,
    /// Days since last hit.
    pub days_since_last: i64,
}

impl HotwordCandidate {
    pub fn status(&self) -> CandidateStatus {
        if self.score >= AUTO_PROMOTE_THRESHOLD {
            CandidateStatus::AutoPromote
        } else if self.score >= SUGGEST_THRESHOLD {
            CandidateStatus::Suggest
        } else if self.score < DISCARD_THRESHOLD && self.days_since_last > MAX_CANDIDATE_AGE_DAYS
        {
            CandidateStatus::Discard
        } else {
            CandidateStatus::Accumulating
        }
    }

    /// Apply weekly decay.
    pub fn decay(&mut self) {
        self.score = (self.score * DECAY_FACTOR * 100.0).round() / 100.0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateStatus {
    /// Ready to be automatically added to dictionary.
    AutoPromote,
    /// Show in suggestion UI for user review.
    Suggest,
    /// Still accumulating evidence.
    Accumulating,
    /// Too old, too weak — clean up.
    Discard,
}

/// Check if a word is a common stop word that should never be extracted.
pub fn is_stop_word(word: &str) -> bool {
    let s = word.trim();
    if s.len() < 2 {
        return true;
    }

    // Chinese common stop characters and words
    const CN_STOPS: &[&str] = &[
        "的", "了", "是", "在", "我", "你", "他", "她", "它", "们",
        "这", "那", "不", "就", "也", "都", "还", "很", "要", "能",
        "会", "和", "与", "或", "但", "而", "且", "所", "以", "为",
        "着", "过", "得", "地", "把", "被", "让", "给", "对", "从",
        "到", "向", "用", "比", "吗", "呢", "吧", "啊", "嘛", "哦",
        "嗯", "呵", "呀", "哈", "嘛",
    ];

    const EN_STOPS: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been",
        "have", "has", "had", "do", "does", "did", "will", "would",
        "can", "could", "may", "might", "shall", "should", "must",
        "to", "of", "in", "on", "at", "for", "with", "by", "from",
        "this", "that", "it", "and", "or", "but", "so", "if", "not",
        "no", "yes", "just", "only", "also", "very", "too", "all",
        "some", "any", "each", "every", "both", "few", "more", "most",
        "other", "such", "than", "then", "now", "here", "there",
        "when", "where", "which", "who", "what", "how", "why",
    ];

    let lower = s.to_lowercase();
    CN_STOPS.contains(&s) || EN_STOPS.contains(&lower.as_str())
}

/// Compute composite score for a candidate.
pub fn compute_score(occurrences: u64, _days_active: i64, days_since_last: i64) -> f64 {
    let freq = (occurrences as f64 / 5.0).min(1.0);
    let recency = (-(days_since_last as f64 / 7.0)).exp();
    let uniqueness = 1.0; // Placeholder; would use IDF if we had a corpus

    let raw = freq * FREQUENCY_WEIGHT + recency * RECENCY_WEIGHT + uniqueness * UNIQUENESS_WEIGHT;
    (raw * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_stop_word_chinese() {
        assert!(is_stop_word("的"));
        assert!(is_stop_word("我"));
        assert!(is_stop_word("不"));
    }

    #[test]
    fn test_is_stop_word_english() {
        assert!(is_stop_word("the"));
        assert!(is_stop_word("and"));
        assert!(is_stop_word("is"));
    }

    #[test]
    fn test_not_stop_word() {
        assert!(!is_stop_word("微信"));
        assert!(!is_stop_word("API"));
        assert!(!is_stop_word("Python"));
    }

    #[test]
    fn test_compute_score_high() {
        // Very frequent, very recent → high score
        let score = compute_score(10, 3, 0);
        assert!(score > 0.7, "Expected high score, got {}", score);
    }

    #[test]
    fn test_compute_score_low() {
        // Barely seen, old → low score
        let score = compute_score(2, 20, 15);
        assert!(score < 0.5, "Expected low score, got {}", score);
    }

    #[test]
    fn test_candidate_status_auto_promote() {
        let c = HotwordCandidate {
            phrase: "测试".into(),
            score: 0.8,
            occurrences: 10,
            days_active: 1,
            days_since_last: 0,
        };
        assert_eq!(c.status(), CandidateStatus::AutoPromote);
    }

    #[test]
    fn test_candidate_status_discard() {
        let c = HotwordCandidate {
            phrase: "旧的".into(),
            score: 0.05,
            occurrences: 1,
            days_active: 40,
            days_since_last: 35,
        };
        assert_eq!(c.status(), CandidateStatus::Discard);
    }

    #[test]
    fn test_decay() {
        let mut c = HotwordCandidate {
            phrase: "test".into(),
            score: 0.8,
            occurrences: 5,
            days_active: 10,
            days_since_last: 7,
        };
        c.decay();
        assert!((c.score - 0.72).abs() < 0.01, "Decay result: {}", c.score);
    }
}
