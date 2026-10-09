'use client';
import { isProfessionallyVerified, professionalVerificationLabel } from '../lib/professionalVerification';
import { useEffect, useRef } from 'react';
import { Profile } from '../lib/contracts';
import { initials } from './Queue';

export function AccountMenu({profile,onProfile,onSettings,onNotifications,onLogout}:{profile:Profile;onProfile:()=>void;onSettings:()=>void;onNotifications:()=>void;onLogout:()=>void}) {
  const ref=useRef<HTMLDetailsElement>(null);
  useEffect(()=>{const close=(event:PointerEvent)=>{if(ref.current&&!ref.current.contains(event.target as Node))ref.current.open=false;};document.addEventListener('pointerdown',close);return()=>document.removeEventListener('pointerdown',close);},[]);
  function choose(action:()=>void){if(ref.current)ref.current.open=false;action();}
  function key(event:React.KeyboardEvent<HTMLDetailsElement>){
    const root=event.currentTarget;
    const items=[...root.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
    const index=items.indexOf(document.activeElement as HTMLButtonElement);
    if(event.key==='Escape'){event.preventDefault();root.open=false;root.querySelector('summary')?.focus();}
    else if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){
      event.preventDefault();root.open=true;
      const next=event.key==='Home'?0:event.key==='End'?items.length-1:event.key==='ArrowDown'?(index+1)%items.length:(index<0?items.length-1:(index-1+items.length)%items.length);
      items[next]?.focus();
    }
  }
  return <details ref={ref} className="web-account-menu" onKeyDown={key} onBlur={event=>{if(!event.currentTarget.contains(event.relatedTarget as Node|null))event.currentTarget.open=false;}}>
    <summary className="dw-user-badge" aria-label="Open account menu" aria-haspopup="menu" aria-controls="doctorAccountMenu"><span className="dw-user-avatar">{initials(profile.full_name)}</span><span className="dw-user-name">{profile.full_name}</span><span aria-hidden="true">▾</span></summary>
    <div id="doctorAccountMenu" className="web-tools-list" role="menu" aria-label="Doctor account"><div className="web-account-identity"><strong>{profile.full_name}</strong><p>{profile.primary_specialty}</p><span className={`badge ${isProfessionallyVerified(profile)?'badge-green':'badge-slate'}`}>{professionalVerificationLabel(profile)}</span></div><button role="menuitem" className="web-tools-item" onClick={()=>choose(onProfile)}>View profile</button><button role="menuitem" className="web-tools-item" onClick={()=>choose(onSettings)}>Settings</button><button role="menuitem" className="web-tools-item" onClick={()=>choose(onNotifications)}>Notifications</button><div className="web-account-separator"/><button role="menuitem" className="web-tools-item" onClick={()=>choose(onLogout)}>Log out</button></div>
  </details>;
}
