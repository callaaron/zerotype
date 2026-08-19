//! ASR 提供方总入口：云引擎客户端已迁入 zerotype-core（本目录为 shim），
//! 本地引擎（qwen/sherpa/foundry，依赖 Tauri AppHandle）保留在本 crate。

pub mod bailian;
pub mod dashscope_multimodal;
pub mod elevenlabs;
mod frame;
pub mod local;
pub mod mimo;
pub mod pcm;
pub mod qwen_realtime;
pub mod stepfun_realtime;
pub mod volcengine;
pub mod wav;
pub mod whisper;

pub use bailian::{BailianCredentials, BailianRealtimeASR};
pub use dashscope_multimodal::DashScopeMultimodalASR;
pub use elevenlabs::ElevenLabsBatchASR;
pub use mimo::MimoBatchASR;
pub use qwen_realtime::{Qwen3RealtimeASR, Qwen3RealtimeCredentials};
pub use stepfun_realtime::{StepfunRealtimeASR, StepfunRealtimeCredentials};
pub use volcengine::{VolcengineCredentials, VolcengineStreamingASR};
pub use whisper::WhisperBatchASR;

// 以下类型随云引擎一起迁入 core，此处重导出保持 crate::asr::* 路径不变。
pub use zerotype_core::asr::{AudioConsumer, DictionaryHotword, RawTranscript};
