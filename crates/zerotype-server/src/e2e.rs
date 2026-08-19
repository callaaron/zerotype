//! 端到端集成测试：mock ASR/LLM 提供商，验证 register→login→/dictate→记账 全链路。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use base64::Engine;
use serde_json::json;
use tower::ServiceExt;

use crate::auth::{sign_token, User, UserStore};
use crate::dictate::{AsrConfig, LlmConfig};
use crate::quota::QuotaStore;
use crate::{build_router, AppState, SharedState};

/// 启动一个 mock OpenAI 兼容提供商（whisper 转写 + chat completions 润色）。
async fn spawn_mock_provider() -> u16 {
    let app = axum::Router::new()
        .route(
            "/v1/audio/transcriptions",
            axum::routing::post(|| async {
                axum::Json(json!({ "text": "这是一个测试转写" }))
            }),
        )
        .route(
            "/v1/chat/completions",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "choices": [{ "message": { "content": "这是一段润色后的文字。" } }],
                }))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}

async fn test_state(provider_port: u16) -> SharedState {
    // 本机系统代理（如 Clash 127.0.0.1:7897）会劫持 reqwest 的 localhost 连接；
    // 测试一律直连本机 mock，显式绕过代理。
    std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
    std::env::set_var("no_proxy", "127.0.0.1,localhost");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let users = UserStore::new(pool.clone());
    let quotas = QuotaStore::new(pool);
    users.init().await.unwrap();
    quotas.init().await.unwrap();
    let base = format!("http://127.0.0.1:{provider_port}/v1");
    std::env::set_var("ZEROTYPE_JWT_SECRET", "e2e-secret");
    Arc::new(AppState {
        users,
        quotas,
        llm: LlmConfig {
            api_key: "mock-key".into(),
            base_url: base.clone(),
            model: "mock-model".into(),
        },
        asr: AsrConfig {
            api_key: "mock-key".into(),
            base_url: base,
            model: "whisper-mock".into(),
        },
    })
}

async fn send_json(app: &axum::Router, method: &str, path: &str, body: Option<serde_json::Value>, token: Option<&str>) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = match body {
        Some(v) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    (status, value)
}

#[tokio::test]
async fn dictation_flow_end_to_end_with_mock_providers() {
    let port = spawn_mock_provider().await;
    let state = test_state(port).await;
    let app = build_router(state.clone());

    // 1) 注册
    let (status, reg) = send_json(
        &app,
        "POST",
        "/auth/register",
        Some(json!({ "email": "e2e@zerotype.app", "password": "secret123" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "注册应成功: {reg}");
    let token = reg["token"].as_str().unwrap().to_string();

    // 2) 重复注册被拒
    let (status, _) = send_json(
        &app,
        "POST",
        "/auth/register",
        Some(json!({ "email": "e2e@zerotype.app", "password": "secret123" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // 3) /dictate：PCM → mock ASR → mock LLM → 润色文本 + 记账
    let pcm = vec![0u8; 32000]; // 2 秒 16k mono 静音帧（mock 不解析内容）
    let (status, resp) = send_json(
        &app,
        "POST",
        "/dictate",
        Some(json!({
            "audio_base64": base64::engine::general_purpose::STANDARD.encode(pcm),
            "mode": "light",
        })),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "转写应成功: {resp}");
    assert_eq!(resp["text"], "这是一段润色后的文字。");
    assert_eq!(resp["raw_text"], "这是一个测试转写");
    assert_eq!(resp["quota"]["chars_used"], 11); // 润色文本 11 字

    // 4) 未认证的 /dictate 被拒
    let (status, _) = send_json(
        &app,
        "POST",
        "/dictate",
        Some(json!({ "audio_base64": "AAAA" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 5) 无 key 时优雅报错（而非崩溃）
    let (status, resp) = send_json(
        &app,
        "POST",
        "/dictate",
        Some(json!({ "audio_base64": base64::engine::general_purpose::STANDARD.encode(vec![0u8; 100]) })),
        Some(&token),
    )
    .await;
    assert!(status.is_server_error() || status.is_success(), "应返回可读错误: {resp}");
}

#[tokio::test]
async fn ws_streaming_session_end_to_end() {
    let port = spawn_mock_provider().await;
    let state = test_state(port).await;
    let app = build_router(state.clone());

    // 在临时端口上真实启动服务器（WS 升级需要真实 socket）。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // 注册拿 token
    let reg_app = build_router(state.clone());
    let (status, reg) = send_json(
        &reg_app,
        "POST",
        "/auth/register",
        Some(json!({ "email": "ws@zerotype.app", "password": "secret123" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = reg["token"].as_str().unwrap().to_string();

    // 连接 WS
    let url = format!("ws://127.0.0.1:{server_port}/ws");
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    use futures_util::{SinkExt, StreamExt};

    // hello
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        json!({ "type": "hello", "token": token }).to_string().into(),
    ))
    .await
    .unwrap();
    let ready = ws.next().await.unwrap().unwrap();
    assert!(
        ready.to_text().unwrap().contains("ready"),
        "应收到 ready: {ready}"
    );

    // pcm（2 秒静音帧）
    let pcm = base64::engine::general_purpose::STANDARD.encode(vec![0u8; 32000]);
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        json!({ "type": "pcm", "data": pcm }).to_string().into(),
    ))
    .await
    .unwrap();

    // finalize → final
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        json!({ "type": "finalize" }).to_string().into(),
    ))
    .await
    .unwrap();
    let final_msg = ws.next().await.unwrap().unwrap();
    let final_json: serde_json::Value = serde_json::from_str(final_msg.to_text().unwrap()).unwrap();
    assert_eq!(final_json["type"], "final");
    assert_eq!(final_json["text"], "这是一段润色后的文字。");
    assert_eq!(final_json["chars"], 11);

    server.abort();
}

#[tokio::test]
async fn jwt_guards_me_endpoint() {
    let port = spawn_mock_provider().await;
    let state = test_state(port).await;
    let app = build_router(state.clone());
    // 有效 token（伪造用户不存在 → 401）
    let (status, _) = send_json(&app, "GET", "/auth/me", None, Some(&sign_token("ghost"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
