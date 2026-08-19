//! JWT 认证（HMAC-SHA256）+ bcrypt + sqlx 用户存储（SQLite 起步，Postgres 可切换）。

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::SqlitePool;

use crate::SharedState;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub created_at: String,
    pub is_admin: bool,
}

/// 用户存储（连接池 + users 表）。
#[derive(Clone)]
pub struct UserStore {
    pool: SqlitePool,
}

impl UserStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 启动时建表（幂等）。
    pub async fn init(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                is_admin INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 插入用户；邮箱已存在返回 Err("邮箱已注册")。
    pub async fn insert(&self, user: &User, password_hash: &str) -> Result<(), String> {
        let result = sqlx::query(
            "INSERT INTO users (id, email, password_hash, created_at, is_admin) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&user.id)
        .bind(&user.email)
        .bind(password_hash)
        .bind(&user.created_at)
        .bind(user.is_admin as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        if result.rows_affected() == 0 {
            return Err("写入失败".into());
        }
        Ok(())
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<(User, String)>, String> {
        let row = sqlx::query_as::<_, User>(
            "SELECT id, email, created_at, is_admin FROM users WHERE email = ?1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        if let Some(user) = row {
            let hash = sqlx::query_scalar::<_, String>(
                "SELECT password_hash FROM users WHERE id = ?1",
            )
            .bind(&user.id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
            Ok(Some((user, hash)))
        } else {
            Ok(None)
        }
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<User>, String> {
        sqlx::query_as::<_, User>(
            "SELECT id, email, created_at, is_admin FROM users WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| e.to_string())
    }
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
    if let Err(e) = state.users.insert(&user, &password_hash).await {
        if e.contains("UNIQUE") || e.contains("已注册") || e.contains("constraint") {
            return (StatusCode::CONFLICT, Json(AuthError::new("邮箱已注册"))).into_response();
        }
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(AuthError::new(&e))).into_response();
    }
    let token = sign_token(&user.id);
    (StatusCode::CREATED, Json(AuthResponse { token, user })).into_response()
}

pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let Some((user, hash)) = state.users.get_by_email(&req.email).await.unwrap_or(None) else {
        return (StatusCode::UNAUTHORIZED, Json(AuthError::new("邮箱或密码错误"))).into_response();
    };
    if !bcrypt::verify(&req.password, &hash).unwrap_or(false) {
        return (StatusCode::UNAUTHORIZED, Json(AuthError::new("邮箱或密码错误"))).into_response();
    }
    let token = sign_token(&user.id);
    (StatusCode::OK, Json(AuthResponse { token, user })).into_response()
}

/// 从 Authorization: Bearer <token> 中解析用户（异步，查库）。
pub async fn user_from_bearer(state: &SharedState, auth_header: Option<&str>) -> Result<User, String> {
    let header = auth_header.ok_or("缺少 Authorization 头")?;
    let token = header.strip_prefix("Bearer ").ok_or("Authorization 格式错误")?;
    let user_id = verify_token(token)?;
    state
        .users
        .find_by_id(&user_id)
        .await?
        .ok_or_else(|| "用户不存在".into())
}

pub async fn me(State(state): State<SharedState>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    let user = match user_from_bearer(&state, headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok())).await {
        Ok(u) => u,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(AuthError::new(&e))).into_response(),
    };
    let quota = state.quotas.status(&user.id).await;
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
