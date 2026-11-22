'use client';
import {useEffect, useRef, useState} from 'react';
import {Camera, CheckCircle2, KeyRound, Trash2} from 'lucide-react';
import {tr} from '@/lib/i18n';
import {apiErrorMessage, parseApiError} from '@/lib/i18n/apiErrors';
import {changePassword, getMe, preparePhoto, removeMyPhoto, uploadMyPhoto, type AdminMe} from '@/lib/profileApi';
import {Card, PageHead, eatDate} from '../ui/afya';

/**
 * The signed-in Admin's own profile (product owner, 2026-11-22): account
 * details, profile photo and password change. Opened from the account pill in
 * the top bar; not a sidebar page.
 */
function errorText(e: unknown, fallback: string): string {
  return parseApiError(e).status ? apiErrorMessage(e, fallback) : e instanceof Error && e.message ? e.message : fallback;
}

/** Two letters for the avatar when there is no photo, from the email's name part. */
export function initialsOf(email: string | null | undefined): string {
  const name = (email ?? '').split('@')[0].replace(/[^A-Za-z0-9]+/g, ' ').trim();
  const parts = name.split(' ').filter(Boolean);
  const letters = parts.length > 1 ? parts[0][0] + parts[1][0] : name.slice(0, 2);
  return (letters || 'AD').toUpperCase();
}

export function AdminProfilePage({email, photo, onPhotoChanged}: {email: string | null | undefined; photo: string | null; onPhotoChanged: () => void}) {
  const [me, setMe] = useState<AdminMe | null>(null);
  const [loadError, setLoadError] = useState('');
  useEffect(() => {
    let active = true;
    getMe().then(m => { if (active) setMe(m); }).catch(() => { if (active) setLoadError(tr('profile.loadFailed')); });
    return () => { active = false; };
  }, []);

  return <div className="afya-stack">
    <PageHead title={tr('profile.title')} subtitle={tr('profile.subtitle')}/>
    {loadError && <p className="afya-profile-error" role="alert">{loadError}</p>}
    <div className="afya-profile-grid">
      <div className="afya-stack">
        <PhotoCard email={me?.email ?? email} photo={photo} onPhotoChanged={onPhotoChanged}/>
        <Card title={tr('profile.account')}>
          <dl className="afya-profile-facts">
            <dt>{tr('profile.email')}</dt><dd>{me ? me.email ?? '—' : '…'}</dd>
            <dt>{tr('profile.phone')}</dt><dd>{me ? me.phone : '…'}</dd>
            <dt>{tr('profile.role')}</dt><dd>{tr('ui.administrator')}</dd>
            <dt>{tr('profile.since')}</dt><dd>{me ? eatDate(me.created_at) : '…'}</dd>
          </dl>
        </Card>
      </div>
      <PasswordCard/>
    </div>
  </div>;
}

function PhotoCard({email, photo, onPhotoChanged}: {email: string | null | undefined; photo: string | null; onPhotoChanged: () => void}) {
  const picker = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  async function upload(file: File) {
    setBusy(true); setError('');
    try { await uploadMyPhoto(await preparePhoto(file, {type: tr('profile.photoType'), tooLarge: tr('profile.photoTooLarge')})); onPhotoChanged(); }
    catch (e) { setError(errorText(e, tr('profile.photoFailed'))); }
    finally { setBusy(false); }
  }
  async function remove() {
    if (!window.confirm(tr('profile.removeConfirm'))) return;
    setBusy(true); setError('');
    try { await removeMyPhoto(); onPhotoChanged(); }
    catch (e) { setError(errorText(e, tr('profile.photoFailed'))); }
    finally { setBusy(false); }
  }
  return <Card title={tr('profile.photo')} subtitle={tr('profile.photoHint')}>
    <div className="afya-profile-photo">
      <div className="admin-avatar afya-profile-avatar" aria-hidden>
        {/* eslint-disable-next-line @next/next/no-img-element -- a same-origin data: URL, not an optimisable remote image */}
        {photo ? <img src={photo} alt=""/> : initialsOf(email)}
      </div>
      <div className="afya-profile-photo-actions">
        <input ref={picker} type="file" hidden accept="image/jpeg,image/png,image/webp" aria-label={tr('profile.photo')}
          onChange={e => { const f = e.target.files?.[0]; e.target.value = ''; if (f) void upload(f); }}/>
        <button type="button" className="afya-btn-secondary" disabled={busy} onClick={() => picker.current?.click()}>
          <Camera size={15} aria-hidden/>{busy ? tr('profile.saving') : photo ? tr('profile.change') : tr('profile.upload')}
        </button>
        {photo && <button type="button" className="afya-btn-danger" disabled={busy} onClick={() => void remove()}>
          <Trash2 size={15} aria-hidden/>{tr('profile.remove')}
        </button>}
      </div>
    </div>
    {error && <p className="afya-profile-error" role="alert">{error}</p>}
  </Card>;
}

function PasswordCard() {
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [done, setDone] = useState(false);
  const edit = (set: (v: string) => void) => (e: React.ChangeEvent<HTMLInputElement>) => { set(e.target.value); setError(''); setDone(false); };
  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (busy) return;
    setError(''); setDone(false);
    if (next.length < 12 || next.length > 128) { setError(tr('profile.tooShort')); return; }
    if (next !== confirm) { setError(tr('profile.mismatch')); return; }
    if (next === current) { setError(tr('profile.same')); return; }
    setBusy(true);
    try { await changePassword(current, next); setDone(true); setCurrent(''); setNext(''); setConfirm(''); }
    catch (err) { setError(errorText(err, tr('ui2.theRequestCouldNotBeCompleted'))); }
    finally { setBusy(false); }
  }
  return <Card title={tr('profile.passwordTitle')} subtitle={tr('profile.passwordHint')}>
    <form className="afya-form" onSubmit={submit} noValidate>
      <label htmlFor="profile-current">{tr('profile.current')}</label>
      <input id="profile-current" type="password" autoComplete="current-password" maxLength={128} value={current} disabled={busy} onChange={edit(setCurrent)}/>
      <label htmlFor="profile-next">{tr('profile.next')}</label>
      <input id="profile-next" type="password" autoComplete="new-password" minLength={12} maxLength={128} value={next} disabled={busy} onChange={edit(setNext)}/>
      <small className="afya-profile-hint">{tr('profile.nextHint')}</small>
      <label htmlFor="profile-confirm">{tr('profile.confirm')}</label>
      <input id="profile-confirm" type="password" autoComplete="new-password" maxLength={128} value={confirm} disabled={busy} onChange={edit(setConfirm)}/>
      {error && <p className="afya-profile-error" role="alert">{error}</p>}
      <div className="afya-profile-submit">
        {done && <span className="afya-profile-ok" role="status"><CheckCircle2 size={16} aria-hidden/>{tr('profile.changed')}</span>}
        <button type="submit" className="afya-btn-primary" disabled={busy || !current || !next || !confirm}>
          <KeyRound size={15} aria-hidden/>{busy ? tr('profile.updating') : tr('profile.update')}
        </button>
      </div>
    </form>
  </Card>;
}
