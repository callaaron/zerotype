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

    /// 测试专用构造器：使用显式目录，避免依赖全局 HOME 环境变量，
    /// 从而支持并行测试互不干扰。
    #[cfg(test)]
    pub(crate) fn with_dir(dir: PathBuf) -> Result<Self> {
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
            // 第一个注册的用户自动成为管理员（无独立晋升流程时的最简单安全模型）。
            is_admin: file.users.is_empty(),
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

    /// Get auth status for the currently active user, resolved by *user id*
    /// (not by session token). The desktop app tracks a single in-memory active
    /// user; validating by id avoids the historical bug where the user UUID was
    /// mistaken for a session token, which made get_auth_status always report
    /// logged_out.
    pub fn status_for_user(&self, user_id: Option<&str>) -> AuthStatus {
        match user_id {
            Some(id) => match self.get_user_by_id(id) {
                Ok(Some(user)) => AuthStatus {
                    logged_in: true,
                    user: Some(user),
                },
                _ => AuthStatus {
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

    /// Look up a user by id (no password). Returns Ok(None) when not found.
    pub fn get_user_by_id(&self, user_id: &str) -> Result<Option<PublicUser>, String> {
        let _guard = self.lock.lock();
        let file = self.read_users()?;
        Ok(file
            .users
            .iter()
            .find(|u| u.id == user_id)
            .map(PublicUser::from))
    }

    /// Create a new session for a user without re-verifying a password.
    /// Used by the register flow to sign the user in immediately.
    pub fn create_session_for_user(&self, user_id: &str) -> Result<Session, String> {
        let _guard = self.lock.lock();
        let mut sessions_file = self.read_sessions()?;
        let token = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::days(SESSION_DAYS);
        let session = Session {
            token,
            user_id: user_id.to_string(),
            created_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        };
        // Remove old sessions for this user.
        sessions_file.sessions.retain(|s| s.user_id != user_id);
        sessions_file.sessions.push(session.clone());
        self.write_sessions(&sessions_file)?;
        Ok(session)
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
        atomic_write(&self.users_path, &json).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.users_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn read_sessions(&self) -> Result<SessionsFile, String> {
        read_or_default::<SessionsFile>(&self.sessions_path).map_err(|e| e.to_string())
    }

    fn write_sessions(&self, file: &SessionsFile) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(file).map_err(|e| format!("encode sessions failed: {e}"))?;
        atomic_write(&self.sessions_path, &json).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.sessions_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 每个测试用独立临时目录，直接构造 store（不碰全局 HOME，可并行）。
    fn isolated_store() -> AuthStore {
        let tmp = std::env::temp_dir().join(format!("zerotype-auth-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).expect("create temp dir");
        AuthStore::with_dir(tmp).expect("store init")
    }

    fn register(store: &AuthStore, email: &str, password: &str) -> PublicUser {
        store
            .register(&RegisterRequest {
                email: Some(email.into()),
                phone: None,
                password: password.into(),
            })
            .expect("register ok")
    }

    #[test]
    fn first_registered_user_is_admin_second_is_not() {
        let store = isolated_store();
        let first = register(&store, "a@example.com", "secret1");
        let second = register(&store, "b@example.com", "secret2");
        assert!(first.is_admin, "首个注册用户应为管理员");
        assert!(!second.is_admin, "后续用户不应是管理员");
    }

    #[test]
    fn get_user_by_id_finds_user_and_hides_password() {
        let store = isolated_store();
        let u = register(&store, "x@example.com", "secret1");
        let found = store.get_user_by_id(&u.id).unwrap().expect("user exists");
        assert_eq!(found.id, u.id);
        assert_eq!(found.email.as_deref(), Some("x@example.com"));
        // PublicUser 是独立结构，本身不含 password/password_hash 字段。
        let json = serde_json::to_string(&found).unwrap();
        assert!(!json.contains("password"), "PublicUser 序列化不应含密码");
    }

    #[test]
    fn get_user_by_id_returns_none_for_unknown() {
        let store = isolated_store();
        assert!(store.get_user_by_id("no-such-user").unwrap().is_none());
    }

    #[test]
    fn status_for_user_reflects_active_user_by_id() {
        let store = isolated_store();
        let u = register(&store, "s@example.com", "secret1");
        assert!(store.status_for_user(None).logged_in == false);
        let st = store.status_for_user(Some(&u.id));
        assert!(st.logged_in);
        assert_eq!(st.user.unwrap().id, u.id);
        assert!(store.status_for_user(Some("unknown")).logged_in == false);
    }

    #[test]
    fn create_session_for_user_persists_session_and_replaces_old() {
        let store = isolated_store();
        let u = register(&store, "t@example.com", "secret1");
        let s1 = store.create_session_for_user(&u.id).unwrap();
        let s2 = store.create_session_for_user(&u.id).unwrap();
        assert_ne!(s1.token, s2.token, "每次创建应生成新 token");
        // 同用户旧会话被替换：只剩一个新会话。
        let sessions: SessionsFile = read_or_default(&store.sessions_path).unwrap();
        let active: Vec<_> = sessions.sessions.iter().filter(|s| s.user_id == u.id).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].token, s2.token);
    }

    #[test]
    fn validate_session_accepts_created_session() {
        let store = isolated_store();
        let u = register(&store, "v@example.com", "secret1");
        let session = store.create_session_for_user(&u.id).unwrap();
        let user = store.validate_session(&session.token).unwrap();
        assert_eq!(user.id, u.id);
        assert!(store.validate_session("bogus-token").is_err());
    }

    #[test]
    fn duplicate_email_registration_rejected() {
        let store = isolated_store();
        register(&store, "d@example.com", "secret1");
        let dup = store.register(&RegisterRequest {
            email: Some("d@example.com".into()),
            phone: None,
            password: "secret2".into(),
        });
        assert!(dup.is_err(), "重复邮箱应被拒绝");
    }

    #[test]
    fn register_requires_email_or_phone() {
        let store = isolated_store();
        let err = store
            .register(&RegisterRequest {
                email: None,
                phone: None,
                password: "secret1".into(),
            })
            .unwrap_err();
        assert!(err.contains("邮箱或手机号"));
    }
}

