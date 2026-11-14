'use client';

import React, { useEffect, useRef, useState } from 'react';
import { BotChallenge } from '@/components/BotChallenge';

// Public page for accepting an Admin invitation. The token arrives in the
// emailed link (?token=…). It is read once, removed from the address bar and
// history straight away, kept only in memory, and sent only in POST bodies to
// this origin's /api/invite/* routes. React escapes every rendered value.
const TOKEN_PATTERN = /^[0-9a-f]{64}$/i;
const RULES: [string, (p: string) => boolean][] = [
  ['At least 8 characters', (p) => p.length >= 8],
  ['An uppercase letter', (p) => /[A-Z]/.test(p)],
  ['A lowercase letter', (p) => /[a-z]/.test(p)],
  ['A digit', (p) => /[0-9]/.test(p)],
  ['A special character', (p) => /[^A-Za-z0-9]/.test(p)],
];

type Stage = 'checking' | 'invalid' | 'form' | 'done';

async function post(path: string, body: Record<string, unknown>) {
  const res = await fetch(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body), cache: 'no-store', credentials: 'same-origin' });
  const json = await res.json().catch(() => null);
  return { ok: res.ok && json?.success === true, data: json?.data, error: typeof json?.error === 'string' ? json.error : 'Something went wrong. Please try again.' };
}

export default function AcceptInvitationPage() {
  const [stage, setStage] = useState<Stage>('checking');
  const [token, setToken] = useState('');
  const [email, setEmail] = useState('');
  const [phone, setPhone] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [challengeToken, setChallengeToken] = useState<string | null>(null);
  const [challengeReset, setChallengeReset] = useState(0);
  // Read the token exactly once: the effect below can run twice (React
  // StrictMode), and the address bar is already cleaned on the second run.
  const tokenRef = useRef<string | null>(null);

  useEffect(() => {
    if (tokenRef.current === null) {
      tokenRef.current = new URLSearchParams(window.location.search).get('token') ?? '';
      // Drop the secret from the address bar and history immediately.
      window.history.replaceState(null, '', '/invite');
    }
    const raw = tokenRef.current;
    let active = true;
    (async () => {
      if (!TOKEN_PATTERN.test(raw)) { if (active) setStage('invalid'); return; }
      const result = await post('/api/invite/lookup', { token: raw });
      if (!active) return;
      if (result.ok && typeof result.data?.email === 'string') { setToken(raw); setEmail(result.data.email); setStage('form'); }
      else { setError(result.error); setStage('invalid'); }
    })();
    return () => { active = false; };
  }, []);

  const rulesMet = RULES.every(([, test]) => test(password));
  const canSubmit = !submitting && challengeToken !== null && rulesMet && password === confirm && phone.trim().length >= 9;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setSubmitting(true); setError('');
    const result = await post('/api/invite/accept', { token, phone_number: phone.trim(), password, turnstile_token: challengeToken });
    setChallengeToken(null); setChallengeReset((v) => v + 1);
    if (result.ok) { setPassword(''); setConfirm(''); setToken(''); setStage('done'); }
    else setError(result.error);
    setSubmitting(false);
  }

  return (
    <div className="afya-login">
      <section className="afya-login-brand" aria-hidden="true">
        <div className="afya-login-brand-top">
          {/* eslint-disable-next-line @next/next/no-img-element -- static same-origin asset */}
          <img src="/brand/afya_logo_white.png" alt="" />
          <div><div className="afya-login-brand-name">AfyaMkononi</div><div className="afya-login-brand-sub">Health for You</div></div>
        </div>
        <div><h2>Join the Admin team</h2><p>Set up your account to manage content, people and care records, operations and campaigns.</p></div>
        <p className="afya-login-brand-foot">Kenya deployment · English and Kiswahili</p>
      </section>
      <main className="afya-login-main">
        <div className="afya-login-card">
          {stage === 'checking' && <div className="afya-login-head"><h1>Checking your invitation…</h1><p>One moment.</p></div>}
          {stage === 'invalid' && (
            <div className="afya-login-head">
              <h1>Invitation not valid</h1>
              <p>{error || 'This invitation link is invalid or has expired.'} Ask an Admin to send you a new invitation.</p>
              <a className="afya-btn-secondary" href="/login" style={{ textDecoration: 'none' }}>Go to sign in</a>
            </div>
          )}
          {stage === 'done' && (
            <div className="afya-login-head">
              <h1>Your Admin account is ready</h1>
              <p>Sign in with <strong>{email}</strong> and your new password. We will email you a verification code.</p>
              <a className="afya-btn-primary afya-login-submit" href="/login" style={{ textDecoration: 'none' }}>Go to sign in</a>
            </div>
          )}
          {stage === 'form' && (
            <>
              <div className="afya-login-head">
                <h1>Accept your invitation</h1>
                <p>You were invited as an Admin with <strong>{email}</strong>. Set your phone number and password.</p>
              </div>
              <form onSubmit={submit} className="afya-login-form">
                {error && <div className="afya-login-error" role="alert">{error}</div>}
                <label htmlFor="inv-phone">Phone number (Kenya)</label>
                <input id="inv-phone" type="tel" inputMode="tel" autoComplete="tel" required maxLength={20} placeholder="0712 345 678" value={phone} onChange={(e) => setPhone(e.target.value)} />
                <label htmlFor="inv-password">Password</label>
                <input id="inv-password" type="password" autoComplete="new-password" required maxLength={128} value={password} onChange={(e) => setPassword(e.target.value)} />
                <ul className="afya-check-list" style={{ margin: '4px 0 2px', gap: 4, fontSize: 12.5 }} aria-label="Password requirements">
                  {RULES.map(([label, test]) => <li key={label} style={{ color: test(password) ? 'var(--afya-success-fg)' : 'var(--afya-muted)' }}>{label}</li>)}
                </ul>
                <label htmlFor="inv-confirm">Confirm password</label>
                <input id="inv-confirm" type="password" autoComplete="new-password" required maxLength={128} value={confirm} onChange={(e) => setConfirm(e.target.value)} />
                {confirm && password !== confirm && <span className="afya-stat-note" role="alert">Passwords do not match.</span>}
                <BotChallenge action="admin_invite" reset={challengeReset} onToken={setChallengeToken} />
                <button type="submit" className="afya-btn-primary afya-login-submit" disabled={!canSubmit}>{submitting ? 'Creating account…' : 'Create Admin account'}</button>
              </form>
            </>
          )}
          <p className="afya-login-note">Invitations are single use and expire 5 minutes after they are sent. All Admin actions are audited.</p>
        </div>
      </main>
    </div>
  );
}
