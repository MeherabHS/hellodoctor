'use client';
import { isProfessionallyVerified, professionalVerificationLabel } from '../lib/professionalVerification';
import { useRef, useState } from 'react';
import { api, ApiError, errorText } from '../lib/client';
import { Profile } from '../lib/contracts';
import { State } from './Common';
import { initials } from './Queue';
import { DeferredFinancialNotice, PayoutSettings } from './Wallet';

export type SettingsSection='profile'|'professional'|'availability'|'payout'|'security';
export function Settings({profile,section,onSection,onUpdated,duty,onDuty,onWallet,onLogout,paymentRequired=true}:{profile:Profile;section:SettingsSection;onSection:(section:SettingsSection)=>void;onUpdated:(profile:Profile)=>void;duty:{value?:boolean;busy:boolean;error:string};onDuty:()=>void;onWallet:()=>void;onLogout:()=>void;paymentRequired?:boolean}) {
  const [name,setName]=useState(profile.full_name);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [fieldError,setFieldError]=useState('');
  const [done,setDone]=useState(false);
  const lock=useRef(false);
  const sections:[SettingsSection,string][]=[['profile','Profile'],['professional','Professional Details'],['availability','Availability'],['payout','Payout'],['security','Security']];
  async function save(event:React.FormEvent){
    event.preventDefault();if(lock.current)return;
    setError('');setFieldError('');setDone(false);
    if(!name.trim()||[...name].length>150){setFieldError('Enter a display name of 1–150 characters.');return;}
    lock.current=true;setBusy(true);
    try {await api('doctor/profile',{method:'PATCH',body:JSON.stringify({full_name:name.trim()})});const updated=await api<Profile>('doctor/profile');onUpdated(updated);setDone(true);}
    catch(e){if(e instanceof ApiError&&[400,422].includes(e.status))setFieldError(errorText(e));else setError(errorText(e));}
    finally{lock.current=false;setBusy(false);}
  }
  return <section id="webSettingsWorkspace" className="dw-page-view active">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>Settings</h2><p>Your profile, professional details and account preferences</p></div></header>
    <div className="web-settings-layout">
      <nav className="web-settings-nav" aria-label="Settings sections">{sections.map(([key,label])=><button key={key} className={`dw-nav-item ${section===key?'active':''}`} aria-current={section===key?'page':undefined} aria-controls={`settings-${key}`} onClick={()=>onSection(key)}>{label}</button>)}</nav>
      <div className="web-settings-content">
        <section id="settings-profile" className="dw-section-card" hidden={section!=='profile'} aria-label="Profile settings"><div className="dw-section-header"><h3 className="dw-section-title">Profile</h3></div><div className="web-section-body">
          <div className="web-profile-identity"><div className="dw-doc-avatar-img">{initials(profile.full_name)}</div><div><strong>{profile.full_name}</strong><p className="sheet-desc">{profile.primary_specialty}</p></div></div>
          <form className="web-form" onSubmit={event=>void save(event)}><label htmlFor="settingsDisplayName">Display name</label><input id="settingsDisplayName" required maxLength={150} value={name} disabled={busy} aria-invalid={!!fieldError} aria-describedby={fieldError?'settingsNameError':'settingsNameHelp'} onChange={e=>{setName(e.target.value);setDone(false);setFieldError('');}}/><p id="settingsNameHelp" className="sheet-desc">Your display name is editable. Verified professional details are managed separately.</p>{fieldError&&<p id="settingsNameError" role="alert" className="web-field-error">{fieldError}</p>}<State error={error}/>{done&&<p role="status" className="web-save-confirmation">Profile updated</p>}<div className="web-action-row"><button className="dw-btn-primary" type="submit" disabled={busy}>{busy?'Saving…':'Save changes'}</button></div></form>
        </div></section>
        <section id="settings-professional" className="dw-section-card" hidden={section!=='professional'} aria-label="Professional details"><div className="dw-section-header"><h3 className="dw-section-title">Professional Details</h3><span className={`badge ${isProfessionallyVerified(profile)?'badge-green':'badge-slate'}`}>{professionalVerificationLabel(profile)}</span></div><div className="web-section-body">
          <div className="web-profile-identity"><div className="dw-doc-avatar-img">{initials(profile.full_name)}</div><div><strong>{profile.full_name}</strong><p className="sheet-desc">{profile.primary_specialty}</p></div></div>
          <dl className="web-profile-details"><dt>Regulatory authority</dt><dd>Kenya Medical Practitioners and Dentists Council (KMPDC)</dd><dt>Country</dt><dd>Kenya ({profile.license_country})</dd><dt>KMPDC Registration Number</dt><dd>{profile.license_number}</dd><dt>Hospital / institution</dt><dd>{profile.current_hospital||'Not provided'}</dd><dt>Qualifications</dt><dd>{profile.qualifications.join(', ')||'Not provided'}</dd></dl><p className="sheet-desc">Read-only professional information. Credential and verification changes require clinical administration review.</p>
        </div></section>
        <section id="settings-availability" className="dw-section-card" hidden={section!=='availability'} aria-label="Availability settings"><div className="dw-section-header"><h3 className="dw-section-title">Availability</h3></div><div className="web-section-body"><div className="web-action-row"><strong>Duty status</strong><button className="web-duty" role="switch" aria-label="On duty" aria-checked={duty.value??false} disabled={duty.busy||duty.value===undefined} onClick={onDuty}>{duty.busy?'Updating…':duty.value?'On duty':'Off duty'}</button></div><p className="sheet-desc">Set your operational duty status. Bookable consultation times are managed in Appointments.</p><State error={duty.error}/></div></section>
        <section id="settings-payout" className="dw-section-card" hidden={section!=='payout'} aria-label="Payout settings"><div className="dw-section-header"><h3 className="dw-section-title">Payout</h3>{paymentRequired&&<button className="dw-btn-outline-sm" onClick={onWallet}>View Wallet</button>}</div><div className="web-section-body">{section==='payout'&&(paymentRequired?<PayoutSettings/>:<DeferredFinancialNotice/>)}</div></section>
        <section id="settings-security" className="dw-section-card" hidden={section!=='security'} aria-label="Security settings"><div className="dw-section-header"><h3 className="dw-section-title">Security</h3></div><div className="web-section-body"><strong>Current session</strong><p className="sheet-desc">Sign out of AfyaMkononi on this browser when you finish your work.</p><div className="web-action-row"><button className="dw-btn-outline" onClick={onLogout}>Log out</button></div></div></section>
      </div>
    </div>
  </section>;
}
