//! ZeroType voiceprint (声纹) recognition module.
//!
//! ## Architecture
//!
//! ```
//! Recorder PCM → Buffer (5s) → Extract features → Match against profiles → Speaker ID
//!                                                │
//!                              Enroll: store features as new profile
//! ```
//!
//! ## Feature Extraction
//!
//! Extracts acoustic features from 16kHz mono PCM:
//! - RMS energy (volume)
//! - Zero-crossing rate (pitch proxy)
//! - Spectral centroid (timbre)
//! - Spectral flux (change rate)
//!
//! ## Matching
//!
//! Cosine similarity between feature vectors. Threshold ≥ 0.75 = same speaker.
//!
//! ## Future Upgrade
//!
//! The `SpeakerEngine` trait is designed to be swapped with sherpa-onnx
//! 3D-Speaker embeddings without changing the store or coordinator integration.

pub mod engine;
pub mod store;
pub mod types;

pub use engine::SpeakerEngine;
pub use store::SpeakerProfileStore;
pub use types::{SpeakerEnrollRequest, SpeakerMatchResult, SpeakerProfile, VoiceprintFeatures};
