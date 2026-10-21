'use client';
import { useState } from 'react';
import { errorText, request } from '../lib/client';
import { Prototype, id, hasClass } from './Prototype';
import { State } from './Common';
import { BotChallenge } from './BotChallenge';

const fields = [
  ['full_name','Full legal name','text'], ['license_number','KMPDC registration number','text'],
  ['primary_specialty','Primary specialty','text'], ['current_hospital','Affiliated hospital','text'],
  ['phone_number','Official phone number (+254)','tel'], ['email','Email address','email'],
  ['password','Password (12–128 characters)','password'], ['experience_years','Years of experience','number'],
  ['qualifications','Qualifications (comma separated)','text'], ['residential_address','Residential address','text']
];
export function Auth({ onSignedIn, onBoundary }: { onSignedIn: () => void; onBoundary: () => void }) {
  const [mode, setMode] = useState<'signin' | 'register' | 'mfa' | 'pending'>('signin');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [developmentCode, setDevelopmentCode] = useState<string | null>(null);
  const [challengeToken, setChallengeToken] = useState<string | null>(null);
  const [challengeReset, setChallengeReset] = useState(0);
  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy || ((mode === 'signin' || mode === 'register') && challengeToken === null)) return;
    setBusy(true); setError('');
    const values = Object.fromEntries(new FormData(event.currentTarget));
    if (mode === 'signin' || mode === 'register') values.turnstile_token = challengeToken ?? '';
    try {
      if (mode === 'register') {
        await request('/api/auth/register', { method: 'POST', body: JSON.stringify({ ...values, email: String(values.email).trim().toLowerCase(), experience_years: Number(values.experience_years), qualifications: String(values.qualifications).split(',').map(value => value.trim()).filter(Boolean) }) });
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
  function change(next: 'signin' | 'register') { if (!busy) { setChallengeToken(null); setMode(next); setError(''); setDevelopmentCode(null); } }
  if (mode === 'pending') return <main className="web-auth"><div className="sheet-pill-tag">Pending verification</div><h1 className="sheet-title">Application submitted</h1><p className="sheet-desc">Clinical credentialing must approve your professional registration before consultations become available.</p><button className="sheet-btn primary" onClick={() => change('signin')}>Return to sign in</button></main>;
  return <main className="web-auth">
    <div className="sheet-header-top"><span className="sheet-pill-tag">🩺 Doctor Access</span><span className="dw-topbar-brand">HelloDoctor</span></div>
    {mode === 'mfa' ? <>
      <h1 className="sheet-title">Verify your session</h1><p className="sheet-desc">Enter the one-time verification code sent through your configured channel.</p>
      {developmentCode && <p className="web-gap" role="status">Development backend code: <strong>{developmentCode}</strong></p>}
      <form onSubmit={submit}><label htmlFor="otp">Verification code</label><input className="auth-input" id="otp" name="otp_code" inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]{6}" maxLength={6} required autoFocus /><button className="sheet-btn primary" style={{ width: '100%' }} disabled={busy}>{busy ? 'Verifying…' : 'Verify & sign in →'}</button></form>
      <button className="auth-tab" disabled={busy} onClick={() => change('signin')}>Sign in again</button>
    </> : <Prototype name="doctorAuthSection" override={n => {
      const nodeId = id(n);
      if (nodeId === 'doctorAuthSection') return { props: { style: { display: 'block' } } };
      if (nodeId === 'docAuthTitle') return { tag: 'h1', children: mode === 'signin' ? 'Physician Secure Login' : 'Professional Verification Application' };
      if (nodeId === 'docAuthDesc') return { children: 'Licensed medical practitioners gateway for consultations, clinical history & handwritten prescriptions.' };
      if (hasClass(n, 'demo-btn-box')) return { omit: true };
      if (nodeId === 'docTabSignIn' || nodeId === 'docTabRegister') return { props: { className: `auth-tab ${(nodeId === 'docTabSignIn') === (mode === 'signin') ? 'active' : ''}`, disabled: busy, onClick: () => change(nodeId === 'docTabSignIn' ? 'signin' : 'register') } };
      if (nodeId === 'docSignInFields') return { children: mode !== 'signin' ? null : <form onSubmit={submit}>
        <label htmlFor="email">Email address</label><input className="auth-input" id="email" name="email" type="email" autoComplete="username" placeholder="Your registered email address" required />
        <label htmlFor="password">Password</label><input className="auth-input" id="password" name="password" type="password" autoComplete="current-password" placeholder="Your password" required />
        <BotChallenge action="doctor_login" reset={challengeReset} onToken={setChallengeToken} />
        <button className="sheet-btn primary" type="submit" style={{ width: '100%' }} disabled={busy || challengeToken === null}>{busy ? 'Signing in…' : 'Doctor Secure Sign In →'}</button>
      </form> };
      if (nodeId === 'docRegisterFields') return { props: { style: { display: mode === 'register' ? 'block' : 'none' } }, children: mode !== 'register' ? null : <form onSubmit={submit}>
        <p className="sheet-desc">Professional Registration · Kenya Medical Practitioners and Dentists Council (KMPDC) · Kenya. Applications undergo manual HelloDoctor review.</p>
        {fields.map(([name,label,type]) => <label key={name} htmlFor={name}>{label}<input className="auth-input" id={name} name={name} type={type} required min={type === 'number' ? 0 : undefined} max={type === 'number' ? 100 : undefined} minLength={type === 'password' ? 12 : undefined} maxLength={type === 'password' ? 128 : name === 'full_name' ? 120 : undefined} autoComplete={type === 'password' ? 'new-password' : undefined} /></label>)}
        <BotChallenge action="doctor_register" reset={challengeReset} onToken={setChallengeToken} />
        <button className="sheet-btn primary" type="submit" style={{ width: '100%' }} disabled={busy || challengeToken === null}>{busy ? 'Submitting…' : 'Submit Verification Application →'}</button>
      </form> };
      return undefined;
    }} />}
    <State error={error} />
  </main>;
}
