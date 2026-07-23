//! Phrase extraction and candidate generation from dictation transcripts.
//!
//! ## Extraction Strategy
//!
//! 1. Split raw transcript into sentences (by Chinese/English punctuation)
//! 2. For each sentence, tokenize into words/characters
//! 3. Extract bigrams and trigrams
//! 4. Filter out stop words and too-short phrases
//! 5. Deduplicate and return unique candidates
//!
//! ## Chinese Tokenization
//!
//! We use a simple character-based approach for Chinese (no complex NLP needed):
//! - CJK characters are treated as individual tokens
//! - Latin/English words are kept whole (split by whitespace)
//! - Numbers and symbols are filtered
//!
//! This is intentionally lightweight — the goal is to surface *potential*
//! hotwords, not do perfect linguistic analysis. The scoring system handles
//! noise filtering naturally.

use super::scoring::{is_stop_word, MAX_NGRAM, MAX_PHRASE_LEN, MIN_PHRASE_LEN};
use std::collections::HashSet;

/// Track candidate phrases and their occurrence counts within a session.
pub struct CandidateTracker {
    /// phrase → count (within this session)
    counts: std::collections::HashMap<String, u64>,
    /// Total sentences processed
    sentences_processed: u64,
}

impl CandidateTracker {
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
            sentences_processed: 0,
        }
    }

    /// Process a block of text, extracting and counting candidate phrases.
    pub fn process(&mut self, text: &str) {
        let sentences = split_sentences(text);
        self.sentences_processed += sentences.len() as u64;

        for sentence in &sentences {
            let tokens = tokenize(sentence);
            let ngrams = extract_ngrams(&tokens, MAX_NGRAM);

            for ngram in ngrams {
                let phrase: String = ngram.join("");
                if !is_valid_phrase(&phrase) {
                    continue;
                }
                *self.counts.entry(phrase).or_insert(0) += 1;
            }
        }
    }

    /// Get all unique candidates found in this session.
    pub fn candidates(&self) -> Vec<String> {
        self.counts.keys().cloned().collect()
    }
}

impl Default for CandidateTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract candidate phrases from raw dictation text.
///
/// Returns a deduplicated list of potential hotwords.
pub fn extract_candidates(text: &str) -> Vec<String> {
    let mut tracker = CandidateTracker::new();
    tracker.process(text);
    tracker.candidates()
}

/// Split text into sentences using CJK and Western punctuation.
fn split_sentences(text: &str) -> Vec<String> {
    let delimiters = [
        '。', '！', '？', '；', '\n', '.', '!', '?', ';',
        '，', ',', '、',
    ];

    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if delimiters.contains(&ch) {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() && trimmed.len() > 1 {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }

    // Don't forget the last segment
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() && trimmed.len() > 1 {
        sentences.push(trimmed);
    }

    sentences
}

/// Tokenize a sentence into word-level tokens.
///
/// Strategy:
/// - CJK characters (U+4E00..U+9FFF, U+3400..U+4DBF, etc.): each char = 1 token
/// - ASCII letters + digits: grouped into words
/// - Everything else: filtered out
fn tokenize(sentence: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current_word = String::new();

    fn is_cjk(ch: char) -> bool {
        matches!(
            ch,
            '\u{4E00}'..='\u{9FFF}'   // CJK Unified
            | '\u{3400}'..='\u{4DBF}' // CJK Extension A
            | '\u{F900}'..='\u{FAFF}' // CJK Compatibility
            | '\u{3040}'..='\u{309F}' // Hiragana
            | '\u{30A0}'..='\u{30FF}' // Katakana
        )
    }

    fn is_word_char(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '\''
    }

    for ch in sentence.chars() {
        if is_cjk(ch) {
            // Flush any accumulating word
            if !current_word.is_empty() {
                if current_word.len() >= 2 {
                    tokens.push(current_word.clone());
                }
                current_word.clear();
            }
            tokens.push(ch.to_string());
        } else if is_word_char(ch) {
            current_word.push(ch);
        } else {
            // Whitespace or punctuation: flush word
            if !current_word.is_empty() {
                if current_word.len() >= 2 {
                    tokens.push(current_word.clone());
                }
                current_word.clear();
            }
        }
    }

    // Flush remaining word
    if !current_word.is_empty() && current_word.len() >= 2 {
        tokens.push(current_word);
    }

    tokens
}

/// Extract all n-grams up to max_n from a token slice.
fn extract_ngrams(tokens: &[String], max_n: usize) -> Vec<Vec<&str>> {
    let mut results: Vec<Vec<&str>> = Vec::new();

    for n in 2..=max_n {
        if tokens.len() < n {
            continue;
        }
        for window in tokens.windows(n) {
            // Skip windows containing stop words
            if window.iter().any(|t| is_stop_word(t)) {
                continue;
            }
            results.push(window.iter().map(|s| s.as_str()).collect());
        }
    }

    results
}

/// Check if a phrase is a valid hotword candidate.
fn is_valid_phrase(phrase: &str) -> bool {
    let len = phrase.chars().count();
    if len < MIN_PHRASE_LEN || len > MAX_PHRASE_LEN {
        return false;
    }
    // Skip pure number/symbol phrases
    if phrase.chars().all(|c| c.is_ascii_digit() || c.is_ascii_punctuation()) {
        return false;
    }
    // Skip phrases that are entirely stop words
    if is_stop_word(phrase) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_sentences_chinese() {
        let text = "今天我们讨论了产品方案。明天继续。";
        let sentences = split_sentences(text);
        assert_eq!(sentences.len(), 2);
        assert!(sentences[0].contains("产品方案"));
    }

    #[test]
    fn test_tokenize_cjk() {
        let tokens = tokenize("微信支付");
        assert_eq!(tokens, vec!["微", "信", "支", "付"]);
    }

    #[test]
    fn test_tokenize_mixed() {
        let tokens = tokenize("使用Python开发API接口");
        // 使 用 Python 开 发 API 接 口
        assert!(tokens.contains(&"Python".to_string()));
        assert!(tokens.contains(&"API".to_string()));
    }

    #[test]
    fn test_extract_ngrams_no_stopwords() {
        let tokens: Vec<String> = vec!["数据", "分析", "平台"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let ngrams = extract_ngrams(&tokens, 3);
        // Bigram: 数据分析 + 分析平台
        // Trigram: 数据分析平台
        assert_eq!(ngrams.len(), 3);
    }

    #[test]
    fn test_ngrams_filter_stopwords() {
        let tokens: Vec<String> = vec!["我", "的", "产品"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let ngrams = extract_ngrams(&tokens, 3);
        // All windows contain stops — should be empty
        assert_eq!(ngrams.len(), 0);
    }

    #[test]
    fn test_candidate_tracker_basic() {
        let mut tracker = CandidateTracker::new();
        tracker.process("微信支付很方便。支付宝也很好用。微信支付很安全。");
        let candidates = tracker.candidates();
        // Should find "微信支付" (appears twice), "支付宝" (once)
        // Note: "微信" and "支付" won't qualify because they're 1-char each when
        // CJK tokenized, and bigrams filter stop words ("很" is not a stop word)
        assert!(!candidates.is_empty());
    }

    #[test]
    fn test_is_valid_phrase_too_short() {
        assert!(!is_valid_phrase("A"));
    }

    #[test]
    fn test_is_valid_phrase_good() {
        assert!(is_valid_phrase("Python"));
        assert!(is_valid_phrase("数据分析"));
    }
}
