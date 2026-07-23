//! Auth data types for ZeroType user system.

use serde::{Deserialize, Serialize};

/// A registered user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// bcrypt hash of the password.
    #[serde(skip_serializing)]
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
