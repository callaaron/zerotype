//! Billing IPC commands — quota checking, usage recording, status queries.

use super::*;

#[tauri::command]
pub fn get_quota_status(coord: CoordinatorState<'_>) -> Result<crate::billing::QuotaStatus, String> {
    let user_id = coord
        .active_user_id()
        .ok_or("请先登录".to_string())?;
    coord.billing().get_quota_status(&user_id)
}

#[tauri::command]
pub fn check_quota_before_record(
    coord: CoordinatorState<'_>,
    char_count: u64,
) -> Result<bool, String> {
    let user_id = coord
        .active_user_id()
        .ok_or("请先登录".to_string())?;
    let action = coord.billing().check_quota(&user_id, char_count)?;
    Ok(matches!(action, crate::billing::QuotaAction::Allow))
}

#[tauri::command]
pub fn admin_set_user_quota(
    coord: CoordinatorState<'_>,
    user_id: String,
    limit: u64,
) -> Result<(), String> {
    // P0 修复：此前无任何鉴权，任意调用方可把任意用户配额改为无限。
    crate::commands::auth::require_admin(&coord)?;
    coord.billing().set_override_limit(&user_id, limit)
}

#[tauri::command]
pub fn get_weekly_free_chars() -> u64 {
    crate::billing::WEEKLY_FREE_CHARS
}
