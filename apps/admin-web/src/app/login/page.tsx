'use client';

import { clearAdminRuntime } from '@/lib/api';
import React, { useState } from 'react';
import { useRouter } from 'next/navigation';
import { BotChallenge } from '@/components/BotChallenge';

// Deployed Admin login requires a browser challenge and password, followed by
// emailed OTP plus the enrolled authenticator code. Tokens stay in the BFF.
export default function AdminLoginPage() {
  const router = useRouter();
  const [step, setStep] = useState<'credentials' | 'code'>('credentials');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [otpCode, setOtpCode] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [requiresTotp, setRequiresTotp] = useState(true);
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
        setError(json.error || 'Invalid credentials.');
        setSubmitting(false);
        return;
      }
      setSessionToken(json.data.session_token);
      setDebugCode(json.data.debug_otp_code ?? null);
      setOtpCode('');
      setTotpCode('');
      setRequiresTotp(json.data.requires_totp !== false);
      setStep('code');
      setSubmitting(false);
    } catch {
      setError('Could not reach the server. Please try again.');
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
        body: JSON.stringify({ session_token: sessionToken, otp_code: otpCode, ...(totpCode ? { totp_code: totpCode } : {}) }),
      });
      const json = await res.json();
      if (!res.ok || !json.success) {
        setError(json.error || 'Invalid or expired verification code.');
        setSubmitting(false);
        return;
      }
      clearAdminRuntime();
      router.push('/');
      router.refresh();
    } catch {
      setError('Could not reach the server. Please try again.');
      setSubmitting(false);
    }
  }

  function backToCredentials() {
    setStep('credentials');
    setError(null);
    setOtpCode('');
    setTotpCode('');
    setSessionToken('');
    setDebugCode(null);
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-slate-50 px-4">
      <div className="w-full max-w-sm">
        <div className="text-center mb-8">
          <div className="inline-flex h-12 w-12 items-center justify-center rounded-xl bg-emerald-600 text-white font-bold text-lg mb-3">
            HD
          </div>
          <h1 className="text-xl font-semibold text-slate-900">AfyaMkononi Admin Portal</h1>
          <p className="text-sm text-slate-500 mt-1">Central Hospital &amp; Operations Command</p>
        </div>

        {step === 'credentials' ? (
          <form onSubmit={handleCredentialsSubmit} className="bg-white border border-slate-200 rounded-2xl shadow-sm p-6 space-y-4">
            {error && (
              <div className="rounded-lg bg-red-50 border border-red-200 text-red-700 text-sm px-3 py-2">{error}</div>
            )}

            <div>
              <label htmlFor="email" className="block text-sm font-medium text-slate-700 mb-1">
                Admin email
              </label>
              <input
                id="email"
                type="email"
                required
                autoComplete="username"
                autoFocus
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500"
              />
            </div>

            <div>
              <label htmlFor="password" className="block text-sm font-medium text-slate-700 mb-1">
                Password
              </label>
              <input
                id="password"
                type="password"
                required
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-emerald-500"
              />
            </div>

            <BotChallenge action="admin_login" reset={challengeReset} onToken={setChallengeToken} />
            <button
              type="submit"
              disabled={submitting || challengeToken === null}
              className="w-full rounded-lg bg-emerald-600 text-white font-medium py-2.5 text-sm hover:bg-emerald-700 disabled:opacity-60 transition-colors"
            >
              {submitting ? 'Checking…' : 'Continue'}
            </button>
          </form>
        ) : (
          <form onSubmit={handleCodeSubmit} className="bg-white border border-slate-200 rounded-2xl shadow-sm p-6 space-y-4">
            {error && (
              <div className="rounded-lg bg-red-50 border border-red-200 text-red-700 text-sm px-3 py-2">{error}</div>
            )}

            <div>
              <p className="text-sm text-slate-700">
                We emailed a verification code to <span className="font-medium">{email}</span>. Enter it below to finish signing in.
              </p>
              {debugCode && (
                <p className="mt-2 rounded-lg bg-amber-50 border border-amber-200 text-amber-800 text-xs px-3 py-2">
                  Development mode — email delivery bypassed. Code: <span className="font-mono font-semibold">{debugCode}</span>
                </p>
              )}
            </div>

            <div>
              <label htmlFor="otp" className="block text-sm font-medium text-slate-700 mb-1">
                Verification code
              </label>
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
                placeholder="6-digit code"
                className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm tracking-widest focus:outline-none focus:ring-2 focus:ring-emerald-500"
              />
            </div>

            {requiresTotp && <div>
              <label htmlFor="totp" className="block text-sm font-medium text-slate-700 mb-1">Authenticator code</label>
              <input id="totp" type="text" inputMode="numeric" pattern="[0-9]{6}" maxLength={6} required
                autoComplete="off" value={totpCode} onChange={event => setTotpCode(event.target.value.replace(/\D/g, ''))}
                placeholder="6-digit authenticator code"
                className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm tracking-widest focus:outline-none focus:ring-2 focus:ring-emerald-500" />
            </div>}
            <button
              type="submit"
              disabled={submitting}
              className="w-full rounded-lg bg-emerald-600 text-white font-medium py-2.5 text-sm hover:bg-emerald-700 disabled:opacity-60 transition-colors"
            >
              {submitting ? 'Verifying…' : 'Sign in'}
            </button>

            <button
              type="button"
              onClick={backToCredentials}
              disabled={submitting}
              className="w-full text-center text-sm text-slate-500 hover:text-slate-700 disabled:opacity-60"
            >
              ← Use a different account
            </button>
          </form>
        )}

        <p className="text-center text-xs text-slate-400 mt-6">
          Access is restricted to verified platform administrators. All sign-ins are audited.
        </p>
      </div>
    </div>
  );
}
