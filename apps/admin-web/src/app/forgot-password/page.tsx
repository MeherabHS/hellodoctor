'use client';

import React, { useState } from 'react';
import { BotChallenge } from '@/components/BotChallenge';

// Public Admin password reset: email -> 6-digit code -> new password.
// Every value is sent only in POST bodies to this origin's /api/password-reset/*
// routes and is never placed in the URL. React escapes all rendered text; no
// HTML from the server is ever injected. The reset authorization lives only in
// memory and is dropped as soon as it is used.
// Plain ASCII only, like the backend: no lookalike or invisible characters.
const EMAIL = /^[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9.-]{3,253}$/;
const RULES: [string, (p: string) => boolean][] = [
  ['At least 8 characters', (p) => p.length >= 8],
  ['An uppercase letter', (p) => /[A-Z]/.test(p)],
  ['A lowercase letter', (p) => /[a-z]/.test(p)],
  ['A digit', (p) => /[0-9]/.test(p)],
  ['A special character', (p) => /[^A-Za-z0-9]/.test(p)],
];

type Stage = 'email' | 'code' | 'password' | 'done';

async function post(path: string, body: Record<string, unknown>) {
  const res = await fetch(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body), cache: 'no-store', credentials: 'same-origin' });
  const json = await res.json().catch(() => null);
  return { ok: res.ok && json?.success === true, data: json?.data, error: typeof json?.error === 'string' ? json.error : 'Something went wrong. Please try again.' };
}

export default function ForgotPasswordPage() {
  const [stage, setStage] = useState<Stage>('email');
  const [email, setEmail] = useState('');
  const [code, setCode] = useState('');
  const [authorization, setAuthorization] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState('');
  const [devCode, setDevCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [challengeToken, setChallengeToken] = useState<string | null>(null);
  const [challengeReset, setChallengeReset] = useState(0);

  async function requestCode(e: React.FormEvent) {
    e.preventDefault();
    const value = email.trim();
    if (!EMAIL.test(value)) { setError('Enter a valid email address.'); return; }
    if (challengeToken === null || busy) return;
    setBusy(true); setError('');
    const result = await post('/api/password-reset/request', { email: value, turnstile_token: challengeToken });
    setChallengeToken(null); setChallengeReset((v) => v + 1);
    if (result.ok) {
      const dev = result.data?.debug_otp_code;
      setEmail(value); setCode('');
      setDevCode(typeof dev === 'string' && /^[0-9]{6}$/.test(dev) ? dev : '');
      setStage('code');
    } else setError(result.error);
    setBusy(false);
  }

  async function verifyCode(e: React.FormEvent) {
    e.preventDefault();
    if (!/^[0-9]{6}$/.test(code)) { setError('Enter the 6-digit code from the email.'); return; }
    if (busy) return;
    setBusy(true); setError('');
    const result = await post('/api/password-reset/verify', { email, otp_code: code });
    if (result.ok && typeof result.data?.reset_authorization === 'string') {
      setAuthorization(result.data.reset_authorization); setCode(''); setDevCode(''); setStage('password');
    } else setError(result.error);
    setBusy(false);
  }

  const rulesMet = RULES.every(([, test]) => test(password));
  async function savePassword(e: React.FormEvent) {
    e.preventDefault();
    if (!rulesMet || password !== confirm || busy) return;
    setBusy(true); setError('');
    const result = await post('/api/password-reset/complete', { reset_authorization: authorization, new_password: password });
    setAuthorization(''); setPassword(''); setConfirm('');
    if (result.ok) setStage('done');
    else { setError(result.error); setStage('email'); }
    setBusy(false);
  }

  function restart() { setStage('email'); setError(''); setCode(''); setDevCode(''); }

  return (
    <div className="afya-login">
      <section className="afya-login-brand" aria-hidden="true">
        <div className="afya-login-brand-top">
          {/* eslint-disable-next-line @next/next/no-img-element -- static same-origin asset */}
          <img src="/brand/afya_logo_white.png" alt="" />
          <div><div className="afya-login-brand-name">AfyaMkononi</div><div className="afya-login-brand-sub">Health for You</div></div>
        </div>
        <div><h2>Reset your password</h2><p>We will email a 6-digit code to your Admin address. The code works once and expires quickly.</p></div>
        <p className="afya-login-brand-foot">Kenya deployment · English and Kiswahili</p>
      </section>
      <main className="afya-login-main">
        <div className="afya-login-card">
          {stage === 'email' && (
            <>
              <div className="afya-login-head"><h1>Forgot password</h1><p>Enter the email address you use to sign in to the Admin console.</p></div>
              <form onSubmit={requestCode} className="afya-login-form" noValidate>
                {error && <div className="afya-login-error" role="alert">{error}</div>}
                <label htmlFor="fp-email">Email address</label>
                <input id="fp-email" type="email" autoComplete="username" required maxLength={254} value={email} onChange={(e) => setEmail(e.target.value)} />
                <BotChallenge action="admin_password_reset" reset={challengeReset} onToken={setChallengeToken} />
                <button type="submit" className="afya-btn-primary afya-login-submit" disabled={busy || challengeToken === null || !email.trim()}>{busy ? 'Sending…' : 'Send code'}</button>
              </form>
            </>
          )}
          {stage === 'code' && (
            <>
              <div className="afya-login-head"><h1>Check your email</h1><p>If an Admin account exists for <strong>{email}</strong>, we sent it a 6-digit code.</p></div>
              <form onSubmit={verifyCode} className="afya-login-form" noValidate>
                {error && <div className="afya-login-error" role="alert">{error}</div>}
                {devCode && <p className="afya-login-debug" role="status">Development only: code <span>{devCode}</span></p>}
                <label htmlFor="fp-code">6-digit code</label>
                <input id="fp-code" inputMode="numeric" autoComplete="one-time-code" required maxLength={6} value={code} onChange={(e) => setCode(e.target.value.replace(/[^0-9]/g, '').slice(0, 6))} />
                <button type="submit" className="afya-btn-primary afya-login-submit" disabled={busy || code.length !== 6}>{busy ? 'Checking…' : 'Continue'}</button>
                <button type="button" className="afya-btn-secondary" onClick={restart}>Use a different email</button>
              </form>
            </>
          )}
          {stage === 'password' && (
            <>
              <div className="afya-login-head"><h1>Choose a new password</h1><p>For <strong>{email}</strong>. You will be signed out everywhere else.</p></div>
              <form onSubmit={savePassword} className="afya-login-form" noValidate>
                {error && <div className="afya-login-error" role="alert">{error}</div>}
                <label htmlFor="fp-password">New password</label>
                <input id="fp-password" type="password" autoComplete="new-password" required maxLength={128} value={password} onChange={(e) => setPassword(e.target.value)} />
                <ul className="afya-check-list" style={{ margin: '4px 0 2px', gap: 4, fontSize: 12.5 }} aria-label="Password requirements">
                  {RULES.map(([label, test]) => <li key={label} style={{ color: test(password) ? 'var(--afya-success-fg)' : 'var(--afya-muted)' }}>{label}</li>)}
                </ul>
                <label htmlFor="fp-confirm">Confirm new password</label>
                <input id="fp-confirm" type="password" autoComplete="new-password" required maxLength={128} value={confirm} onChange={(e) => setConfirm(e.target.value)} />
                {confirm && password !== confirm && <span className="afya-stat-note" role="alert">Passwords do not match.</span>}
                <button type="submit" className="afya-btn-primary afya-login-submit" disabled={busy || !rulesMet || password !== confirm}>{busy ? 'Saving…' : 'Set new password'}</button>
              </form>
            </>
          )}
          {stage === 'done' && (
            <div className="afya-login-head">
              <h1>Password changed</h1>
              <p>Sign in with <strong>{email}</strong> and your new password. Every other session was signed out.</p>
              <a className="afya-btn-primary afya-login-submit" href="/login" style={{ textDecoration: 'none' }}>Go to sign in</a>
            </div>
          )}
          {stage !== 'done' && <p className="afya-login-note"><a href="/login">Back to sign in</a> · Codes are single use. All Admin actions are audited.</p>}
        </div>
      </main>
    </div>
  );
}
