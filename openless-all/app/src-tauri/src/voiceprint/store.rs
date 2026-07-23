//! SpeakerProfileStore — persistent speaker profile storage.
//!
//! Stores profiles in `zerotype-speakers.json` in the app data directory.

use std::path::PathBuf;

use parking_lot::Mutex;
use uuid::Uuid;

use super::engine::SpeakerEngine;
use super::types::*;
use crate::persistence::{atomic_write, data_dir, ensure_dir, read_or_default};

const PROFILES_FILE: &str = "zerotype-speakers.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfilesFile {
    profiles: Vec<SpeakerProfile>,
}

impl Default for ProfilesFile {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }
}

use serde::{Deserialize, Serialize};

pub struct SpeakerProfileStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl SpeakerProfileStore {
    pub fn new() -> Result<(), String> {
        // We implement new() to return Result for the module pattern, but
        // the actual creation happens inline since we need `self` for methods.
        Ok(())
    }

    pub fn from_data_dir() -> Result<Self, String> {
        let dir = data_dir().map_err(|e| e.to_string())?;
        ensure_dir(&dir).map_err(|e| e.to_string())?;
        Ok(Self {
            path: dir.join(PROFILES_FILE),
            lock: Mutex::new(()),
        })
    }

    /// Enroll a new speaker from PCM audio.
    pub fn enroll(&self, name: &str, pcm: &[i16]) -> Result<SpeakerProfile, String> {
        let features = SpeakerEngine::enroll(pcm)?;
        let _guard = self.lock.lock();
        let mut file = self.read()?;

        let profile = SpeakerProfile {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            features,
            created_at: chrono::Utc::now().to_rfc3339(),
            match_count: 0,
        };

        file.profiles.push(profile.clone());
        self.write(&file)?;
        Ok(profile)
    }

    /// Identify a speaker from PCM audio. Returns best match.
    pub fn identify(&self, pcm: &[i16]) -> Option<SpeakerMatchResult> {
        let _guard = self.lock.lock();
        let file = self.read().ok()?;
        SpeakerEngine::identify(pcm, &file.profiles)
    }

    /// List all registered speakers.
    pub fn list(&self) -> Result<Vec<SpeakerProfile>, String> {
        let _guard = self.lock.lock();
        let file = self.read()?;
        Ok(file.profiles.clone())
    }

    /// Remove a speaker profile.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let _guard = self.lock.lock();
        let mut file = self.read()?;
        file.profiles.retain(|p| p.id != id);
        self.write(&file)
    }

    /// Increment match count for a speaker.
    pub fn bump_match_count(&self, id: &str) -> Result<(), String> {
        let _guard = self.lock.lock();
        let mut file = self.read()?;
        if let Some(profile) = file.profiles.iter_mut().find(|p| p.id == id) {
            profile.match_count = profile.match_count.saturating_add(1);
            self.write(&file)?;
        }
        Ok(())
    }

    fn read(&self) -> Result<ProfilesFile, String> {
        read_or_default::<ProfilesFile>(&self.path).map_err(|e| e.to_string())
    }

    fn write(&self, file: &ProfilesFile) -> Result<(), String> {
        let json =
            serde_json::to_vec_pretty(file).map_err(|e| format!("encode profiles failed: {e}"))?;
        atomic_write(&self.path, &json).map_err(|e| e.to_string())
    }
}
