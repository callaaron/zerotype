// AccountSection — ZeroType account & billing in settings.
import { useEffect, useState } from 'react'
import { Card } from '../_atoms'
import { SettingRow, SectionTitle } from './shared'
import * as authIpc from '../../lib/ipc/auth'
import * as billingIpc from '../../lib/ipc/billing'
import type { AuthUser, AuthStatus } from '../../lib/ipc/auth'
import type { QuotaStatus } from '../../lib/ipc/billing'

import { LoginDialog } from '../../components/LoginDialog'

export function AccountSection() {
  const [auth, setAuth] = useState<AuthStatus | null>(null)
  const [quota, setQuota] = useState<QuotaStatus | null>(null)
  const [showLogin, setShowLogin] = useState(false)

  const reload = () => {
    authIpc.getAuthStatus().then(setAuth)
    billingIpc.getQuotaStatus().then(setQuota).catch(() => {})
  }

  useEffect(() => { reload() }, [])

  if (!auth?.loggedIn) {
    return (
      <>
        <Card>
          <SectionTitle>账户</SectionTitle>
          <p style={{ fontSize: 13, color: 'var(--ol-ink-3)', padding: '0 0 12px' }}>
            登录后可使用热词学习、声纹识别、用量统计等功能
          </p>
          <button
            onClick={() => setShowLogin(true)}
            style={{
              padding: '6px 20px',
              borderRadius: 8,
              border: 'none',
              background: 'var(--ol-accent)',
              color: '#fff',
              fontSize: 13,
              cursor: 'pointer',
            }}
          >
            登录 / 注册
          </button>
        </Card>
        {showLogin && (
          <LoginDialog
            onLogin={() => { setShowLogin(false); reload() }}
          />
        )}
      </>
    )
  }

  const u = auth.user!

  return (
    <Card>
      <SectionTitle>账户</SectionTitle>
      <SettingRow label="用户">
        <span style={{ fontSize: 13, color: 'var(--ol-ink-1)' }}>
          {u.email ?? u.phone ?? u.id.slice(0, 8)}
        </span>
      </SettingRow>
      <SettingRow label="类型">
        <span style={{
          fontSize: 11,
          padding: '2px 8px',
          borderRadius: 999,
          background: u.isAdmin ? 'var(--ol-blue-soft)' : 'var(--ol-bg-3)',
          color: u.isAdmin ? 'var(--ol-blue)' : 'var(--ol-ink-3)',
        }}>
          {u.isAdmin ? '管理员' : '普通用户'}
        </span>
      </SettingRow>

      <div style={{ marginTop: 24 }} />
      <SectionTitle>本周用量</SectionTitle>
      {quota ? (
        <>
          <SettingRow label="已用">
            <span style={{ fontSize: 13, fontVariantNumeric: 'tabular-nums' }}>
              {quota.charsUsed.toLocaleString()} / {quota.charsLimit.toLocaleString()} 字
            </span>
          </SettingRow>
          <SettingRow label="剩余">
            <span style={{
              fontSize: 13,
              color: quota.charsRemaining < 1000 ? '#ef4444' : 'var(--ol-ink-2)',
            }}>
              {quota.charsRemaining.toLocaleString()} 字
            </span>
          </SettingRow>
          <div style={{ marginTop: 8 }}>
            <div style={{
              height: 6,
              borderRadius: 3,
              background: 'var(--ol-bg-3)',
              overflow: 'hidden',
            }}>
              <div style={{
                height: '100%',
                width: `${Math.min(quota.percentUsed, 100)}%`,
                borderRadius: 3,
                background: quota.percentUsed > 80 ? '#ef4444' : 'var(--ol-accent)',
                transition: 'width 0.3s',
              }} />
            </div>
            <div style={{ textAlign: 'right', fontSize: 11, color: 'var(--ol-ink-3)', marginTop: 4 }}>
              本周 {quota.weekId}
            </div>
          </div>
          {quota.isOverLimit && (
            <div style={{
              marginTop: 12,
              padding: '8px 12px',
              borderRadius: 8,
              background: 'rgba(239, 68, 68, 0.1)',
              color: '#ef4444',
              fontSize: 13,
            }}>
              本周免费额度已用完，请联系管理员购买额度。
            </div>
          )}
        </>
      ) : (
        <p style={{ fontSize: 13, color: 'var(--ol-ink-3)' }}>加载中...</p>
      )}
      <div style={{ marginTop: 16 }}>
        <button
          onClick={async () => {
            try { await authIpc.logout('') } catch {}
            reload()
          }}
          style={{
            padding: '4px 16px',
            borderRadius: 8,
            border: '1px solid var(--ol-border)',
            background: 'transparent',
            color: 'var(--ol-ink-3)',
            fontSize: 12,
            cursor: 'pointer',
          }}
        >
          登出
        </button>
      </div>
    </Card>
  )
}
