// LoginDialog — ZeroType login/register modal.
// Shown on app launch if no valid session exists.

import { useState } from 'react'
import { Modal } from './ui/Modal'
import * as authIpc from '../lib/ipc/auth'
import type { AuthUser } from '../lib/ipc/auth'

type Mode = 'login' | 'register'

interface LoginDialogProps {
  onLogin: (user: AuthUser) => void
}

export function LoginDialog({ onLogin }: LoginDialogProps) {
  const [mode, setMode] = useState<Mode>('login')
  const [credential, setCredential] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const handleSubmit = async () => {
    setError('')
    if (!credential.trim() || !password) {
      setError('请填写所有字段')
      return
    }
    setLoading(true)
    try {
      if (mode === 'login') {
        const user = await authIpc.login({ credential: credential.trim(), password })
        onLogin(user)
      } else {
        const isEmail = credential.includes('@')
        const user = await authIpc.register({
          [isEmail ? 'email' : 'phone']: credential.trim(),
          password,
        })
        onLogin(user)
      }
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  return (
    <Modal onClose={() => {}} zIndex={100}>
      <div style={{ padding: '32px 28px 24px' }}>
        <h2 style={{ margin: '0 0 4px', fontSize: 20, fontWeight: 600, color: 'var(--ol-ink-1)' }}>
          {mode === 'login' ? '登录 ZeroType' : '注册 ZeroType'}
        </h2>
        <p style={{ margin: '0 0 24px', fontSize: 13, color: 'var(--ol-ink-3)' }}>
          使用邮箱或手机号{mode === 'register' ? '注册' : '登录'}
        </p>

        {error && (
          <div style={{
            padding: '8px 12px',
            marginBottom: 16,
            borderRadius: 6,
            background: 'rgba(239, 68, 68, 0.1)',
            color: '#ef4444',
            fontSize: 13,
          }}>
            {error}
          </div>
        )}

        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <input
            type="text"
            placeholder="邮箱 或 手机号"
            value={credential}
            onChange={e => setCredential(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleSubmit()}
            style={{
              padding: '10px 14px',
              borderRadius: 8,
              border: '1px solid var(--ol-line-2)',
              background: 'var(--ol-bg-2)',
              color: 'var(--ol-ink-1)',
              fontSize: 14,
              outline: 'none',
            }}
          />
          <input
            type="password"
            placeholder="密码（至少6位）"
            value={password}
            onChange={e => setPassword(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleSubmit()}
            style={{
              padding: '10px 14px',
              borderRadius: 8,
              border: '1px solid var(--ol-line-2)',
              background: 'var(--ol-bg-2)',
              color: 'var(--ol-ink-1)',
              fontSize: 14,
              outline: 'none',
            }}
          />
          <button
            onClick={handleSubmit}
            disabled={loading}
            style={{
              padding: '10px 0',
              borderRadius: 8,
              border: 'none',
              background: loading ? 'var(--ol-bg-3)' : 'var(--ol-accent)',
              color: loading ? 'var(--ol-ink-3)' : '#fff',
              fontSize: 15,
              fontWeight: 500,
              cursor: loading ? 'not-allowed' : 'pointer',
            }}
          >
            {loading ? '处理中...' : mode === 'login' ? '登录' : '注册'}
          </button>
        </div>

        <div style={{ marginTop: 16, textAlign: 'center', fontSize: 13, color: 'var(--ol-ink-3)' }}>
          {mode === 'login' ? (
            <>还没有账号？<a onClick={() => { setMode('register'); setError('') }} style={{ color: 'var(--ol-accent)', cursor: 'pointer' }}>注册</a></>
          ) : (
            <>已有账号？<a onClick={() => { setMode('login'); setError('') }} style={{ color: 'var(--ol-accent)', cursor: 'pointer' }}>登录</a></>
          )}
        </div>
      </div>
    </Modal>
  )
}
