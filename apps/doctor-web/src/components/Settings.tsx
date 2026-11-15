'use client';
import { isProfessionallyVerified, professionalVerificationLabel } from '../lib/professionalVerification';
import { useRef, useState } from 'react';
import { api, ApiError, errorText, post } from '../lib/client';
import { Profile } from '../lib/contracts';
import { Avatar, Badge, PageHeader, State } from './Common';
import { Icon, type IconName } from './Icons';
import { DeferredFinancialNotice, PayoutSettings } from './Wallet';

export type SettingsSection = 'profile' | 'professional' | 'availability' | 'payout' | 'security';
type ProfileExtra = Profile & { verified_phone?: string; experience_years?: number };

/** Resize to at most 512 px and re-encode as JPEG in the browser (canvas only, no storage). */
async function preparePhoto(file: File): Promise<{ mime_type: string; base64_content: string }> {
  if (!['image/jpeg', 'image/png', 'image/webp'].includes(file.type)) throw new Error('Choose a JPEG, PNG or WebP photo.');
  if (file.size > 10 * 1024 * 1024) throw new Error('Choose a photo smaller than 10 MB.');
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, 512 / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement('canvas'); canvas.width = Math.round(bitmap.width * scale); canvas.height = Math.round(bitmap.height * scale);
  canvas.getContext('2d')!.drawImage(bitmap, 0, 0, canvas.width, canvas.height); bitmap.close();
  const data = canvas.toDataURL('image/jpeg', 0.86);
  return { mime_type: 'image/jpeg', base64_content: data.split(',')[1] };
}

export function Settings({ profile, photo, onPhotoChanged, section, onSection, onUpdated, duty, onDuty, onWallet, onLogout, paymentRequired = true }: { profile: Profile; photo?: string; onPhotoChanged: () => void; section: SettingsSection; onSection: (section: SettingsSection) => void; onUpdated: (profile: Profile) => void; duty: { value?: boolean; busy: boolean; error: string }; onDuty: () => void; onWallet: () => void; onLogout: () => void; paymentRequired?: boolean }) {
  const p = profile as ProfileExtra;
  const sections: [SettingsSection, string, IconName][] = [['profile', 'Profile', 'user'], ['professional', 'Professional details', 'shield'], ['availability', 'Availability', 'clock'], ['payout', 'Payout', 'wallet'], ['security', 'Security', 'lock']];
  return <section id="webSettingsWorkspace" className="am-stack" style={{ gap: 22 }}>
    <PageHeader title="Settings" subtitle="Your profile, professional details and account preferences."/>
    <div className="am-grid settings" style={{ alignItems: 'start' }}>
      <nav className="am-card am-card-body tight" aria-label="Settings sections">
        {sections.map(([key, label, icon]) => <button key={key} className="am-menu-item" style={{ padding: '11px 12px', ...(section === key ? { background: 'var(--am-green-soft)', color: 'var(--am-green-deep)' } : {}) }} aria-current={section === key ? 'page' : undefined} aria-controls={`settings-${key}`} onClick={() => onSection(key)}><Icon name={icon} size={18}/>{label}</button>)}
      </nav>
      <div>
        {section === 'profile' && <ProfileSection profile={p} photo={photo} onPhotoChanged={onPhotoChanged} onUpdated={onUpdated}/>}
        {section === 'professional' && <section id="settings-professional" className="am-card" aria-label="Professional details">
          <div className="am-card-head"><div><h2>Professional details</h2><p>Verified by AfyaMkononi against your KMPDC registration.</p></div><Badge tone={isProfessionallyVerified(profile) ? 'green' : 'amber'} dot>{professionalVerificationLabel(profile)}</Badge></div>
          <div className="am-card-body am-stack">
            <div className="am-person"><Avatar name={profile.full_name} size="lg" src={photo}/><div><strong style={{ fontSize: 17 }}>{profile.full_name}</strong><span>{profile.primary_specialty}</span></div></div>
            <dl className="am-details">
              <dt>Regulatory authority</dt><dd>Kenya Medical Practitioners and Dentists Council (KMPDC)</dd>
              <dt>Registration number</dt><dd>{profile.license_number}</dd>
              <dt>Country</dt><dd>Kenya ({profile.license_country})</dd>
              <dt>Specialty</dt><dd>{profile.primary_specialty}</dd>
              {typeof p.experience_years === 'number' && <><dt>Clinical experience</dt><dd>{p.experience_years} years</dd></>}
              <dt>Hospital / institution</dt><dd>{profile.current_hospital || 'Not provided'}</dd>
              <dt>Qualifications</dt><dd>{profile.qualifications.join(', ') || 'Not provided'}</dd>
            </dl>
            <div className="am-callout blue"><Icon name="info"/><span>These details are read-only. Changes to credentials need a new review by AfyaMkononi.</span></div>
          </div>
        </section>}
        {section === 'availability' && <section id="settings-availability" className="am-card" aria-label="Availability settings">
          <div className="am-card-head"><div><h2>Availability</h2><p>Let the team know whether you are taking consultations right now.</p></div></div>
          <div className="am-card-body am-stack">
            <div className="am-row between" style={{ padding: 16, border: '1px solid var(--am-line)', borderRadius: 14 }}>
              <div><strong className="am-strong">On duty</strong><p className="am-faint">{duty.value ? 'You are shown as online and ready.' : 'You are shown as offline.'}</p></div>
              <button className="am-switch" role="switch" aria-label="On duty" aria-checked={duty.value ?? false} disabled={duty.busy || duty.value === undefined} onClick={onDuty}/>
            </div>
            <p className="am-faint">Bookable times are managed in Schedule &amp; Slots.</p>
            <State error={duty.error}/>
          </div>
        </section>}
        {section === 'payout' && <section id="settings-payout" className="am-card" aria-label="Payout settings">
          <div className="am-card-head"><div><h2>Payout</h2><p>Where Finance sends your earnings.</p></div>{paymentRequired && <button className="am-btn ghost sm" onClick={onWallet}><Icon name="wallet" size={15}/>View earnings</button>}</div>
          <div className="am-card-body">{paymentRequired ? <PayoutSettings/> : <DeferredFinancialNotice/>}</div>
        </section>}
        {section === 'security' && <SecuritySection onLogout={onLogout}/>}
      </div>
    </div>
  </section>;
}

function ProfileSection({ profile, photo, onPhotoChanged, onUpdated }: { profile: ProfileExtra; photo?: string; onPhotoChanged: () => void; onUpdated: (profile: Profile) => void }) {
  const [name, setName] = useState(profile.full_name); const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [fieldError, setFieldError] = useState(''); const [done, setDone] = useState(false);
  const [photoBusy, setPhotoBusy] = useState(false); const [photoError, setPhotoError] = useState('');
  const lock = useRef(false); const picker = useRef<HTMLInputElement>(null);
  async function save(event: React.FormEvent) {
    event.preventDefault(); if (lock.current) return;
    setError(''); setFieldError(''); setDone(false);
    if (!name.trim() || [...name].length > 150) { setFieldError('Enter a display name of 1–150 characters.'); return; }
    lock.current = true; setBusy(true);
    try { await api('doctor/profile', { method: 'PATCH', body: JSON.stringify({ full_name: name.trim() }) }); const updated = await api<Profile>('doctor/profile'); onUpdated(updated); setDone(true); }
    catch (e) { if (e instanceof ApiError && [400, 422].includes(e.status)) setFieldError(errorText(e)); else setError(errorText(e)); }
    finally { lock.current = false; setBusy(false); }
  }
  async function upload(file: File) {
    setPhotoBusy(true); setPhotoError('');
    try { await post('doctor/profile/photo', await preparePhoto(file)); onPhotoChanged(); }
    catch (e) { setPhotoError(e instanceof ApiError ? errorText(e) : e instanceof Error ? e.message : 'Unable to update your photo.'); } finally { setPhotoBusy(false); }
  }
  async function remove() {
    if (!window.confirm('Remove your profile photo?')) return;
    setPhotoBusy(true); setPhotoError('');
    try { await api('doctor/profile/photo', { method: 'DELETE' }); onPhotoChanged(); } catch (e) { setPhotoError(errorText(e)); } finally { setPhotoBusy(false); }
  }
  return <section id="settings-profile" className="am-card" aria-label="Profile settings">
    <div className="am-card-head"><div><h2>Profile</h2><p>How you appear to patients on AfyaMkononi.</p></div></div>
    <div className="am-card-body am-stack" style={{ gap: 22 }}>
      <div className="am-row" style={{ gap: 18 }}>
        <Avatar name={profile.full_name} size="xl" src={photo}/>
        <div className="am-stack" style={{ gap: 8 }}>
          <strong className="am-strong">Profile photo</strong>
          <span className="am-faint">A clear, friendly photo helps patients feel at ease. JPEG, PNG or WebP.</span>
          <div className="am-row"><input ref={picker} type="file" hidden accept="image/jpeg,image/png,image/webp" aria-label="Profile photo file" onChange={e => { const f = e.target.files?.[0]; e.target.value = ''; if (f) void upload(f); }}/>
            <button className="am-btn soft sm" disabled={photoBusy} onClick={() => picker.current?.click()}><Icon name="camera" size={15}/>{photoBusy ? 'Saving…' : photo ? 'Change photo' : 'Upload photo'}</button>
            {photo && <button className="am-btn ghost sm" disabled={photoBusy} onClick={() => void remove()}>Remove</button>}</div>
          {photoError && <p className="am-error-text" role="alert">{photoError}</p>}
        </div>
      </div>
      <form className="am-form" onSubmit={event => void save(event)}>
        <div className="am-form-grid">
          <label className="am-field" htmlFor="settingsDisplayName"><span>Display name</span><input id="settingsDisplayName" className="am-input" required maxLength={150} value={name} disabled={busy} aria-invalid={!!fieldError} aria-describedby={fieldError ? 'settingsNameError' : 'settingsNameHelp'} onChange={e => { setName(e.target.value); setDone(false); setFieldError(''); }}/></label>
          <label className="am-field"><span>Professional role</span><input className="am-input" value={profile.primary_specialty} disabled readOnly/></label>
          {profile.verified_phone && <label className="am-field"><span>Verified phone</span><input className="am-input" value={profile.verified_phone} disabled readOnly/></label>}
          <label className="am-field"><span>Hospital / institution</span><input className="am-input" value={profile.current_hospital || 'Not provided'} disabled readOnly/></label>
        </div>
        <p id="settingsNameHelp" className="am-faint">Only your display name can be changed here. Verified professional details are managed by AfyaMkononi.</p>
        {fieldError && <p id="settingsNameError" role="alert" className="am-error-text">{fieldError}</p>}
        <State error={error}/>
        <div className="am-row end">{done && <span role="status" className="am-ok-text"><Icon name="checkCircle" size={16}/>Profile updated</span>}<button className="am-btn primary" type="submit" disabled={busy}><Icon name="check" size={16}/>{busy ? 'Saving…' : 'Save changes'}</button></div>
      </form>
    </div>
  </section>;
}

function SecuritySection({ onLogout }: { onLogout: () => void }) {
  const [current, setCurrent] = useState(''); const [next, setNext] = useState(''); const [confirm, setConfirm] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [done, setDone] = useState(false);
  async function submit(event: React.FormEvent) {
    event.preventDefault(); setError(''); setDone(false);
    if (next.length < 12 || next.length > 128) { setError('Use a new password of 12 to 128 characters.'); return; }
    if (next !== confirm) { setError('The new passwords do not match.'); return; }
    setBusy(true);
    try { await post('auth/change-password', { current_password: current, new_password: next }); setDone(true); setCurrent(''); setNext(''); setConfirm(''); }
    catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  }
  return <div className="am-stack" style={{ gap: 18 }}>
    <section id="settings-security" className="am-card" aria-label="Change password">
      <div className="am-card-head"><div><h2>Change password</h2><p>Your other signed-in sessions will be signed out. This session stays open.</p></div></div>
      <form className="am-card-body am-form" onSubmit={event => void submit(event)}>
        <label className="am-field"><span>Current password</span><input className="am-input" type="password" autoComplete="current-password" required value={current} disabled={busy} onChange={e => setCurrent(e.target.value)}/></label>
        <div className="am-form-grid">
          <label className="am-field"><span>New password</span><input className="am-input" type="password" autoComplete="new-password" required minLength={12} maxLength={128} value={next} disabled={busy} onChange={e => setNext(e.target.value)}/><small>12 to 128 characters.</small></label>
          <label className="am-field"><span>Confirm new password</span><input className="am-input" type="password" autoComplete="new-password" required value={confirm} disabled={busy} onChange={e => setConfirm(e.target.value)}/></label>
        </div>
        <State error={error}/>
        <div className="am-row end">{done && <span role="status" className="am-ok-text"><Icon name="checkCircle" size={16}/>Password changed</span>}<button className="am-btn primary" type="submit" disabled={busy || !current || !next}><Icon name="key" size={16}/>{busy ? 'Updating…' : 'Update password'}</button></div>
      </form>
    </section>
    <section className="am-card"><div className="am-card-head"><div><h2>This session</h2><p>Sign out when you finish working on a shared computer.</p></div><button className="am-btn danger" onClick={onLogout}><Icon name="logout" size={16}/>Log out</button></div></section>
  </div>;
}
