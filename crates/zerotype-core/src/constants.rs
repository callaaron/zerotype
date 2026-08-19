//! 跨平台规范常量（从桌面端 app 迁入，避免 core 反向依赖 app）。
//!
//! foundry/sherpa 本地 ASR 的默认模型别名与 provider id 原先定义在
//! openless-all/app/src-tauri/src/asr/local/ 下；types.rs 迁入 core 后，
//! 桌面端 asr/local 通过 pub use zerotype_core::constants::... 重导出，
//! 保证 crate::asr::local::foundry::DEFAULT_MODEL_ALIAS 等路径不变。

/// foundry（Windows 本地 Whisper）provider id。
pub const FOUNDRY_LOCAL_PROVIDER_ID: &str = "foundry-local-whisper";
/// foundry 默认模型别名（与 app 原定义一致）。
pub const FOUNDRY_DEFAULT_MODEL_ALIAS: &str = "whisper-small";
/// sherpa-onnx 默认模型别名（与 app 原定义一致）。
pub const SHERPA_DEFAULT_MODEL_ALIAS: &str = "sense-voice-small-zh";
