//! JWT 认证（HMAC-SHA256）与进程内用户存储（Postgres 待接入）。

use std::collections::HashMap;
use std::sync::Mutex;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::SharedState;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub created_at: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
struct UserRecord {
    user: User,
    password_hash: String,
}

#[derive(Default)]
pub struct UserStore {
    inner: Mutex<HashMap<String, UserRecord>>, // key: email
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

fn jwt_secret() -> String {
    std::env::var("ZEROTYPE_JWT_SECRET")
        .unwrap_or_else(|_| "dev-only-secret-change-me".into())
}

fn b64url(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// 签发 HMAC-SHA256 JWT（header.payload.signature）。
pub fn sign_token(user_id: &str) -> String {
    let header = b64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let now = chrono::Utc::now().timestamp();
    let payload = b64url(format!("{{\"sub\":\"{user_id}\",\"iat\":{now}}}").as_bytes());
    let signing_input = format!("{header}.{payload}");
    let mut mac = HmacSha256::new_from_slice(jwt_secret().as_bytes()).expect("hmac key");
    mac.update(signing_input.as_bytes());
    let sig = b64url(&mac.finalize().into_bytes());
    format!("{signing_input}.{sig}")
}
/// 校验 JWT，返回 user_id。非法/过期即 Err。
pub fn verify_token(token: &str) -> Result<String, String> {
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(sig)) = (parts.next(), parts.next(), parts.next()) else {
        return Err("token 格式错误".into());
    };
    let expected = {
        let mut mac = HmacSha256::new_from_slice(jwt_secret().as_bytes()).expect("hmac key");
        mac.update(format!("{header}.{payload}").as_bytes());
        b64url(&mac.finalize().into_bytes())
    };
    if expected != sig {
        return Err("签名无效".into());
    }
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| "payload 解码失败".to_string())?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).map_err(|_| "payload 解析失败".to_string())?;
    claims
        .get("sub")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "缺少 sub".into())
}

pub async fn register(
    State(state): State<SharedState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    if req.password.len() < 6 {
        return (StatusCode::BAD_REQUEST, Json(AuthError::new("密码至少 6 位"))).into_response();
    }
    let password_hash = match bcrypt::hash(&req.password, 12) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError::new(&format!("哈希失败: {e}")))).into_response(),
    };
    let user = User {
        id: uuid::Uuid::new_v4().to_string(),
        email: req.email.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        is_admin: false,
    };
    let mut store = state.users.inner.lock().unwrap();
    if store.contains_key(&user.email) {
        return (StatusCode::CONFLICT, Json(AuthError::new("邮箱已注册"))).into_response();
    }
    store.insert(
        user.email.clone(),
        UserRecord { user: user.clone(), password_hash },
    );
    drop(store);
    let token = sign_token(&user.id);
    (StatusCode::CREATED, Json(AuthResponse { token, user })).into_response()
}

pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let store = state.users.inner.lock().unwrap();
    let Some(record) = store.get(&req.email) else {
        return (StatusCode::UNAUTHORIZED, Json(AuthError::new("邮箱或密码错误"))).into_response();
    };
    if !bcrypt::verify(&req.password, &record.password_hash).unwrap_or(false) {
        return (StatusCode::UNAUTHORIZED, Json(AuthError::new("邮箱或密码错误"))).into_response();
    }
    let token = sign_token(&record.user.id);
    (StatusCode::OK, Json(AuthResponse { token, user: record.user.clone() })).into_response()
}

/// 从 Authorization: Bearer <token> 中解析用户。
pub fn user_from_bearer(state: &SharedState, auth_header: Option<&str>) -> Result<User, String> {
    let header = auth_header.ok_or("缺少 Authorization 头")?;
    let token = header.strip_prefix("Bearer ").ok_or("Authorization 格式错误")?;
    let user_id = verify_token(token)?;
    let store = state.users.inner.lock().unwrap();
    store
        .values()
        .find(|r| r.user.id == user_id)
        .map(|r| r.user.clone())
        .ok_or_else(|| "用户不存在".into())
}

pub async fn me(State(state): State<SharedState>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    let user = match user_from_bearer(&state, headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok())) {
        Ok(u) => u,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(AuthError::new(&e))).into_response(),
    };
    let quota = state.quotas.status(&user.id);
    (StatusCode::OK, Json(serde_json::json!({ "user": user, "quota": quota }))).into_response()
}

#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
}

impl AuthError {
    fn new(msg: &str) -> Self {
        Self { error: msg.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_roundtrip() {
        let token = sign_token("user-123");
        assert_eq!(verify_token(&token).unwrap(), "user-123");
    }

    #[test]
    fn jwt_rejects_tampered_token() {
        let token = sign_token("user-123");
        let tampered = format!("{}x", token);
        assert!(verify_token(&tampered).is_err());
    }
}

