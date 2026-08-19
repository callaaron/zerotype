//! ZeroType 云后端（zerotype-server）。
//!
//! Phase 1 骨架：JWT 认证 + 每周配额 + ASR/LLM 网关（复用 zerotype-core）。
//! 数据先用进程内存储（Postgres 待接入）；WS 流式协议见 ws.rs。

mod auth;
mod dictate;
mod quota;
mod ws;

use std::sync::Arc;

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

/// 全局状态：认证 / 配额 / 网关配置。
pub struct AppState {
    pub users: auth::UserStore,
    pub quotas: quota::QuotaStore,
    pub llm: dictate::LlmConfig,
    pub asr: dictate::AsrConfig,
}

pub type SharedState = Arc<AppState>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,zerotype_server=debug,tower_http=info".into()),
        )
        .init();

    let state = Arc::new(AppState {
        users: auth::UserStore::default(),
        quotas: quota::QuotaStore::default(),
        llm: dictate::LlmConfig::from_env(),
        asr: dictate::AsrConfig::from_env(),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/dictate", post(dictate::dictate))
        .route("/ws", get(ws_upgrade))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = std::env::var("ZEROTYPE_ADDR").unwrap_or_else(|_| "0.0.0.0:8300".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("ZeroType server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

/// WS 流式听写会话入口：/ws 升级为 WebSocket 后交给 ws::handle_session。
async fn ws_upgrade(
    State(state): State<SharedState>,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| ws::handle_session(state, socket))
}
