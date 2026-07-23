//! Voiceprint data types.

use serde::{Deserialize, Serialize};

/// A registered speaker profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerProfile {
    pub id: String,
    pub name: String,
    /// Feature vector (normalized).
    pub features: VoiceprintFeatures,
    pub created_at: String,
    /// Number of successful identifications.
    pub match_count: u64,
}

/// Acoustic feature vector extracted from audio.
///
/// Simple 8-dim feature vector for lightweight matching.
/// Upgradable to 192-dim sherpa-onnx embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceprintFeatures {
    /// Mean RMS energy (volume).
    pub rms_mean: f64,
    /// RMS standard deviation.
    pub rms_std: f64,
    /// Mean zero-crossing rate.
    pub zcr_mean: f64,
    /// Zero-crossing rate std.
    pub zcr_std: f64,
    /// Mean spectral centroid (approximate via frame energy distribution).
    pub centroid_mean: f64,
    pub centroid_std: f64,
    /// Spectral flux mean (frame-to-frame change).
    pub flux_mean: f64,
    pub flux_std: f64,
}

impl VoiceprintFeatures {
    /// Convert to a flat normalized vector for similarity comparison.
    pub fn to_vector(&self) -> [f64; 8] {
        [
            self.rms_mean,
            self.rms_std,
            self.zcr_mean,
            self.zcr_std,
            self.centroid_mean,
            self.centroid_std,
            self.flux_mean,
            self.flux_std,
        ]
    }

    /// Cosine similarity between two feature vectors.
    pub fn similarity(&self, other: &VoiceprintFeatures) -> f64 {
        let a = self.to_vector();
        let b = other.to_vector();

        let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_a < 1e-10 || norm_b < 1e-10 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)).max(-1.0).min(1.0)
    }

    /// Normalize self so that all dimensions have comparable scales.
    pub fn normalize(&mut self) {
        let vec = self.to_vector();
        let max_val = vec.iter().cloned().fold(0.0f64, f64::max);
        if max_val > 1e-10 {
            self.rms_mean /= max_val;
            self.rms_std /= max_val;
            self.zcr_mean /= max_val;
            self.zcr_std /= max_val;
            self.centroid_mean /= max_val;
            self.centroid_std /= max_val;
            self.flux_mean /= max_val;
            self.flux_std /= max_val;
        }
    }
}

/// Enrollment request from frontend.
#[derive(Debug, Clone, Deserialize)]
pub struct SpeakerEnrollRequest {
    pub name: String,
}

/// Speaker match result.
#[derive(Debug, Clone, Serialize)]
pub struct SpeakerMatchResult {
    pub speaker_id: String,
    pub speaker_name: String,
    pub confidence: f64,
    pub is_match: bool,
}

/// Similarity threshold for considering a match.
pub const MATCH_THRESHOLD: f64 = 0.75;
