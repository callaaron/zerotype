//! Auth IPC commands — registration, login, logout, session validation.

use super::*;

#[tauri::command]
pub fn register(
    coord: CoordinatorState<'_>,
    req: crate::auth::RegisterRequest,
) -> Result<crate::auth::PublicUser, String> {
    let user = coord.auth().register(&req)?;
    // 注册成功即自动登录：创建会话并设为活跃用户（首个注册用户同时是管理员）。
    if coord
        .auth()
        .create_session_for_user(&user.id)
        .is_ok()
    {
        coord.set_active_user(user.id.clone());
    }
    Ok(user)
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
    // 按 user_id 校验活跃用户（历史 bug：把 user_id 当 session token 传，
    // 导致登录后永远显示未登录）。
    coord.auth().status_for_user(user_id.as_deref())
}

/// 校验当前活跃用户是否为管理员。未登录或非管理员一律拒绝。
///
/// P0 修复：list_users / admin_set_user_quota 此前无任何鉴权，
/// 任意 IPC 调用方可枚举全部用户（含 email/phone）或改写任意配额。
pub(crate) fn require_admin(coord: &Coordinator) -> Result<(), String> {
    let current = coord.active_user_id().ok_or("请先登录")?;
    let user = coord
        .auth()
        .get_user_by_id(&current)
        .map_err(|e| e.to_string())?
        .ok_or("用户不存在")?;
    if !user.is_admin {
        return Err("需要管理员权限".into());
    }
    Ok(())
}

#[tauri::command]
pub fn list_users(
    coord: CoordinatorState<'_>,
) -> Result<Vec<crate::auth::PublicUser>, String> {
    require_admin(&coord)?;
    coord.auth().list_users()
}
