// VoiceprintSection — ZeroType speaker management in settings.
import { useEffect, useState } from 'react'
import { Card } from '../_atoms'
import { SettingRow, SectionTitle } from './shared'
import * as voiceprintIpc from '../../lib/ipc/voiceprint'
import type { SpeakerProfile } from '../../lib/ipc/voiceprint'
import { Icon } from '../../components/Icon'

export function VoiceprintSection() {
  const [profiles, setProfiles] = useState<SpeakerProfile[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    voiceprintIpc.voiceprintList()
      .then(setProfiles)
      .finally(() => setLoading(false))
  }, [])

  const handleRemove = async (id: string) => {
    await voiceprintIpc.voiceprintRemove(id)
    setProfiles(prev => prev.filter(p => p.id !== id))
  }

  return (
    <Card>
      <SectionTitle>声纹管理</SectionTitle>
      <p style={{ fontSize: 12, color: 'var(--ol-ink-3)', margin: '0 0 16px' }}>
        已注册的说话人声纹。录音时自动识别并标注说话人。
      </p>

      {loading ? (
        <p style={{ fontSize: 13, color: 'var(--ol-ink-3)' }}>加载中...</p>
      ) : profiles.length === 0 ? (
        <div style={{
          padding: '24px 0',
          textAlign: 'center',
          color: 'var(--ol-ink-3)',
          fontSize: 13,
        }}>
          <div style={{ fontSize: 32, marginBottom: 8, opacity: 0.3 }}>🎤</div>
          暂无注册的说话人<br />
          <span style={{ fontSize: 12, opacity: 0.6 }}>
            请在其他设置中使用"注册声纹"功能添加
          </span>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          {profiles.map(p => (
            <div
              key={p.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '8px 12px',
                borderRadius: 8,
                background: 'var(--ol-bg-2)',
              }}
            >
              <div>
                <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--ol-ink-1)' }}>
                  {p.name}
                </div>
                <div style={{ fontSize: 11, color: 'var(--ol-ink-3)', marginTop: 2 }}>
                  匹配 {p.matchCount} 次 · {new Date(p.createdAt).toLocaleDateString()}
                </div>
              </div>
              <button
                onClick={() => handleRemove(p.id)}
                title="删除声纹"
                style={{
                  border: 'none',
                  background: 'transparent',
                  color: 'var(--ol-ink-3)',
                  cursor: 'pointer',
                  padding: 4,
                  borderRadius: 4,
                }}
              >
                <Icon name="trash" size={14} />
              </button>
            </div>
          ))}
        </div>
      )}
    </Card>
  )
}
