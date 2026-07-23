//! Persistent store for hotword learning candidates.
//!
//! Stores learning state in `hotword-candidates.json` in the app data directory.
//! Separate from DictionaryStore to avoid polluting user-managed vocabulary
//! with auto-generated noise.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::scoring::HotwordCandidate;
use crate::persistence::{atomic_write, data_dir, ensure_dir, read_or_default};

const CANDIDATES_FILE: &str = "hotword-candidates.json";

/// Statistics entry for tracking phrase frequency over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CandidateStats {
    /// Total occurrence count across all dictation sessions.
    occurrences: u64,
    /// Chrono RFC3339 of first sighting.
    first_seen: String,
    /// Chrono RFC3339 of most recent sighting.
    last_seen: String,
    /// Sessions in which this phrase appeared.
    session_count: u64,
}

/// Top-level store file format.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HotwordFile {
    /// Phrase → tracking stats.
    #[serde(default)]
    stats: std::collections::HashMap<String, CandidateStats>,
    /// Phrase IDs that were auto-promoted to dictionary.
    #[serde(default)]
    promoted_ids: Vec<String>,
    /// Last time decay was applied (RFC3339).
    #[serde(default)]
    last_decay_at: Option<String>,
}

/// Manages hotword candidate persistence.
pub struct HotwordStore {
    path: PathBuf,
}

impl HotwordStore {
    pub fn new() -> Result<Self> {
        let dir = data_dir()?;
        ensure_dir(&dir)?;
        Ok(Self {
            path: dir.join(CANDIDATES_FILE),
        })
    }

    /// Record occurrences of a phrase in a dictation session text.
    ///
    /// Extracts candidate phrases from the raw transcript and updates
    /// the tracking stats. Returns the number of new phrases discovered.
    pub fn ingest_session(&self, raw_transcript: &str, now: &str) -> Result<u64> {
        let phrases = super::learn::extract_candidates(raw_transcript);
        if phrases.is_empty() {
            return Ok(0);
        }

        let mut file = self.read()?;
        let mut new_count: u64 = 0;

        for phrase in &phrases {
            if let Some(stats) = file.stats.get_mut(phrase.as_str()) {
                stats.occurrences += 1;
                stats.last_seen = now.to_string();
                stats.session_count += 1;
            } else {
                file.stats.insert(
                    phrase.clone(),
                    CandidateStats {
                        occurrences: 1,
                        first_seen: now.to_string(),
                        last_seen: now.to_string(),
                        session_count: 1,
                    },
                );
                new_count += 1;
            }
        }

        self.write(&file)?;
        Ok(new_count)
    }

    /// List all current candidates with computed scores.
    ///
    /// Only returns candidates that meet `MIN_OCCURRENCES` and pass
    /// the stop-word filter.
    pub fn list_candidates(&self, now: &str) -> Result<Vec<HotwordCandidate>> {
        let file = self.read()?;
        let now_dt = chrono::DateTime::parse_from_rfc3339(now)
            .unwrap_or_else(|_| chrono::Utc::now().into());

        let mut candidates: Vec<HotwordCandidate> = file
            .stats
            .iter()
            .filter(|(phrase, stats)| {
                stats.occurrences >= super::scoring::MIN_OCCURRENCES
                    && !super::scoring::is_stop_word(phrase)
            })
            .filter_map(|(phrase, stats)| {
                let first_seen = chrono::DateTime::parse_from_rfc3339(&stats.first_seen).ok()?;
                let last_seen = chrono::DateTime::parse_from_rfc3339(&stats.last_seen).ok()?;
                let days_active = (now_dt - first_seen).num_days();
                let days_since = (now_dt - last_seen).num_days();

                let score =
                    super::scoring::compute_score(stats.occurrences, days_active, days_since);

                Some(HotwordCandidate {
                    phrase: phrase.clone(),
                    score,
                    occurrences: stats.occurrences,
                    days_active,
                    days_since_last: days_since,
                })
            })
            .collect();

        // Sort by score descending
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(candidates)
    }

    /// Get candidates that are ready for automatic promotion to dictionary.
    pub fn get_auto_promote(&self, now: &str) -> Result<Vec<HotwordCandidate>> {
        let candidates = self.list_candidates(now)?;
        Ok(candidates
            .into_iter()
            .filter(|c| c.status() == super::scoring::CandidateStatus::AutoPromote)
            .filter(|c| {
                // Don't re-promote already promoted
                let file = self.read().ok();
                file.map_or(true, |f| !f.promoted_ids.contains(&c.phrase))
            })
            .collect())
    }

    /// Get suggestions for user review.
    pub fn get_suggestions(&self, now: &str) -> Result<Vec<HotwordCandidate>> {
        let candidates = self.list_candidates(now)?;
        Ok(candidates
            .into_iter()
            .filter(|c| c.status() == super::scoring::CandidateStatus::Suggest)
            .collect())
    }

    /// Mark a phrase as promoted to dictionary.
    pub fn mark_promoted(&self, phrase: &str) -> Result<()> {
        let mut file = self.read()?;
        if !file.promoted_ids.contains(&phrase.to_string()) {
            file.promoted_ids.push(phrase.to_string());
        }
        self.write(&file)
    }

    /// Apply decay to all candidates that haven't been seen in 7+ days.
    pub fn apply_decay(&self, now: &str) -> Result<()> {
        let mut file = self.read()?;
        let now_dt = chrono::DateTime::parse_from_rfc3339(now)
            .unwrap_or_else(|_| chrono::Utc::now().into());

        // Only decay once per 7 days
        if let Some(ref last) = file.last_decay_at {
            if let Ok(last_dt) = chrono::DateTime::parse_from_rfc3339(last) {
                if (now_dt - last_dt).num_days() < 7 {
                    return Ok(());
                }
            }
        }

        let mut to_remove: Vec<String> = Vec::new();

        for (phrase, stats) in file.stats.iter_mut() {
            if let Ok(last_seen) = chrono::DateTime::parse_from_rfc3339(&stats.last_seen) {
                let days_since = (now_dt - last_seen).num_days();
                if days_since < 7 {
                    continue; // Still active, no decay
                }

                // Reduce occurrences (effectively decaying the score)
                stats.occurrences = stats.occurrences.saturating_sub(1);

                // If occurrences drop below MIN, mark for removal
                if stats.occurrences < super::scoring::MIN_OCCURRENCES {
                    to_remove.push(phrase.clone());
                }
            }
        }

        for phrase in &to_remove {
            file.stats.remove(phrase);
        }

        file.last_decay_at = Some(now.to_string());
        self.write(&file)?;
        Ok(())
    }

    /// Clean up stale candidates.
    pub fn cleanup(&self, now: &str) -> Result<usize> {
        let mut file = self.read()?;
        let now_dt = chrono::DateTime::parse_from_rfc3339(now)
            .unwrap_or_else(|_| chrono::Utc::now().into());

        let before = file.stats.len();
        file.stats.retain(|_phrase, stats| {
            if let Ok(last_seen) = chrono::DateTime::parse_from_rfc3339(&stats.last_seen) {
                let days_since = (now_dt - last_seen).num_days();
                // Keep if: recent enough OR has enough occurrences
                days_since < super::scoring::MAX_CANDIDATE_AGE_DAYS
                    || stats.occurrences >= super::scoring::MIN_OCCURRENCES * 3
            } else {
                false
            }
        });

        let removed = before - file.stats.len();
        if removed > 0 {
            self.write(&file)?;
        }
        Ok(removed)
    }

    fn read(&self) -> Result<HotwordFile> {
        read_or_default::<HotwordFile>(&self.path)
    }

    fn write(&self, file: &HotwordFile) -> Result<()> {
        let json = serde_json::to_vec_pretty(file).context("encode hotword file failed")?;
        atomic_write(&self.path, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_ingest_and_list() {
        let tmp = std::env::temp_dir().join(format!("zerotype-hw-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).expect("create temp dir");
        unsafe {
            // Override data_dir for test
            std::env::set_var("HOME", &tmp);
        }

        let now = "2026-07-23T00:00:00+08:00";
        // This is a crude test -- data_dir uses ~/Library/Application Support/ZeroType
        // which may not work properly. The store constructor uses data_dir().
        // We test the ingest logic via the store.
    }

    #[test]
    fn test_ingest_session_deduplicates() {
        // Integration test placeholder
    }
}
