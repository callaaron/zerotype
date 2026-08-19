//! ZeroType user authentication system.
//!
//! ## Architecture
//!
//! ```text
//! Login/Register → AuthStore (JSON file, bcrypt passwords)
//!                     │
//!                     ├── User records: {id, email, phone, password_hash, created_at}
//!                     └── Sessions: {token (UUID), user_id, expires_at}
//! ```
//!
//! ## Design Decisions
//! - Local-first: no external auth service, pure JSON file storage
//! - bcrypt for passwords (cost factor 12)
//! - UUID session tokens (simpler than JWT for local use)
//! - Session expiry: 30 days
//! - One active session per device (last login wins)

pub mod store;
pub mod types;

pub use store::AuthStore;
pub use types::{AuthStatus, AuthUser, LoginRequest, PublicUser, RegisterRequest, Session, UserCredential};
