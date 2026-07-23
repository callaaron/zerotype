// Billing IPC bindings for ZeroType.
import { invokeOrMock } from "./shared"

export interface QuotaStatus {
  charsUsed: number
  charsLimit: number
  charsRemaining: number
  weekId: string
  isOverLimit: boolean
  percentUsed: number
}

export function getQuotaStatus(): Promise<QuotaStatus> {
  return invokeOrMock("get_quota_status", undefined, () => ({
    charsUsed: 2500,
    charsLimit: 10000,
    charsRemaining: 7500,
    weekId: "2026-W30",
    isOverLimit: false,
    percentUsed: 25,
  }))
}

export function checkQuotaBeforeRecord(charCount: number): Promise<boolean> {
  return invokeOrMock("check_quota_before_record", { charCount }, () => true)
}

export function adminSetUserQuota(userId: string, limit: number): Promise<void> {
  return invokeOrMock("admin_set_user_quota", { userId, limit }, () => undefined)
}

export function getWeeklyFreeChars(): Promise<number> {
  return invokeOrMock("get_weekly_free_chars", undefined, () => 10000)
}
