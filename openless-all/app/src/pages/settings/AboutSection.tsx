// 关于 → 版本信息 / 检查更新 / 字体大小 / 文档链接。
// 「个性化」原本是独立 tab，但只剩字体大小一项、整页太空，遂并入「关于」。
// 「加入 Beta 渠道」已挪到「高级」页底部（见 BetaChannelSection），这里图标旁
// 只保留查正式版的「检查更新」按钮。

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Row } from '../../components/ui/Row';
import { getPlatformCapabilities, openExternal } from '../../lib/ipc';
import type { PlatformCapabilities } from '../../lib/types';
import { APP_VERSION_LABEL } from '../../lib/appVersion';
import { Card } from '../_atoms';
import { btnGhostStyle, SectionTitle } from './shared';
import { CheckUpdateButton } from './CheckUpdateButton';

const GITHUB_URL = 'https://github.com/finelab/zerotype';
const HELP_URL = `${GITHUB_URL}#readme`;
const RELEASE_NOTES_URL = `${GITHUB_URL}/releases`;

export function AboutSection() {
  const { t } = useTranslation();
  const [platformCaps, setPlatformCaps] = useState<PlatformCapabilities | null>(null);

  useEffect(() => {
    void getPlatformCapabilities().then(setPlatformCaps);
  }, []);

  return (
    <>
      {/* ─── 版本信息 + 检查更新（正式版）─────────────────────────────── */}
      <Card>
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <img
            src="AppIcon.png"
            alt=""
            style={{ width: 56, height: 56, borderRadius: 13, boxShadow: '0 4px 10px rgba(0,0,0,.10), 0 0 0 0.5px rgba(0,0,0,.06)' }}
          />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 17, fontWeight: 600 }}>ZeroType</div>
            <div style={{ fontSize: 12, color: 'var(--ol-ink-3)', marginTop: 2 }}>
              {t('modal.about.tagline')} · {APP_VERSION_LABEL}
            </div>
          </div>
          {/* 图标右上方：查正式版的检查更新按钮。Beta 渠道在「高级」页。 */}
          {platformCaps?.supportsAutoUpdate === true && (
            <CheckUpdateButton channel="stable" />
          )}
        </div>
      </Card>

      {/* 个性化（字体大小）已按需求移除（页面瘦身）。 */}

      {/* ─── 文档链接 ─────────────────────────────────────────────── */}
      <Card>
        <SectionTitle>{t('settings.about.linksTitle')}</SectionTitle>
        <Row label={t('modal.about.source')}>
          <button style={btnGhostStyle} onClick={() => openExternal(GITHUB_URL)}>
            GitHub
          </button>
        </Row>
        <Row label={t('modal.about.docs')}>
          <button style={btnGhostStyle} onClick={() => openExternal(HELP_URL)}>
            {t('modal.about.docsBtn')}
          </button>
        </Row>
        <Row label={t('modal.about.feedback')}>
          <button style={btnGhostStyle} onClick={() => openExternal(`${GITHUB_URL}/issues`)}>
            {t('modal.about.feedbackBtn')}
          </button>
        </Row>
      </Card>
    </>
  );
}
