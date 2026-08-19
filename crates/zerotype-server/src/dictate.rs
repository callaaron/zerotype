//! 听写网关：ASR（zerotype-core::asr::whisper）→ 润色（zerotype-core::polish）。

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::Engine;
use serde::{Deserialize, Serialize};
use zerotype_core::asr::whisper::WhisperBatchASR;
use zerotype_core::asr::RawTranscript;
use zerotype_core::polish::{OpenAICompatibleConfig, OpenAICompatibleLLMProvider};
use zerotype_core::recorder::AudioConsumer;
use zerotype_core::types::{
    default_style_system_prompt_for_mode, ChineseScriptPreference, OutputLanguagePreference,
    PolishMode,
};

use crate::auth::{user_from_bearer, User};
use crate::SharedState;

/// LLM 网关配置（环境变量注入，平台托管模式下由后端持有 key）。
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl LlmConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("ZEROTYPE_LLM_API_KEY").unwrap_or_default(),
            base_url: std::env::var("ZEROTYPE_LLM_BASE_URL")
                .unwrap_or_else(|_| "https://api.deepseek.com/v1".into()),
            model: std::env::var("ZEROTYPE_LLM_MODEL").unwrap_or_else(|_| "deepseek-chat".into()),
        }
    }
}

/// ASR 网关配置（默认 OpenAI 兼容 Whisper 端点，可换火山/豆包等）。
#[derive(Debug, Clone)]
pub struct AsrConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl AsrConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("ZEROTYPE_ASR_API_KEY").unwrap_or_default(),
            base_url: std::env::var("ZEROTYPE_ASR_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
            model: std::env::var("ZEROTYPE_ASR_MODEL").unwrap_or_else(|_| "whisper-1".into()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DictateRequest {
    /// 16kHz mono s16le PCM，base64 编码。
    pub audio_base64: String,
    /// 润色模式：raw / light / structured / formal，缺省 light。
    #[serde(default)]
    pub mode: Option<String>,
    /// 热词（提升专有名词识别）。
    #[serde(default)]
    pub hotwords: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DictateResponse {
    pub text: String,
    pub raw_text: String,
    pub chars: u64,
    pub quota: crate::quota::WeekUsage,
}

fn parse_mode(mode: Option<&str>) -> PolishMode {
    match mode.unwrap_or("light").to_ascii_lowercase().as_str() {
        "raw" => PolishMode::Raw,
        "structured" => PolishMode::Structured,
        "formal" => PolishMode::Formal,
        _ => PolishMode::Light,
    }
}

/// POST /dictate：Bearer 鉴权 → 配额检查 → Whisper 转写 → LLM 润色 → 记账。
pub async fn dictate(
    State(state): State<SharedState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<DictateRequest>,
) -> impl IntoResponse {
    let user = match user_from_bearer(&state, headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok())) {
        Ok(u) => u,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(DictateError::new(&e))).into_response(),
    };

    let pcm = match base64::engine::general_purpose::STANDARD.decode(req.audio_base64) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(DictateError::new(&format!("audio_base64 解码失败: {e}")))).into_response(),
    };

    match run_pipeline(&state, &user, &pcm, &req.hotwords, req.mode.as_deref()).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(DictateError::new(&e))).into_response(),
    }
}

/// 听写流水线：ASR → 润色 → 记账。REST 与 WS 会话共用。
pub async fn run_pipeline(
    state: &SharedState,
    user: &User,
    pcm: &[u8],
    hotwords: &[String],
    mode_hint: Option<&str>,
) -> Result<DictateResponse, String> {
    // 1) ASR（OpenAI 兼容 Whisper 批式端点）。
    let asr = WhisperBatchASR::new(
        state.asr.api_key.clone(),
        state.asr.base_url.clone(),
        state.asr.model.clone(),
        None,          // prompt
        Some(30_000),  // max chunk
        false,         // verbose_json
    )
    .with_hotwords(hotwords.to_vec());
    asr.consume_pcm_chunk(pcm);
    let RawTranscript { text: raw_text, duration_ms } = asr
        .transcribe()
        .await
        .map_err(|e| format!("ASR 失败: {e}"))?;
    let raw_text = raw_text.trim().to_string();
    if raw_text.is_empty() {
        return Ok(DictateResponse {
            text: String::new(),
            raw_text,
            chars: 0,
            quota: state.quotas.status(&user.id),
        });
    }

    // 2) LLM 润色（复用 core::polish，DeepSeek/Ark/OpenAI 兼容）。
    let mode = parse_mode(mode_hint);
    let config = OpenAICompatibleConfig::new(
        "zerotype-server",
        "ZeroType Server LLM",
        state.llm.base_url.clone(),
        state.llm.api_key.clone(),
        state.llm.model.clone(),
    );
    let llm = OpenAICompatibleLLMProvider::new(config);
    let polished = llm
        .polish(
            &raw_text,
            mode,
            hotwords,
            &default_style_system_prompt_for_mode(mode),
            &["简体中文".to_string()],
            ChineseScriptPreference::Auto,
            OutputLanguagePreference::Auto,
            None, // front_app
            &[], // prior_turns
        )
        .await
        .map_err(|e| format!("润色失败: {e}"))?;

    // 3) 记账（超额不阻断，Phase 2 接入支付升级）。
    let chars = polished.chars().count() as u64;
    let quota = state.quotas.record(&user.id, chars);
    tracing::info!(
        "dictate user={} chars={} asr_ms={} mode={:?}",
        user.email,
        chars,
        duration_ms,
        mode
    );

    Ok(DictateResponse {
        text: polished,
        raw_text,
        chars,
        quota,
    })
}

#[derive(Debug, Serialize)]
pub struct DictateError {
    pub error: String,
}

impl DictateError {
    fn new(msg: &str) -> Self {
        Self { error: msg.into() }
    }
}
