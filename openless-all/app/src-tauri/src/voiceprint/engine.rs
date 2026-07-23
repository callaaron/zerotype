//! SpeakerEngine — extracts acoustic features from 16kHz mono PCM audio
//! and matches against stored speaker profiles.

use super::types::*;

/// Frame size for feature extraction: 25ms at 16kHz = 400 samples.
const FRAME_SIZE: usize = 400;

/// Hop size between frames: 10ms = 160 samples.
const FRAME_HOP: usize = 160;

/// Minimum audio duration (seconds) for enrollment.
const MIN_ENROLL_SECS: f64 = 3.0;

/// The speaker recognition engine.
pub struct SpeakerEngine;

impl SpeakerEngine {
    /// Extract voiceprint features from raw 16kHz mono Int16 PCM data.
    pub fn extract_features(pcm: &[i16]) -> Option<VoiceprintFeatures> {
        if pcm.len() < FRAME_SIZE {
            return None;
        }

        let num_frames = (pcm.len() - FRAME_SIZE) / FRAME_HOP + 1;
        if num_frames == 0 {
            return None;
        }

        let mut rms_values = Vec::with_capacity(num_frames);
        let mut zcr_values = Vec::with_capacity(num_frames);
        let mut centroid_values = Vec::with_capacity(num_frames);
        let mut flux_values = Vec::with_capacity(num_frames);

        let mut prev_spectrum: Option<Vec<f64>> = None;

        for i in 0..num_frames {
            let start = i * FRAME_HOP;
            let end = (start + FRAME_SIZE).min(pcm.len());
            let frame = &pcm[start..end];

            // RMS (root mean square energy)
            let rms = rms_of_frame(frame);
            rms_values.push(rms);

            // Zero-crossing rate
            let zcr = zero_crossing_rate(frame);
            zcr_values.push(zcr);

            // Spectral centroid (approximation via weighted energy in FFT bands)
            let (centroid, spectrum) = spectral_centroid(frame);
            centroid_values.push(centroid);

            // Spectral flux (change from previous frame)
            if let Some(prev) = &prev_spectrum {
                let flux = spectral_flux(&spectrum, prev);
                flux_values.push(flux);
            } else {
                flux_values.push(0.0);
            }

            prev_spectrum = Some(spectrum);
        }

        let mut features = VoiceprintFeatures {
            rms_mean: mean(&rms_values),
            rms_std: std_dev(&rms_values),
            zcr_mean: mean(&zcr_values),
            zcr_std: std_dev(&zcr_values),
            centroid_mean: mean(&centroid_values),
            centroid_std: std_dev(&centroid_values),
            flux_mean: mean(&flux_values),
            flux_std: std_dev(&flux_values),
        };

        features.normalize();
        Some(features)
    }

    /// Enroll a new speaker from PCM audio.
    pub fn enroll(pcm: &[i16]) -> Result<VoiceprintFeatures, String> {
        let duration_secs = pcm.len() as f64 / 16000.0;
        if duration_secs < MIN_ENROLL_SECS {
            return Err(format!(
                "录音时长不足，需要至少 {} 秒",
                MIN_ENROLL_SECS as u32
            ));
        }

        Self::extract_features(pcm).ok_or("无法提取声纹特征".into())
    }

    /// Match audio against a list of profiles. Returns best match if above threshold.
    pub fn identify(
        pcm: &[i16],
        profiles: &[SpeakerProfile],
    ) -> Option<SpeakerMatchResult> {
        let features = Self::extract_features(pcm)?;

        let mut best: Option<SpeakerMatchResult> = None;

        for profile in profiles {
            let similarity = features.similarity(&profile.features);
            if similarity < MATCH_THRESHOLD {
                continue;
            }
            match &best {
                Some(current) if similarity <= current.confidence => {}
                _ => {
                    best = Some(SpeakerMatchResult {
                        speaker_id: profile.id.clone(),
                        speaker_name: profile.name.clone(),
                        confidence: similarity,
                        is_match: true,
                    });
                }
            }
        }

        best
    }
}

// ─── Feature extraction helpers ──────────────────────

/// Compute RMS (root mean square) of a frame of i16 samples.
fn rms_of_frame(frame: &[i16]) -> f64 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = frame.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / frame.len() as f64).sqrt()
}

/// Compute zero-crossing rate of a frame.
fn zero_crossing_rate(frame: &[i16]) -> f64 {
    if frame.len() < 2 {
        return 0.0;
    }
    let mut crossings: u64 = 0;
    for i in 1..frame.len() {
        if (frame[i] >= 0) != (frame[i - 1] >= 0) {
            crossings += 1;
        }
    }
    crossings as f64 / (frame.len() - 1) as f64
}

/// Compute an approximate spectral centroid using a simple DFT with 8 bands.
///
/// Returns (centroid value, band energies for flux calculation).
fn spectral_centroid(frame: &[i16]) -> (f64, Vec<f64>) {
    const NUM_BANDS: usize = 8;
    let n = frame.len();
    let mut band_energies = vec![0.0f64; NUM_BANDS];

    // Simple band-pass energy estimation using moving average filters
    let band_width = (n / 2) / NUM_BANDS;

    for (band_idx, energy) in band_energies.iter_mut().enumerate().take(NUM_BANDS) {
        let freq_start = band_idx * band_width + 1;
        let freq_end = (band_idx + 1) * band_width;

        let mut sum = 0.0f64;
        for k in freq_start..freq_end.min(n / 2) {
            // DFT for single frequency bin (Goertzel-style)
            let omega = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            let (mut real, mut imag) = (0.0f64, 0.0f64);
            for (t, &sample) in frame.iter().enumerate() {
                let angle = omega * t as f64;
                real += sample as f64 * angle.cos();
                imag -= sample as f64 * angle.sin();
            }
            sum += (real * real + imag * imag).sqrt();
        }
        *energy = sum / (freq_end - freq_start).max(1) as f64;
    }

    // Compute centroid
    let total: f64 = band_energies.iter().sum();
    if total < 1e-10 {
        return (0.0, band_energies);
    }

    let centroid = band_energies
        .iter()
        .enumerate()
        .map(|(i, e)| (i + 1) as f64 * e)
        .sum::<f64>()
        / total;

    (centroid, band_energies)
}

/// Compute spectral flux between two frames (sum of positive energy differences).
fn spectral_flux(current: &[f64], previous: &[f64]) -> f64 {
    current
        .iter()
        .zip(previous.iter())
        .map(|(&c, &p)| (c - p).max(0.0))
        .sum()
}

/// Arithmetic mean.
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Standard deviation.
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_pcm(freq: f64, sample_rate: u32, duration_secs: f64) -> Vec<i16> {
        let num_samples = (sample_rate as f64 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f64 / sample_rate as f64;
            let val = (t * 2.0 * std::f64::consts::PI * freq).sin();
            samples.push((val * 16000.0) as i16); // Scaled for audibility
        }
        samples
    }

    #[test]
    fn test_extract_features_basic() {
        let pcm = make_test_pcm(440.0, 16000, 3.0);
        let features = SpeakerEngine::extract_features(&pcm);
        assert!(features.is_some(), "Should extract features from valid audio");
    }

    #[test]
    fn test_extract_features_too_short() {
        let pcm = vec![0i16; 100]; // Too short
        let features = SpeakerEngine::extract_features(&pcm);
        assert!(features.is_none(), "Should reject too-short audio");
    }

    #[test]
    fn test_same_speaker_similarity() {
        // Two samples of the same frequency should have high similarity
        let pcm1 = make_test_pcm(440.0, 16000, 3.0);
        let pcm2 = make_test_pcm(440.0, 16000, 3.0);

        let f1 = SpeakerEngine::extract_features(&pcm1).unwrap();
        let f2 = SpeakerEngine::extract_features(&pcm2).unwrap();

        let sim = f1.similarity(&f2);
        assert!(sim > 0.9, "Same pitch should have high similarity, got {}", sim);
    }

    #[test]
    fn test_different_speaker_different() {
        // Different frequencies should have lower similarity
        let pcm1 = make_test_pcm(220.0, 16000, 3.0);
        let pcm2 = make_test_pcm(880.0, 16000, 3.0);

        let f1 = SpeakerEngine::extract_features(&pcm1).unwrap();
        let f2 = SpeakerEngine::extract_features(&pcm2).unwrap();

        let sim = f1.similarity(&f2);
        assert!(sim < 0.9, "Different pitch should differ more, got {}", sim);
    }
}
