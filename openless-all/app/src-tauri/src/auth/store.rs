//! AuthStore — persistent user and session storage.
//!
//! Stores data in `users.json` and `sessions.json` in the app data directory.
//! Follows the same pattern as DictionaryStore/HistoryStore from persistence module.

use std::path::PathBuf;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use uuid::Uuid;

use super::types::*;
use crate::persistence::{atomic_write, data_dir, ensure_dir, read_or_default};

const USERS_FILE: &str = "zerotype-users.json";
const SESSIONS_FILE: &str = "zerotype-sessions.json";
const BCRYPT_COST: u32 = 12;
const SESSION_DAYS: i64 = 30;

/// In-memory user store backed by a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsersFile {
    users: Vec<AuthUser>,
}

impl Default for UsersFile {
    fn default() -> Self {
        Self { users: Vec::new() }
    }
}

/// In-memory session store backed by a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionsFile {
    sessions: Vec<Session>,
}

impl Default for SessionsFile {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }
}

use serde::{Deserialize, Serialize};

pub struct AuthStore {
    users_path: PathBuf,
    sessions_path: PathBuf,
    lock: Mutex<()>,
}

impl AuthStore {
    pub fn new() -> Result<Self> {
        let dir = data_dir()?;
        ensure_dir(&dir)?;
        Ok(Self {
            users_path: dir.join(USERS_FILE),
            sessions_path: dir.join(SESSIONS_FILE),
            lock: Mutex::new(()),
        })
    }

    // ─── Registration ────────────────────────────────────────

    /// Register a new user. Returns the created user (without password).
    pub fn register(&self, req: &RegisterRequest) -> Result<PublicUser, String> {
        validate_password(&req.password)?;

        if let Some(ref email) = req.email {
            validate_email(email)?;
        }
        if let Some(ref phone) = req.phone {
            validate_phone(phone)?;
        }
        if req.email.is_none() && req.phone.is_none() {
            return Err("邮箱或手机号至少填一个".into());
        }

        let _guard = self.lock.lock();
        let mut file = self.read_users()?;

        // Check for duplicate email or phone
        if let Some(ref email) = req.email {
            if file.users.iter().any(|u| u.email.as_deref() == Some(email.as_str())) {
                return Err("该邮箱已注册".into());
            }
        }
        if let Some(ref phone) = req.phone {
            if file.users.iter().any(|u| u.phone.as_deref() == Some(phone.as_str())) {
                return Err("该手机号已注册".into());
            }
        }

        let password_hash = bcrypt::hash(&req.password, BCRYPT_COST).map_err(|e| e.to_string())?;
        let user = AuthUser {
            id: Uuid::new_v4().to_string(),
            email: req.email.clone(),
            phone: req.phone.clone(),
            password_hash,
            created_at: chrono::Utc::now().to_rfc3339(),
            is_admin: false, // First user becomes admin via separate flow
        };

        let public = PublicUser::from(&user);
        file.users.push(user);
        self.write_users(&file)?;
        Ok(public)
    }

    // ─── Login ───────────────────────────────────────────────

    /// Authenticate a user and create a session. Returns the session token.
    pub fn login(&self, req: &LoginRequest) -> Result<(Session, PublicUser), String> {
        let _guard = self.lock.lock();
        let users_file = self.read_users()?;
        let mut sessions_file = self.read_sessions()?;

        let cred = UserCredential::from_str(&req.credential);
        let user = users_file
            .users
            .iter()
            .find(|u| match &cred {
                UserCredential::Email(e) => u.email.as_deref() == Some(e.as_str()),
                UserCredential::Phone(p) => u.phone.as_deref() == Some(p.as_str()),
            })
            .ok_or("用户不存在")?;

        let valid = bcrypt::verify(&req.password, &user.password_hash).unwrap_or(false);
        if !valid {
            return Err("密码错误".into());
        }

        // Create session
        let token = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::days(SESSION_DAYS);
        let session = Session {
            token: token.clone(),
            user_id: user.id.clone(),
            created_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        };

        // Remove old sessions for this user
        sessions_file
            .sessions
            .retain(|s| s.user_id != user.id);
        sessions_file.sessions.push(session.clone());
        self.write_sessions(&sessions_file)?;

        Ok((session, PublicUser::from(user)))
    }

    /// Validate a session token. Returns the user if valid.
    pub fn validate_session(&self, token: &str) -> Result<PublicUser, String> {
        let _guard = self.lock.lock();
        let sessions_file = self.read_sessions()?;
        let users_file = self.read_users()?;

        let session = sessions_file
            .sessions
            .iter()
            .find(|s| s.token == token)
            .ok_or("会话无效，请重新登录")?;

        // Check expiry
        let expires = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
            .map_err(|_| "会话数据损坏".to_string())?;
        if chrono::Utc::now() > expires {
            return Err("会话已过期，请重新登录".into());
        }

        let user = users_file
            .users
            .iter()
            .find(|u| u.id == session.user_id)
            .ok_or("用户不存在")?;

        Ok(PublicUser::from(user))
    }

    /// Log out by removing the session.
    pub fn logout(&self, token: &str) -> Result<(), String> {
        let _guard = self.lock.lock();
        let mut file = self.read_sessions()?;
        file.sessions.retain(|s| s.token != token);
        self.write_sessions(&file).map_err(|e| e.to_string())
    }

    /// Get current auth status for a token.
    pub fn status(&self, token: Option<&str>) -> AuthStatus {
        match token {
            Some(t) => match self.validate_session(t) {
                Ok(user) => AuthStatus {
                    logged_in: true,
                    user: Some(user),
                },
                Err(_) => AuthStatus {
                    logged_in: false,
                    user: None,
                },
            },
            None => AuthStatus {
                logged_in: false,
                user: None,
            },
        }
    }

    /// Get all users (admin only).
    pub fn list_users(&self) -> Result<Vec<PublicUser>, String> {
        let _guard = self.lock.lock();
        let file = self.read_users()?;
        Ok(file.users.iter().map(PublicUser::from).collect())
    }

    /// Clean up expired sessions.
    pub fn cleanup_sessions(&self) -> Result<usize, String> {
        let _guard = self.lock.lock();
        let mut file = self.read_sessions()?;
        let before = file.sessions.len();
        let now = chrono::Utc::now();
        file.sessions.retain(|s| {
            chrono::DateTime::parse_from_rfc3339(&s.expires_at)
                .map(|expires| now <= expires)
                .unwrap_or(false)
        });
        let removed = before - file.sessions.len();
        if removed > 0 {
            self.write_sessions(&file).map_err(|e| e.to_string())?;
        }
        Ok(removed)
    }

    // ─── Private helpers ─────────────────────────────────────

    fn read_users(&self) -> Result<UsersFile, String> {
        read_or_default::<UsersFile>(&self.users_path).map_err(|e| e.to_string())
    }

    fn write_users(&self, file: &UsersFile) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(file).map_err(|e| format!("encode users failed: {e}"))?;
        atomic_write(&self.users_path, &json).map_err(|e| e.to_string())
    }

    fn read_sessions(&self) -> Result<SessionsFile, String> {
        read_or_default::<SessionsFile>(&self.sessions_path).map_err(|e| e.to_string())
    }

    fn write_sessions(&self, file: &SessionsFile) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(file).map_err(|e| format!("encode sessions failed: {e}"))?;
        atomic_write(&self.sessions_path, &json).map_err(|e| e.to_string())
    }
}
