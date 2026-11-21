'use client';

import { tr, LanguageSwitch, useT } from '@/lib/i18n';
import { authErrorMessage } from '@/lib/i18n/apiErrors';
import { clearAdminRuntime } from '@/lib/api';
import React, { useState } from 'react';
import { useRouter } from 'next/navigation';
import { BotChallenge } from '@/components/BotChallenge';

// Deployed Admin login requires a browser challenge and password, followed by
// an emailed one-time verification code. Admin, Doctor and Patient share this
// same single-factor-OTP MFA shape; there is no authenticator-app factor.
// Tokens stay in the BFF.
export default function AdminLoginPage() {
  useT(); // re-render this screen when the language changes
  const router = useRouter();
  const [step, setStep] = useState<'credentials' | 'code'>('credentials');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [otpCode, setOtpCode] = useState('');
  const [sessionToken, setSessionToken] = useState('');
  const [debugCode, setDebugCode] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [challengeToken, setChallengeToken] = useState<string | null>(null);
  const [challengeReset, setChallengeReset] = useState(0);

  async function handleCredentialsSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (submitting || challengeToken === null) return;
    setError(null);
    setSubmitting(true);
    try {
      const res = await fetch('/api/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email, password, turnstile_token: challengeToken }),
      });
      const json = await res.json();
      if (!res.ok || !json.success) {
        setError(authErrorMessage(res.status, json.error, tr('ui2.invalidCredentials')));
        setSubmitting(false);
        return;
      }
      setSessionToken(json.data.session_token);
      setDebugCode(json.data.debug_otp_code ?? null);
      setOtpCode('');
      setStep('code');
      setSubmitting(false);
    } catch {
      setError(tr('ui.couldNotReachTheServerPlease'));
      setSubmitting(false);
    } finally {
      setChallengeToken(null);
      setChallengeReset(value => value + 1);
    }
  }

  async function handleCodeSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (submitting) return;
    setError(null);
    setSubmitting(true);
    try {
      const res = await fetch('/api/login/verify', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_token: sessionToken, otp_code: otpCode }),
      });
      const json = await res.json();
      if (!res.ok || !json.success) {
        setError(authErrorMessage(res.status, json.error, tr('ui2.invalidOrExpiredVerificationCode')));
        setSubmitting(false);
        return;
      }
      clearAdminRuntime();
      router.push('/');
      router.refresh();
    } catch {
      setError(tr('ui.couldNotReachTheServerPlease'));
      setSubmitting(false);
    }
  }

  function backToCredentials() {
    setStep('credentials');
    setError(null);
    setOtpCode('');
    setSessionToken('');
    setDebugCode(null);
  }

  return (
    <div className="afya-login">
      <section className="afya-login-brand" aria-hidden="true">
        <div className="afya-login-brand-top">
          {/* eslint-disable-next-line @next/next/no-img-element -- static same-origin asset */}
          <img src="/brand/afya_logo_white.png" alt="" />
          <div>
            <div className="afya-login-brand-name">AfyaMkononi</div>
            <div className="afya-login-brand-sub">{tr('ui.healthForYou')}</div>
          </div>
        </div>
        <div>
          <h2>{tr('ui.adminConsole')}</h2>
          <p>{tr('ui.contentPeopleAndCareRecordsOperations')}</p>
        </div>
        <p className="afya-login-brand-foot">{tr('ui.kenyaDeploymentEnglishAndKiswahili')}</p>
      </section>

      <main className="afya-login-main">
        <LanguageSwitch className="afya-login-lang" />
        <div className="afya-login-card">
          <div className="afya-login-head">
            {/* eslint-disable-next-line @next/next/no-img-element -- static same-origin asset */}
            <img src="/brand/afya_logo.png" alt="AfyaMkononi" className="afya-login-mobile-logo" />
            <h1>{step === 'credentials' ? tr('ui2.signInToAdmin') : tr('ui2.checkYourEmail')}</h1>
            <p>{step === 'credentials' ? tr('ui2.useYourAdministratorEmailAndPassword') : <>{tr('ui.weEmailedAVerificationCodeTo')} <strong>{email}</strong>.</>}</p>
          </div>

          {step === 'credentials' ? (
            <form onSubmit={handleCredentialsSubmit} className="afya-login-form">
              {error && <div className="afya-login-error" role="alert">{error}</div>}

              <label htmlFor="email">{tr('ui.adminEmail')}</label>
              <input
                id="email"
                type="email"
                required
                autoComplete="username"
                autoFocus
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />

              <label htmlFor="password">{tr('ui.password')}</label>
              <input
                id="password"
                type="password"
                required
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />

              <a href="/forgot-password" className="afya-login-forgot">{tr('ui.forgotPassword2')}</a>
              <BotChallenge action="admin_login" reset={challengeReset} onToken={setChallengeToken} />
              <button type="submit" className="afya-btn-primary afya-login-submit" disabled={submitting || challengeToken === null}>
                {submitting ? tr('ui2.checking') : tr('ui2.continue')}
              </button>
            </form>
          ) : (
            <form onSubmit={handleCodeSubmit} className="afya-login-form">
              {error && <div className="afya-login-error" role="alert">{error}</div>}
              {debugCode && (
                <p className="afya-login-debug">
                  Development mode — email delivery bypassed. Code: <span>{debugCode}</span>
                </p>
              )}

              <label htmlFor="otp">{tr('ui.verificationCode')}</label>
              <input
                id="otp"
                type="text"
                inputMode="numeric"
                pattern="[0-9]*"
                maxLength={6}
                required
                autoComplete="one-time-code"
                autoFocus
                value={otpCode}
                onChange={(e) => setOtpCode(e.target.value.replace(/\D/g, ''))}
                placeholder={tr('ui.n6DigitCode')}
                className="afya-login-otp"
              />

              <button type="submit" className="afya-btn-primary afya-login-submit" disabled={submitting}>
                {submitting ? tr('ui2.verifying') : tr('ui2.signIn')}
              </button>
              <button type="button" className="afya-login-back" onClick={backToCredentials} disabled={submitting}>
                {tr('ui.useADifferentAccount')}
              </button>
            </form>
          )}

          <p className="afya-login-note">{tr('ui.accessIsRestrictedToVerifiedPlatform')}</p>
        </div>
      </main>
    </div>
  );
}