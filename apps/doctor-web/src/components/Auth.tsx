'use client';
import { useEffect, useRef, useState } from 'react';
import { errorText, request } from '../lib/client';
import { State } from './Common';
import { Icon, type IconName } from './Icons';
import { BotChallenge } from './BotChallenge';
import { allRequiredDocumentsAccepted, filterDoctorApplicableDocuments } from '../lib/legalConsent';

/** Doctor sign-up fields, all required. Professional details follow the
 *  product-owner order (2026-11-14): KMPDC registration number, years of
 *  experience, hospital name, specialty, then qualifications and address. */
const fields: [string, string, string, string?][] = [
  ['full_name', 'Full legal name', 'text'], ['email', 'Email address', 'email'],
  ['phone_number', 'Phone number', 'tel', '+254…'], ['password', 'Password', 'password', '8+ characters: upper, lower, number, symbol'],
  ['license_number', 'KMPDC registration number', 'text', 'e.g. A12345'], ['experience_years', 'Years of experience', 'number', '0 to 100'],
  ['current_hospital', 'Hospital name', 'text', 'Where you currently practise'], ['primary_specialty', 'Specialty', 'text', 'e.g. General Practitioner'],
  ['qualifications', 'Qualifications', 'text', 'Comma separated, e.g. MBChB, MMed'], ['residential_address', 'Residential address', 'text'],
];
/** Mirrors the backend's validate_password_strength. */
const STRONG_PASSWORD = '(?=.*[A-Z])(?=.*[a-z])(?=.*[0-9])(?=.*[^A-Za-z0-9]).{8,128}';
const MAX_LENGTH: Record<string, number> = { full_name: 120, password: 128, license_number: 150, current_hospital: 200, primary_specialty: 80, qualifications: 1500, residential_address: 500, email: 255, phone_number: 20 };

// Fetched from the backend's public registry (GET /legal/documents); the
// document_type/version pairs echoed back on submit are exactly what it returned.
type LegalDocument = { document_type: string; version: string; title: string; body: string; required: boolean };

function Art() {
  const points: [IconName, string][] = [['video', 'Secure video and chat consultations'], ['shield', 'KMPDC-verified professionals only'], ['heart', 'Care for patients across Kenya']];
  return <aside className="am-auth-art" aria-hidden="true">
    <div className="am-row" style={{ gap: 12, position: 'relative', zIndex: 1 }}><span className="am-brand-mark"><img src="/brand/afya_logo.png" alt=""/></span><span><strong style={{ fontSize: 20, display: 'block' }}>AfyaMkononi</strong><span style={{ opacity: .8 }}>Doctor Portal</span></span></div>
    <div><h1>Care for your patients, wherever they are.</h1><p>Your consultations, schedule and patient conversations in one calm, secure workspace.</p></div>
    <div className="am-auth-points">{points.map(([icon, text]) => <div key={text} className="am-auth-point"><span><Icon name={icon} size={19}/></span>{text}</div>)}</div>
  </aside>;
}

export function Auth({ onSignedIn, onBoundary, sessionError, onRetry }: { onSignedIn: () => void; onBoundary: () => void; sessionError?: string; onRetry?: () => void }) {
  const [mode, setMode] = useState<'signin' | 'register' | 'mfa' | 'pending' | 'forgot'>('signin');
  const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  const [developmentCode, setDevelopmentCode] = useState<string | null>(null);
  const [challengeToken, setChallengeToken] = useState<string | null>(null); const [challengeReset, setChallengeReset] = useState(0);
  const [legalDocs, setLegalDocs] = useState<LegalDocument[] | null>(null); const [legalDocsError, setLegalDocsError] = useState('');
  const [acceptedTypes, setAcceptedTypes] = useState<Record<string, boolean>>({}); const [openDocType, setOpenDocType] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => { if (openDocType) dialogRef.current?.showModal(); else dialogRef.current?.close(); }, [openDocType]);
  const [legalDocsLoading, setLegalDocsLoading] = useState(false); const [legalDocsRetry, setLegalDocsRetry] = useState(0);
  useEffect(() => {
    if (mode !== 'register' || legalDocs !== null) return;
    let cancelled = false; setLegalDocsLoading(true); setLegalDocsError('');
    // `request<T>` already unwraps {success,data}. The registry is role-agnostic, so
    // Patient-only documents are filtered out with the Doctor allow-list.
    request<LegalDocument[]>('/api/auth/legal-documents')
      .then(documents => { if (!cancelled) setLegalDocs(filterDoctorApplicableDocuments(documents)); })
      .catch(() => { if (!cancelled) setLegalDocsError('Unable to load the required legal documents.'); })
      .finally(() => { if (!cancelled) setLegalDocsLoading(false); });
    return () => { cancelled = true; };
  }, [mode, legalDocs, legalDocsRetry]);
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || ((mode === 'signin' || mode === 'register') && challengeToken === null)) return;
    if (mode === 'register') {
      if (!legalDocs || legalDocs.length === 0) { setError('The required legal documents have not loaded yet. Please wait and try again.'); return; }
      if (!allRequiredDocumentsAccepted(legalDocs, acceptedTypes)) { setError('You must accept every required document before submitting.'); return; }
    }
    setBusy(true); setError('');
    const values = Object.fromEntries(new FormData(event.currentTarget));
    if (mode === 'signin' || mode === 'register') values.turnstile_token = challengeToken ?? '';
    try {
      if (mode === 'register') {
        const accepted_documents = (legalDocs ?? []).filter(doc => acceptedTypes[doc.document_type]).map(doc => ({ type: doc.document_type, version: doc.version }));
        await request('/api/auth/register', { method: 'POST', body: JSON.stringify({ ...values, email: String(values.email).trim().toLowerCase(), experience_years: Number(values.experience_years), qualifications: String(values.qualifications).split(',').map(value => value.trim()).filter(Boolean), accepted_documents }) });
        setMode('pending');
      } else if (mode === 'mfa') {
        await request('/api/auth/verify', { method: 'POST', body: JSON.stringify(values) });
        setDevelopmentCode(null); onBoundary(); onSignedIn();
      } else {
        onBoundary();
        const data = await request<{ development_otp: string | null }>('/api/auth/login', { method: 'POST', body: JSON.stringify(values) });
        setDevelopmentCode(data.development_otp); setMode('mfa');
      }
    } catch (e) { setError(errorText(e)); }
    finally { setBusy(false); setChallengeToken(null); setChallengeReset(value => value + 1); }
  }
  function change(next: 'signin' | 'register') { if (!busy) { setChallengeToken(null); setMode(next); setError(''); setDevelopmentCode(null); setAcceptedTypes({}); } }

  const openDoc = legalDocs?.find(doc => doc.document_type === openDocType);
  return <main className="am-auth">
    <Art/>
    <div className="am-auth-panel">
      <div className={`am-auth-card${mode === 'register' ? ' wide' : ''}`}>
        {mode === 'forgot' ? <ForgotPassword onDone={() => change('signin')}/> : mode === 'pending' ? <div className="am-success">
          <span className="am-success-mark"><Icon name="check" size={38} stroke={2.6}/></span>
          <h2>Application submitted</h2>
          <p className="am-muted">Thank you. AfyaMkononi will review your KMPDC registration, usually within one to two business days. Sign in any time to check your status.</p>
          <button className="am-btn primary lg block" onClick={() => change('signin')}>Back to sign in</button>
        </div> : mode === 'mfa' ? <>
          <span className="am-stat-icon" style={{ marginBottom: 16 }}><Icon name="shield" size={22}/></span>
          <h2>Verify it&apos;s you</h2><p>Enter the 6-digit code sent to your registered contact.</p>
          {developmentCode && <div className="am-callout amber" role="status" style={{ marginTop: 16 }}><Icon name="info"/><span>Development code: <strong>{developmentCode}</strong></span></div>}
          <form className="am-form" style={{ marginTop: 22 }} onSubmit={submit}>
            <label className="am-field" htmlFor="otp"><span>Verification code</span><input className="am-input" id="otp" name="otp_code" inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]{6}" maxLength={6} required autoFocus style={{ fontSize: 22, letterSpacing: '0.4em', textAlign: 'center' }}/></label>
            <button className="am-btn primary lg block" disabled={busy}>{busy ? 'Verifying…' : 'Verify & sign in'}</button>
            <button type="button" className="am-btn ghost block" disabled={busy} onClick={() => change('signin')}>Use a different account</button>
          </form>
        </> : <>
          <div className="am-row" style={{ gap: 10, marginBottom: 18 }}><img src="/brand/afya_logo.png" alt="" width={36} height={36}/><strong className="am-strong" style={{ fontSize: 17 }}>AfyaMkononi Doctor Portal</strong></div>
          <h2>{mode === 'signin' ? 'Welcome back, Doctor' : 'Join AfyaMkononi'}</h2>
          <p>{mode === 'signin' ? 'Sign in to see your consultations for today.' : 'Apply as a licensed practitioner. Every application is reviewed against the KMPDC register.'}</p>
          <div className="am-tabs" style={{ marginTop: 20, width: '100%' }} role="tablist">
            <button className="am-tab am-grow" role="tab" aria-selected={mode === 'signin'} disabled={busy} onClick={() => change('signin')}>Sign in</button>
            <button className="am-tab am-grow" role="tab" aria-selected={mode === 'register'} disabled={busy} onClick={() => change('register')}>Apply to join</button>
          </div>
          {mode === 'signin' ? <form className="am-form" style={{ marginTop: 22 }} onSubmit={submit}>
            <label className="am-field" htmlFor="email"><span>Email address</span><input className="am-input" id="email" name="email" type="email" autoComplete="username" placeholder="you@hospital.co.ke" required/></label>
            <label className="am-field" htmlFor="password"><span className="am-row between"><span>Password</span><button type="button" className="am-link" style={{ fontSize: 13 }} disabled={busy} onClick={() => { setError(''); setMode('forgot'); }}>Forgot password?</button></span><input className="am-input" id="password" name="password" type="password" autoComplete="current-password" required/></label>
            <BotChallenge action="doctor_login" reset={challengeReset} onToken={setChallengeToken}/>
            <button className="am-btn primary lg block" type="submit" disabled={busy || challengeToken === null}>{busy ? 'Signing in…' : 'Sign in securely'}<Icon name="arrowRight" size={17}/></button>
          </form> : <form className="am-form" style={{ marginTop: 22 }} onSubmit={submit}>
            <p className="am-faint" style={{ margin: '0 0 6px' }}>All fields are required (<span className="am-required" aria-hidden>*</span>).</p>
            <div className="am-form-grid">{fields.map(([name, label, type, hint]) => <label key={name} className="am-field" htmlFor={name}><span>{label} <span className="am-required" aria-hidden>*</span></span><input className="am-input" id={name} name={name} type={type} placeholder={hint} required aria-required="true" min={type === 'number' ? 0 : undefined} max={type === 'number' ? 100 : undefined} step={type === 'number' ? 1 : undefined} minLength={name === 'license_number' ? 3 : type === 'password' ? 8 : undefined} pattern={type === 'password' ? STRONG_PASSWORD : undefined} title={type === 'password' ? 'At least 8 characters with an uppercase letter, a lowercase letter, a number and a symbol.' : undefined} maxLength={MAX_LENGTH[name]} autoComplete={type === 'password' ? 'new-password' : undefined}/></label>)}</div>
            <div className="am-stack" style={{ gap: 8 }}>
              <div className="am-row between"><span className="am-label">Agreements</span>{!!legalDocs?.length && <span className="am-faint" role="status">{legalDocs.filter(doc => acceptedTypes[doc.document_type]).length}/{legalDocs.length} accepted</span>}</div>
              {legalDocsError && <p role="alert" className="am-error-text">{legalDocsError} <button type="button" className="am-link" onClick={() => setLegalDocsRetry(value => value + 1)}>Retry</button></p>}
              {!legalDocsError && legalDocsLoading && <p className="am-faint" role="status">Loading required documents…</p>}
              {legalDocs?.map(doc => <div key={doc.document_type} className="am-legal-row">
                <label className="am-check" htmlFor={`legal-${doc.document_type}`}><input type="checkbox" id={`legal-${doc.document_type}`} checked={!!acceptedTypes[doc.document_type]} onChange={event => setAcceptedTypes(previous => ({ ...previous, [doc.document_type]: event.target.checked }))}/><span>{doc.title}{doc.required ? '' : ' (optional)'}</span></label>
                <button type="button" className="am-link" onClick={() => setOpenDocType(doc.document_type)}>Read</button>
              </div>)}
            </div>
            <dialog ref={dialogRef} className="am-legal-dialog" onClose={() => setOpenDocType(null)}>
              {openDoc && <><div className="am-dialog-head"><h2>{openDoc.title}</h2><button type="button" className="am-close" aria-label="Close" onClick={() => setOpenDocType(null)}><Icon name="close"/></button></div><div className="am-dialog-body am-pre" style={{ maxHeight: '60vh', overflowY: 'auto' }}>{openDoc.body}</div></>}
            </dialog>
            <BotChallenge action="doctor_register" reset={challengeReset} onToken={setChallengeToken}/>
            <button className="am-btn primary lg block" type="submit" disabled={busy || challengeToken === null}>{busy ? 'Submitting…' : 'Submit application'}<Icon name="arrowRight" size={17}/></button>
          </form>}
        </>}
        <div style={{ marginTop: 14 }}><State compact error={error}/></div>
        {sessionError && mode === 'signin' && <div className="am-callout amber" style={{ marginTop: 12 }}><Icon name="info"/><span className="am-grow">{sessionError}</span>{onRetry && <button className="am-btn ghost sm" onClick={onRetry}>Restore session</button>}</div>}
        <p className="am-faint" style={{ marginTop: 24, textAlign: 'center' }}>Protected access for licensed practitioners in Kenya.</p>
      </div>
    </div>
  </main>;
}

/** Forgot password: email a code, verify it, set a new password. The reset
 * authorization never reaches the browser (HttpOnly cookie in the BFF). */
function ForgotPassword({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<'email' | 'code' | 'password' | 'done'>('email');
  const [email, setEmail] = useState(''); const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  const [developmentCode, setDevelopmentCode] = useState<string | null>(null);
  const [challengeToken, setChallengeToken] = useState<string | null>(null); const [challengeReset, setChallengeReset] = useState(0);
  const [password, setPassword] = useState(''); const [confirm, setConfirm] = useState('');
  async function run(task: () => Promise<void>) { setBusy(true); setError(''); try { await task(); } catch (e) { setError(errorText(e)); } finally { setBusy(false); } }
  const requestCode = (event?: React.FormEvent) => { event?.preventDefault(); void run(async () => {
    const data = await request<{ development_otp: string | null }>('/api/auth/reset-request', { method: 'POST', body: JSON.stringify({ email, turnstile_token: challengeToken ?? '' }) });
    setDevelopmentCode(data.development_otp); setStep('code');
  }).finally(() => { setChallengeToken(null); setChallengeReset(v => v + 1); }); };
  const steps: [string, string][] = [['email', 'Email'], ['code', 'Code'], ['password', 'New password']];
  const index = steps.findIndex(([key]) => key === step);
  if (step === 'done') return <div className="am-success">
    <span className="am-success-mark"><Icon name="check" size={38} stroke={2.6}/></span>
    <h2>Password updated</h2>
    <p className="am-muted">Your password has been changed. Sign in with your new password.</p>
    <button className="am-btn primary lg block" onClick={onDone}>Back to sign in</button>
  </div>;
  return <>
    <button type="button" className="am-crumb" onClick={onDone} disabled={busy}><Icon name="arrowLeft" size={15}/>Back to sign in</button>
    <span className="am-stat-icon" style={{ margin: '10px 0 16px' }}><Icon name="key" size={22}/></span>
    <h2>Reset your password</h2>
    <p>{step === 'email' ? "Enter your registered email and we'll send you a 6-digit code." : step === 'code' ? `If an account exists for ${email}, a code is on its way. Enter it below.` : 'Choose a new password you have not used before.'}</p>
    <div className="am-row" style={{ gap: 6, margin: '18px 0 4px' }}>{steps.map(([key, label], i) => <span key={key} className={`am-badge ${i < index ? 'green' : i === index ? 'teal' : ''}`}>{i < index ? <Icon name="check" size={12}/> : `${i + 1}.`} {label}</span>)}</div>
    {step === 'email' && <form className="am-form" style={{ marginTop: 18 }} onSubmit={requestCode}>
      <label className="am-field" htmlFor="resetEmail"><span>Email address</span><input className="am-input" id="resetEmail" type="email" autoComplete="username" required value={email} disabled={busy} placeholder="you@hospital.co.ke" onChange={e => setEmail(e.target.value)} autoFocus/></label>
      <BotChallenge action="doctor_password_reset" reset={challengeReset} onToken={setChallengeToken}/>
      <button className="am-btn primary lg block" disabled={busy || challengeToken === null || !email.trim()}>{busy ? 'Sending…' : 'Send reset code'}<Icon name="arrowRight" size={17}/></button>
    </form>}
    {step === 'code' && <form className="am-form" style={{ marginTop: 18 }} onSubmit={e => { e.preventDefault(); const code = String(new FormData(e.currentTarget).get('otp_code') ?? ''); void run(async () => { await request('/api/auth/reset-verify', { method: 'POST', body: JSON.stringify({ email, otp_code: code }) }); setStep('password'); }); }}>
      {developmentCode && <div className="am-callout amber" role="status"><Icon name="info"/><span>Development code: <strong>{developmentCode}</strong></span></div>}
      <label className="am-field" htmlFor="resetCode"><span>Verification code</span><input className="am-input" id="resetCode" name="otp_code" inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]{6}" maxLength={6} required autoFocus style={{ fontSize: 22, letterSpacing: '0.4em', textAlign: 'center' }}/></label>
      <button className="am-btn primary lg block" disabled={busy}>{busy ? 'Checking…' : 'Verify code'}</button>
      <button type="button" className="am-btn ghost block" disabled={busy} onClick={() => { setStep('email'); setDevelopmentCode(null); }}>Use a different email or resend</button>
    </form>}
    {step === 'password' && <form className="am-form" style={{ marginTop: 18 }} onSubmit={e => { e.preventDefault(); if (password !== confirm) { setError('The passwords do not match.'); return; } void run(async () => { await request('/api/auth/reset-complete', { method: 'POST', body: JSON.stringify({ new_password: password }) }); setStep('done'); }); }}>
      <label className="am-field" htmlFor="resetPassword"><span>New password</span><input className="am-input" id="resetPassword" type="password" autoComplete="new-password" required minLength={12} maxLength={128} value={password} disabled={busy} onChange={e => setPassword(e.target.value)} autoFocus/><small>12 to 128 characters.</small></label>
      <label className="am-field" htmlFor="resetConfirm"><span>Confirm new password</span><input className="am-input" id="resetConfirm" type="password" autoComplete="new-password" required value={confirm} disabled={busy} onChange={e => setConfirm(e.target.value)}/></label>
      <button className="am-btn primary lg block" disabled={busy || !password || !confirm}><Icon name="key" size={17}/>{busy ? 'Updating…' : 'Set new password'}</button>
    </form>}
    <div style={{ marginTop: 14 }}><State compact error={error}/></div>
  </>;
}
