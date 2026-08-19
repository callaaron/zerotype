//! WS 流式会话协议（移动端/桌面的实时通道骨架）。
//!
//! 消息格式（JSON，逐行帧）：
//!   客户端 → {\"type\":\"hello\",\"token\":\"<jwt>\"}
//!   客户端 → {\"type\":\"pcm\",\"data\":\"<base64 16k mono s16le>\"}
//!   客户端 → {\"type\":\"finalize\"}
//!   服务端 ← {\"type\":\"ready\"}
//!   服务端 ← {\"type\":\"final\",\"text\":\"...\",\"raw_text\":\"...\",\"chars\":N}

use axum::extract::ws::{Message, WebSocket};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};

use crate::auth::{user_from_bearer, User};
use crate::SharedState;

/// 处理一个流式会话：认证 → 收 PCM → finalize 时转写+润色 → 回传结果。
pub async fn handle_session(state: SharedState, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let mut user: Option<User> = None;
    let mut pcm: Vec<u8> = Vec::new();

    while let Some(Ok(msg)) = receiver.next().await {
        let Message::Text(text) = msg else {
            continue;
        };
        let Ok(frame) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        match frame.get("type").and_then(|v| v.as_str()) {
            Some("hello") => {
                let token = frame.get("token").and_then(|v| v.as_str()).unwrap_or("");
                let bearer = format!("Bearer {token}");
                match user_from_bearer(&state, Some(&bearer)).await {
                    Ok(u) => {
                        user = Some(u);
                        let _ = sender.send(Message::Text(json!({ "type": "ready" }).to_string().into())).await;
                    }
                    Err(e) => {
                        let _ = sender.send(Message::Text(json!({ "type": "error", "error": e }).to_string().into())).await;
                        break;
                    }
                }
            }
            Some("pcm") => {
                if let Some(data) = frame.get("data").and_then(|v| v.as_str()) {
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
                        pcm.extend_from_slice(&bytes);
                    }
                }
            }
            Some("finalize") => {
                let Some(u) = user.as_ref() else {
                    let _ = sender.send(Message::Text(json!({ "type": "error", "error": "未认证" }).to_string().into())).await;
                    break;
                };
                let result = crate::dictate::run_pipeline(&state, &u, &pcm, &[], None).await;
                match result {
                    Ok(resp) => {
                        let _ = sender
                            .send(Message::Text(
                                json!({
                                    "type": "final",
                                    "text": resp.text,
                                    "raw_text": resp.raw_text,
                                    "chars": resp.chars,
                                    "quota": resp.quota,
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                    }
                    Err(e) => {
                        let _ = sender.send(Message::Text(json!({ "type": "error", "error": e }).to_string().into())).await;
                    }
                }
                pcm.clear();
            }
            _ => {}
        }
    }
}
