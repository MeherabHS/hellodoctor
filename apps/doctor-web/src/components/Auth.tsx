'use client';
import { useEffect, useRef, useState } from 'react';
import { errorText, request } from '../lib/client';
import { Prototype, id, hasClass } from './Prototype';
import { State } from './Common';
import { BotChallenge } from './BotChallenge';
import { allRequiredDocumentsAccepted, filterDoctorApplicableDocuments } from '../lib/legalConsent';

const fields = [
  ['full_name','Full legal name','text'], ['license_number','KMPDC registration number','text'],
  ['primary_specialty','Primary specialty','text'], ['current_hospital','Affiliated hospital','text'],
  ['phone_number','Official phone number (+254)','tel'], ['email','Email address','email'],
  ['password','Password (12–128 characters)','password'], ['experience_years','Years of experience','number'],
  ['qualifications','Qualifications (comma separated)','text'], ['residential_address','Residential address','text']
];

// Fetched from the backend's own public registry (GET /legal/documents),
// never duplicated as static text/identifiers here -- the exact same
// document_type/version pairs this component echoes back on submit are
// whatever the backend most recently returned, so there is nothing to keep
// in sync by hand when a document's version changes.
type LegalDocument = { document_type: string; version: string; title: string; body: string; required: boolean };

export function Auth({ onSignedIn, onBoundary }: { onSignedIn: () => void; onBoundary: () => void }) {
  const [mode, setMode] = useState<'signin' | 'register' | 'mfa' | 'pending'>('signin');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [developmentCode, setDevelopmentCode] = useState<string | null>(null);
  const [challengeToken, setChallengeToken] = useState<string | null>(null);
  const [challengeReset, setChallengeReset] = useState(0);
  const [legalDocs, setLegalDocs] = useState<LegalDocument[] | null>(null);
  const [legalDocsError, setLegalDocsError] = useState('');
  const [acceptedTypes, setAcceptedTypes] = useState<Record<string, boolean>>({});
  const [openDocType, setOpenDocType] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    if (openDocType) dialogRef.current?.showModal();
    else dialogRef.current?.close();
  }, [openDocType]);
  const [legalDocsLoading, setLegalDocsLoading] = useState(false);
  const [legalDocsRetry, setLegalDocsRetry] = useState(0);
  useEffect(() => {
    if (mode !== 'register' || legalDocs !== null) return;
    let cancelled = false;
    setLegalDocsLoading(true);
    setLegalDocsError('');
    // `request<T>` already unwraps the backend's {success,data} envelope and
    // resolves with `data` itself (see lib/client.ts::performRequest) -- this
    // previously typed the resolved value as the FULL envelope and then read
    // `.data` on it again, so `setLegalDocs` was actually called with
    // `undefined` (an array has no `.data` property) on every successful
    // fetch. `undefined`, like the initial `null`, is falsy, so the "Loading
    // required documents…" message below never stopped rendering, and
    // because `undefined !== null`, this effect's own re-fetch guard above
    // also never ran again -- a silent, permanently-stuck loading state that
    // never surfaced as an error, exactly as observed in production.
    //
    // GET /legal/documents is a single shared, role-agnostic registry (no
    // audience filter exists server-side -- see legal_repo.rs::list_current),
    // so its response also includes Patient-only document types that must
    // never reach a Doctor registration form; filtered here via the explicit
    // Doctor allow-list in lib/legalConsent.ts.
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
        const accepted_documents = (legalDocs ?? [])
          .filter(doc => acceptedTypes[doc.document_type])
          .map(doc => ({ type: doc.document_type, version: doc.version }));
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
  if (mode === 'pending') return <main className="web-auth"><div className="sheet-pill-tag">Pending verification</div><h1 className="sheet-title">Application submitted</h1><p className="sheet-desc">Clinical credentialing must approve your professional registration before consultations become available.</p><button className="sheet-btn primary" onClick={() => change('signin')}>Return to sign in</button></main>;
  return <main className="web-auth">
    <div className="sheet-header-top"><span className="sheet-pill-tag">🩺 Doctor Access</span><span className="dw-topbar-brand">AfyaMkononi</span></div>
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
        <p className="sheet-desc">Professional Registration · Kenya Medical Practitioners and Dentists Council (KMPDC) · Kenya. Applications undergo manual AfyaMkononi review.</p>
        {fields.map(([name,label,type]) => <label key={name} htmlFor={name}>{label}<input className="auth-input" id={name} name={name} type={type} required min={type === 'number' ? 0 : undefined} max={type === 'number' ? 100 : undefined} minLength={type === 'password' ? 12 : undefined} maxLength={type === 'password' ? 128 : name === 'full_name' ? 120 : undefined} autoComplete={type === 'password' ? 'new-password' : undefined} /></label>)}
        <div className="web-gap">
          <p className="sheet-desc">Before submitting, review and accept the following requirements.</p>
          {!!legalDocs?.length && <p className="sheet-desc" role="status">{legalDocs.filter(doc => acceptedTypes[doc.document_type]).length}/{legalDocs.length} accepted</p>}
          {legalDocsError && <p role="alert">{legalDocsError} <button type="button" onClick={() => setLegalDocsRetry(value => value + 1)}>Retry</button></p>}
          {!legalDocsError && legalDocsLoading && <p className="sheet-desc" role="status">Loading required documents…</p>}
          {legalDocs?.map(doc => <div key={doc.document_type} style={{ display: 'flex', alignItems: 'center', gap: '0.5em' }}>
            <input
              type="checkbox"
              id={`legal-${doc.document_type}`}
              checked={!!acceptedTypes[doc.document_type]}
              onChange={event => setAcceptedTypes(previous => ({ ...previous, [doc.document_type]: event.target.checked }))}
            />
            <label htmlFor={`legal-${doc.document_type}`}>{doc.title}{doc.required ? ' (required)' : ' (optional)'}</label>
            <button type="button" onClick={() => setOpenDocType(doc.document_type)}>View →</button>
          </div>)}
        </div>
        <dialog ref={dialogRef} onClose={() => setOpenDocType(null)}>
          {(() => {
            const openDoc = legalDocs?.find(doc => doc.document_type === openDocType);
            return openDoc ? <>
              <h2>{openDoc.title}</h2>
              <div style={{ maxHeight: '60vh', overflowY: 'auto' }}>{openDoc.body}</div>
              <button type="button" onClick={() => setOpenDocType(null)}>Close</button>
            </> : null;
          })()}
        </dialog>
        <BotChallenge action="doctor_register" reset={challengeReset} onToken={setChallengeToken} />
        <button className="sheet-btn primary" type="submit" style={{ width: '100%' }} disabled={busy || challengeToken === null}>{busy ? 'Submitting…' : 'Submit Verification Application →'}</button>
      </form> };
      return undefined;
    }} />}
    <State error={error} />
  </main>;
}
