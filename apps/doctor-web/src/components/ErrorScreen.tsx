'use client';
import { Icon, type IconName } from './Icons';

/** Shared calm error screen for 404 / 5xx states. Never shows stack traces or request data. */
export function ErrorScreen({ code, title, message, icon, actions }: { code: string; title: string; message: string; icon: IconName; actions: React.ReactNode }) {
  return <main style={{ minHeight: '100dvh', display: 'grid', placeItems: 'center', padding: 24 }}>
    <div className="am-card" style={{ width: '100%', maxWidth: 520, textAlign: 'center' }}>
      <div className="am-card-body am-stack" style={{ alignItems: 'center', gap: 16, padding: '40px 32px' }}>
        <div className="am-row" style={{ gap: 10, justifyContent: 'center' }}><img src="/brand/afya_logo.png" alt="" width={32} height={32}/><strong className="am-strong">AfyaMkononi Doctor Portal</strong></div>
        <span className="am-stat-icon" style={{ width: 72, height: 72, borderRadius: 22 }}><Icon name={icon} size={34}/></span>
        <span className="am-badge green" style={{ fontSize: 13 }}>Error {code}</span>
        <h1 style={{ fontSize: 26 }}>{title}</h1>
        <p className="am-muted" style={{ maxWidth: 400 }}>{message}</p>
        <div className="am-row" style={{ justifyContent: 'center', marginTop: 6 }}>{actions}</div>
      </div>
    </div>
  </main>;
}
