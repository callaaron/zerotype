//! Voiceprint IPC commands — enrollment, identification, management.

use super::*;
use crate::voiceprint::{SpeakerEnrollRequest, SpeakerMatchResult, SpeakerProfile};

#[tauri::command]
pub fn voiceprint_enroll(
    coord: CoordinatorState<'_>,
    req: SpeakerEnrollRequest,
    pcm_base64: String,
) -> Result<SpeakerProfile, String> {
    let pcm_bytes = base64_decode_pcm(&pcm_base64)?;
    coord.voiceprint().enroll(&req.name, &pcm_bytes)
}

#[tauri::command]
pub fn voiceprint_identify(
    coord: CoordinatorState<'_>,
    pcm_base64: String,
) -> Result<Option<SpeakerMatchResult>, String> {
    let pcm_bytes = base64_decode_pcm(&pcm_base64)?;
    Ok(coord.voiceprint().identify(&pcm_bytes))
}

#[tauri::command]
pub fn voiceprint_list(
    coord: CoordinatorState<'_>,
) -> Result<Vec<SpeakerProfile>, String> {
    coord.voiceprint().list()
}

#[tauri::command]
pub fn voiceprint_remove(
    coord: CoordinatorState<'_>,
    id: String,
) -> Result<(), String> {
    coord.voiceprint().remove(&id)
}

fn base64_decode_pcm(b64: &str) -> Result<Vec<i16>, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("base64解码失败: {e}"))?;

    if bytes.len() % 2 != 0 {
        return Err("PCM数据长度必须是偶数".into());
    }

    let samples: Vec<i16> = bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    Ok(samples)
}
