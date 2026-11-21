'use client';
import { useEffect, useRef, useState } from 'react';
import { useT } from '@/lib/i18n';
import { answerStepUp, markStepUpExpired, subscribeStepUp, subscribeStepUpExpiry, type StepUpPrompt } from '@/lib/stepUp';

/** Password re-entry for sensitive Admin actions (see lib/stepUp.ts). */
export function StepUpDialog() {
  const { t } = useT();
  const [prompt, setPrompt] = useState<StepUpPrompt | null>(null);
  const [password, setPassword] = useState('');
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => subscribeStepUp((next) => {
    setPrompt(next);
    if (!next || next.error) setPassword('');
  }), []);
  useEffect(() => {
    if (prompt && !prompt.busy) input.current?.focus();
  }, [prompt]);

  if (!prompt) return null;
  const cancel = () => {
    if (!prompt.busy) answerStepUp(null);
  };
  return (
    <div className="admin-modal-backdrop" style={{ zIndex: 10001 }} onClick={(e) => { if (e.target === e.currentTarget) cancel(); }}>
      <section role="dialog" aria-modal="true" aria-labelledby="step-up-title" aria-describedby="step-up-body" className="admin-modal-card" style={{ maxWidth: 440, padding: 24 }}>
        <h2 id="step-up-title" className="afya-modal-title">{t('stepUp.title')}</h2>
        <p id="step-up-body" className="afya-modal-sub" style={{ marginTop: 8 }}>{t('stepUp.body')}</p>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (password && !prompt.busy) answerStepUp(password);
          }}
          onKeyDown={(e) => { if (e.key === 'Escape') cancel(); }}
          style={{ marginTop: 16 }}
        >
          <label htmlFor="step-up-password" className="afya-field-label">{t('stepUp.password')}</label>
          <input
            ref={input}
            id="step-up-password"
            className="afya-input"
            type="password"
            autoComplete="current-password"
            maxLength={128}
            value={password}
            disabled={prompt.busy}
            onChange={(e) => setPassword(e.target.value)}
          />
          {prompt.error && <p role="alert" className="afya-login-error" style={{ marginTop: 10 }}>{prompt.error}</p>}
          <div className="afya-guard-actions" style={{ marginTop: 18 }}>
            <button type="button" className="afya-btn-secondary" disabled={prompt.busy} onClick={cancel}>{t('stepUp.cancel')}</button>
            <button type="submit" className="afya-btn-primary" disabled={prompt.busy || !password}>
              {prompt.busy ? t('stepUp.checking') : t('stepUp.confirm')}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

/** Time left on the last password confirmation (2 minutes), bottom right. */
export function StepUpTimer() {
  const { t } = useT();
  const [expiresAt, setExpiresAt] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => subscribeStepUpExpiry((next) => {
    setExpiresAt(next);
    setNow(Date.now());
  }), []);
  useEffect(() => {
    if (expiresAt === null) return;
    const timer = setInterval(() => {
      const current = Date.now();
      setNow(current);
      if (current >= expiresAt) markStepUpExpired();
    }, 1000);
    return () => clearInterval(timer);
  }, [expiresAt]);
  if (expiresAt === null) return null;
  const left = Math.max(0, Math.ceil((expiresAt - now) / 1000));
  if (left === 0) return null;
  const time = `${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}`;
  return (
    <div className="afya-step-up-timer" title={t('stepUp.unlockedHint')} aria-live="off">
      <span aria-hidden className="afya-step-up-dot" />
      {t('stepUp.unlocked', { time })}
    </div>
  );
}
