//! ZeroType 纯逻辑核心（zerotype-core）。
//!
//! 目标：把不依赖 Tauri 的可复用逻辑从桌面端抽出来，供桌面 App 与未来的
//! axum 后端服务共享（ASR 客户端 / LLM 客户端 / 录音 / 状态机 / 评分等）。
//! 模块逐个从 openless-all/app/src-tauri/src 迁入；桌面端保留同名 shim 重导出，
//! 使 crate:: 路径零改动。

pub mod asr;

/// 测试专用：绕过本机系统代理（如 Clash 127.0.0.1:7897）对 localhost 连接的劫持。
///
/// core 的 reqwest 客户端开了 system-proxy 特性，测试连本地 mock 时会被系统
/// 代理拦截（代理未运行时直接 ECONNREFUSED）。受影响的测试在开头调用本函数，
/// 一次性设置 NO_PROXY（Once 保证只执行一次，避免并行测试重复写环境变量）。
#[cfg(test)]
pub fn bypass_proxy_for_tests() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
        std::env::set_var("no_proxy", "127.0.0.1,localhost");
    });
}

pub mod constants;
pub mod endpoint_security;
pub mod coordinator_state;
pub mod hotword_scoring;
pub mod net;
pub mod polish;
pub mod recorder;
pub mod types;