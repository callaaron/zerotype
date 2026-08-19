//! Auth data types for ZeroType user system.

use serde::{Deserialize, Serialize};

/// A registered user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// bcrypt hash of the password。
    ///
    /// 必须持久化：登录校验依赖重新读取 hash。此前误用 skip_serializing 导致
    /// 写盘后缺字段、重读 decode 失败——注册后登录必然失败（P0）。
    /// bcrypt hash 是单向散列，本地 JSON 存储可接受（见 write_users 的 0600 权限）。
    pub password_hash: String,
    pub created_at: String,
    pub is_admin: bool,
}

/// Registration request from frontend.
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterRequest {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub password: String,
}

/// Login request from frontend.
#[derive(Debug, Clone, Deserialize)]
pub struct LoginRequest {
    pub credential: String, // email or phone
    pub password: String,
}

/// Login credential type detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserCredential {
    Email(String),
    Phone(String),
}

impl UserCredential {
    pub fn from_str(s: &str) -> Self {
        if s.contains('@') {
            UserCredential::Email(s.to_string())
        } else {
            UserCredential::Phone(s.to_string())
        }
    }
}

/// An active login session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub created_at: String,
    /// ISO-8601: session expires after this time.
    pub expires_at: String,
}

/// Auth status response for frontend.
#[derive(Debug, Clone, Serialize)]
pub struct AuthStatus {
    pub logged_in: bool,
    pub user: Option<PublicUser>,
}

/// Public user info (no password).
#[derive(Debug, Clone, Serialize)]
pub struct PublicUser {
    pub id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub created_at: String,
    pub is_admin: bool,
}

impl From<&AuthUser> for PublicUser {
    fn from(u: &AuthUser) -> Self {
        PublicUser {
            id: u.id.clone(),
            email: u.email.clone(),
            phone: u.phone.clone(),
            created_at: u.created_at.clone(),
            is_admin: u.is_admin,
        }
    }
}

/// Simple input validation for auth fields.
pub fn validate_email(email: &str) -> Result<(), String> {
    let trimmed = email.trim();
    if trimmed.is_empty() || !trimmed.contains('@') || trimmed.len() < 5 {
        return Err("无效的邮箱地址".into());
    }
    Ok(())
}

pub fn validate_phone(phone: &str) -> Result<(), String> {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 || digits.len() > 15 {
        return Err("无效的手机号码".into());
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < 6 {
        return Err("密码至少需要6个字符".into());
    }
    if password.len() > 128 {
        return Err("密码不能超过128个字符".into());
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_detects_email_vs_phone() {
        assert!(matches!(
            UserCredential::from_str("a@b.com"),
            UserCredential::Email(_)
        ));
        assert!(matches!(
            UserCredential::from_str("13800138000"),
            UserCredential::Phone(_)
        ));
    }

    #[test]
    fn email_validation() {
        assert!(validate_email("a@b.com").is_ok());
        assert!(validate_email("").is_err());
        assert!(validate_email("not-an-email").is_err());
        assert!(validate_email("ab").is_err());
    }

    #[test]
    fn phone_validation() {
        assert!(validate_phone("13800138000").is_ok());
        assert!(validate_phone("123").is_err());
        assert!(validate_phone("12345678901234567890").is_err());
    }

    #[test]
    fn password_validation() {
        assert!(validate_password("123456").is_ok());
        assert!(validate_password("12345").is_err());
        assert!(validate_password(&"x".repeat(129)).is_err());
    }

    #[test]
    fn auth_user_roundtrips_with_password_hash() {
        // password_hash 必须能写盘再读回（登录校验依赖它）。回归测试：
        // 曾误用 skip_serializing 导致 roundtrip decode 失败。
        let user = AuthUser {
            id: "u1".into(),
            email: Some("a@b.com".into()),
            phone: None,
            password_hash: "super-secret-hash".into(),
            created_at: "2026-07-23T00:00:00Z".into(),
            is_admin: true,
        };
        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("super-secret-hash"), "hash 必须持久化");
        let back: AuthUser = serde_json::from_str(&json).unwrap();
        assert_eq!(back.password_hash, "super-secret-hash");
    }

    #[test]
    fn public_user_never_serializes_password() {
        let user = AuthUser {
            id: "u1".into(),
            email: Some("a@b.com".into()),
            phone: None,
            password_hash: "super-secret-hash".into(),
            created_at: "2026-07-23T00:00:00Z".into(),
            is_admin: true,
        };
        let public = PublicUser::from(&user);
        let json = serde_json::to_string(&public).unwrap();
        assert!(!json.contains("secret"));
        assert_eq!(public.is_admin, true);
    }
}

