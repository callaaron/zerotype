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
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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

    // 存储：SQLite 起步（零运维）；切 Postgres 时改用 PgPoolOptions + DATABASE_URL。
    // 显式 filename + create_if_missing，避免 sqlx URL 解析歧义（sqlite:// 三斜杠形式）。
    let db_path = std::env::var("ZEROTYPE_DB_PATH").unwrap_or_else(|_| "zerotype.db".into());
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    let users = auth::UserStore::new(pool.clone());
    let quotas = quota::QuotaStore::new(pool);
    users.init().await?;
    quotas.init().await?;

    let state = Arc::new(AppState {
        users,
        quotas,
        llm: dictate::LlmConfig::from_env(),
        asr: dictate::AsrConfig::from_env(),
    });

    let app = build_router(state.clone());

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


/// 组装路由（main 与集成测试共用）。
fn build_router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/dictate", post(dictate::dictate))
        .route("/ws", get(ws_upgrade))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod e2e;

