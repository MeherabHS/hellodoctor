'use client';

import { tr, LanguageSwitch, useT } from '@/lib/i18n';
import { authErrorMessage } from '@/lib/i18n/apiErrors';
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
  return { ok: res.ok && json?.success === true, data: json?.data, error: authErrorMessage(res.status, json?.error, tr('ui2.somethingWentWrongPleaseTryAgain')) };
}

export default function ForgotPasswordPage() {
  useT(); // re-render this screen when the language changes
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
    if (!EMAIL.test(value)) { setError(tr('ui.enterAValidEmailAddress')); return; }
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
    if (!/^[0-9]{6}$/.test(code)) { setError(tr('ui.enterThe6DigitCodeFrom')); return; }
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
          <div><div className="afya-login-brand-name">AfyaMkononi</div><div className="afya-login-brand-sub">{tr('ui.healthForYou')}</div></div>
        </div>
        <div><h2>{tr('ui.resetYourPassword')}</h2><p>{tr('ui.weWillEmailA6Digit')}</p></div>
        <p className="afya-login-brand-foot">{tr('ui.kenyaDeploymentEnglishAndKiswahili')}</p>
      </section>
      <main className="afya-login-main">
        <LanguageSwitch className="afya-login-lang" />
        <div className="afya-login-card">
          {stage === 'email' && (
            <>
              <div className="afya-login-head"><h1>{tr('ui.forgotPassword')}</h1><p>{tr('ui.enterTheEmailAddressYouUse')}</p></div>
              <form onSubmit={requestCode} className="afya-login-form" noValidate>
                {error && <div className="afya-login-error" role="alert">{error}</div>}
                <label htmlFor="fp-email">{tr('ui.emailAddress')}</label>
                <input id="fp-email" type="email" autoComplete="username" required maxLength={254} value={email} onChange={(e) => setEmail(e.target.value)} />
                <BotChallenge action="admin_password_reset" reset={challengeReset} onToken={setChallengeToken} />
                <button type="submit" className="afya-btn-primary afya-login-submit" disabled={busy || challengeToken === null || !email.trim()}>{busy ? tr('ui2.sending') : tr('ui2.sendCode')}</button>
              </form>
            </>
          )}
          {stage === 'code' && (
            <>
              <div className="afya-login-head"><h1>{tr('ui.checkYourEmail')}</h1><p>{tr('ui.ifAnAdminAccountExistsFor')} <strong>{email}</strong>{tr('ui.weSentItA6Digit')}</p></div>
              <form onSubmit={verifyCode} className="afya-login-form" noValidate>
                {error && <div className="afya-login-error" role="alert">{error}</div>}
                {devCode && <p className="afya-login-debug" role="status">{tr('ui.developmentOnlyCode')} <span>{devCode}</span></p>}
                <label htmlFor="fp-code">{tr('ui.n6DigitCode')}</label>
                <input id="fp-code" inputMode="numeric" autoComplete="one-time-code" required maxLength={6} value={code} onChange={(e) => setCode(e.target.value.replace(/[^0-9]/g, '').slice(0, 6))} />
                <button type="submit" className="afya-btn-primary afya-login-submit" disabled={busy || code.length !== 6}>{busy ? tr('ui2.checking') : tr('ui2.continue')}</button>
                <button type="button" className="afya-btn-secondary" onClick={restart}>{tr('ui.useADifferentEmail')}</button>
              </form>
            </>
          )}
          {stage === 'password' && (
            <>
              <div className="afya-login-head"><h1>{tr('ui.chooseANewPassword')}</h1><p>{tr('ui.for')} <strong>{email}</strong>{tr('ui.youWillBeSignedOutEverywhere')}</p></div>
              <form onSubmit={savePassword} className="afya-login-form" noValidate>
                {error && <div className="afya-login-error" role="alert">{error}</div>}
                <label htmlFor="fp-password">{tr('ui.newPassword')}</label>
                <input id="fp-password" type="password" autoComplete="new-password" required maxLength={128} value={password} onChange={(e) => setPassword(e.target.value)} />
                <ul className="afya-check-list" style={{ margin: '4px 0 2px', gap: 4, fontSize: 12.5 }} aria-label={tr('ui.passwordRequirements')}>
                  {RULES.map(([label, test]) => <li key={label} style={{ color: test(password) ? 'var(--afya-success-fg)' : 'var(--afya-muted)' }}>{label}</li>)}
                </ul>
                <label htmlFor="fp-confirm">{tr('ui.confirmNewPassword')}</label>
                <input id="fp-confirm" type="password" autoComplete="new-password" required maxLength={128} value={confirm} onChange={(e) => setConfirm(e.target.value)} />
                {confirm && password !== confirm && <span className="afya-stat-note" role="alert">{tr('ui.passwordsDoNotMatch')}</span>}
                <button type="submit" className="afya-btn-primary afya-login-submit" disabled={busy || !rulesMet || password !== confirm}>{busy ? tr('ui2.saving') : tr('ui2.setNewPassword')}</button>
              </form>
            </>
          )}
          {stage === 'done' && (
            <div className="afya-login-head">
              <h1>{tr('ui.passwordChanged')}</h1>
              <p>{tr('ui.signInWith')} <strong>{email}</strong> {tr('ui.andYourNewPasswordEveryOther')}</p>
              <a className="afya-btn-primary afya-login-submit" href="/login" style={{ textDecoration: 'none' }}>{tr('ui.goToSignIn')}</a>
            </div>
          )}
          {stage !== 'done' && <p className="afya-login-note"><a href="/login">{tr('ui.backToSignIn')}</a> {tr('ui.codesAreSingleUseAllAdmin')}</p>}
        </div>
      </main>
    </div>
  );
}
