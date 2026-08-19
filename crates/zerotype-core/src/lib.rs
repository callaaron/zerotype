//! ZeroType 纯逻辑核心（zerotype-core）。
//!
//! 目标：把不依赖 Tauri 的可复用逻辑从桌面端抽出来，供桌面 App 与未来的
//! axum 后端服务共享（ASR 客户端 / LLM 客户端 / 录音 / 状态机 / 评分等）。
//! 模块逐个从 openless-all/app/src-tauri/src 迁入；桌面端保留同名 shim 重导出，
//! 使 crate:: 路径零改动。

pub mod constants;
pub mod coordinator_state;
pub mod hotword_scoring;
pub mod types;
