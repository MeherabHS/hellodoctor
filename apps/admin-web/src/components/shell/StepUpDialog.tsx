'use client';
import {useEffect, useRef, useState} from 'react';
import {KeyRound, ShieldCheck} from 'lucide-react';
import {confirmAdminPassword} from '@/lib/api';
import {registerStepUpPrompt} from '@/lib/stepUp.mjs';

/**
 * Asks the signed-in Admin for their password before a sensitive action
 * (doctor and peer decisions, money, erasure, identity traces). Mounted once
 * in the Admin shell; `request()` opens it on a `REAUTH_REQUIRED` answer and
 * retries the action after a successful confirmation. See lib/stepUp.mjs.
 */
export function StepUpDialog() {
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const settle = useRef<((ok: boolean) => void) | null>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => registerStepUpPrompt(() => new Promise<boolean>(resolve => {
    settle.current = resolve;
    setPassword(''); setError(''); setBusy(false); setOpen(true);
  })), []);
  useEffect(() => { if (open) input.current?.focus(); }, [open]);

  function finish(ok: boolean) {
    setOpen(false); setPassword(''); setError('');
    settle.current?.(ok); settle.current = null;
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!password || password.length > 128 || busy) return;
    setBusy(true); setError('');
    try {
      await confirmAdminPassword(password);
      finish(true);
    } catch (err) {
      const text = err instanceof Error ? err.message : '';
      setError(text.includes('[403]') ? 'That password is not correct.' : text.includes('[429]') ? 'Too many wrong passwords. Try again in 15 minutes.' : 'Could not check your password. Try again.');
      setPassword('');
      input.current?.focus();
    } finally {
      setBusy(false);
    }
  }

  if (!open) return null;
  return <div className="admin-modal-backdrop" style={{zIndex: 10001}} onKeyDown={e => { if (e.key === 'Escape' && !busy) finish(false); }}>
    <section role="dialog" aria-modal="true" aria-labelledby="stepup-title" aria-describedby="stepup-description" className="admin-modal-card" style={{maxWidth: 440, padding: 0}}>
      <form onSubmit={submit} noValidate>
        <div className="afya-modal-head" style={{justifyContent: 'flex-start'}}>
          <span style={{display: 'grid', placeItems: 'center', width: 36, height: 36, borderRadius: 10, background: 'var(--afya-green-soft)', color: 'var(--afya-green)'}}><ShieldCheck size={20} aria-hidden/></span>
          <div>
            <h2 id="stepup-title" className="afya-modal-title" style={{margin: 0}}>Confirm it&apos;s you</h2>
            <p id="stepup-description" className="afya-modal-sub" style={{margin: 0}}>This decision needs your password. You won&apos;t be asked again for 10 minutes.</p>
          </div>
        </div>
        <div style={{padding: '18px 22px'}}>
          <label className="afya-search" style={{width: '100%'}}>
            <KeyRound size={16} color="var(--afya-faint)" aria-hidden/>
            <input ref={input} type="password" aria-label="Your current password" placeholder="Your current password" autoComplete="current-password" maxLength={128} value={password} onChange={e => { setPassword(e.target.value); setError(''); }}/>
          </label>
          {error && <div className="afya-login-error" role="alert" style={{marginTop: 12}}>{error}</div>}
          <p className="afya-stat-note" style={{margin: '12px 0 0'}}>Wrong passwords are recorded in the Activity Log.</p>
        </div>
        <div className="afya-modal-foot">
          <button type="button" className="afya-btn-secondary" disabled={busy} onClick={() => finish(false)}>Cancel</button>
          <button type="submit" className="afya-btn-primary" disabled={!password || busy}><ShieldCheck size={16} aria-hidden/>{busy ? 'Checking…' : 'Confirm and continue'}</button>
        </div>
      </form>
    </section>
  </div>;
}
