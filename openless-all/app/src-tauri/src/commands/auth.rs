//! Auth IPC commands — registration, login, logout, session validation.

use super::*;

#[tauri::command]
pub fn register(
    coord: CoordinatorState<'_>,
    req: crate::auth::RegisterRequest,
) -> Result<crate::auth::PublicUser, String> {
    coord.auth().register(&req)
}

#[tauri::command]
pub fn login(
    coord: CoordinatorState<'_>,
    req: crate::auth::LoginRequest,
) -> Result<crate::auth::PublicUser, String> {
    let (session, user) = coord.auth().login(&req)?;
    coord.set_active_user(user.id.clone());
    Ok(user)
}

#[tauri::command]
pub fn logout(coord: CoordinatorState<'_>, token: String) -> Result<(), String> {
    coord.auth().logout(&token)?;
    coord.clear_active_user();
    Ok(())
}

#[tauri::command]
pub fn validate_session(
    coord: CoordinatorState<'_>,
    token: String,
) -> Result<crate::auth::PublicUser, String> {
    coord.auth().validate_session(&token)
}

#[tauri::command]
pub fn get_auth_status(coord: CoordinatorState<'_>) -> crate::auth::AuthStatus {
    let user_id = coord.active_user_id();
    coord.auth().status(user_id.as_deref())
}

#[tauri::command]
pub fn list_users(
    coord: CoordinatorState<'_>,
) -> Result<Vec<crate::auth::PublicUser>, String> {
    coord.auth().list_users()
}
