'use client';
import { useEffect, useRef } from 'react';
import { Icon, type IconName } from './Icons';
import { Avatar } from './Common';
import { Profile } from '../lib/contracts';
import { isProfessionallyVerified, professionalVerificationLabel } from '../lib/professionalVerification';

export type Page = 'dashboard' | 'appointments' | 'schedule' | 'telehealth' | 'rx' | 'consultation-history' | 'patients' | 'chat' | 'qna' | 'articles' | 'wallet' | 'settings';
type NavItem = { page: Page; label: string; icon: IconName; count?: number };

export function Shell({ profile, photo, page, go, navOpen, setNavOpen, search, onSearch, duty, onDuty, unread, onNotifications, onProfile, onSettings, onLogout, paymentRequired, liveCount, openQuestions, offline, children }: {
  profile: Profile; photo?: string; page: Page; go: (page: Page) => void; navOpen: boolean; setNavOpen: (open: boolean) => void;
  search: string; onSearch: (value: string) => void; duty: { value?: boolean; busy: boolean; error: string }; onDuty: () => void;
  unread?: number; onNotifications: () => void; onProfile: () => void; onSettings: () => void; onLogout: () => void;
  paymentRequired: boolean; liveCount: number; openQuestions?: number; offline: boolean; children: React.ReactNode;
}) {
  const groups: { label: string; items: NavItem[] }[] = [
    { label: 'Overview', items: [{ page: 'dashboard', label: 'Dashboard', icon: 'dashboard' }] },
    { label: 'Consultations', items: [
      { page: 'appointments', label: 'Appointments', icon: 'calendar', count: liveCount || undefined },
      { page: 'schedule', label: 'Schedule & Slots', icon: 'calendarPlus' },
      { page: 'telehealth', label: 'Consultation Room', icon: 'video' },
      { page: 'consultation-history', label: 'Consultation History', icon: 'history' },
    ] },
    { label: 'Patient care', items: [
      { page: 'patients', label: 'My Patients', icon: 'users' },
      { page: 'chat', label: 'Patient Chats', icon: 'chat' },
    ] },
    { label: 'Knowledge', items: [
      { page: 'qna', label: 'Health Q&A', icon: 'help', count: openQuestions || undefined },
      { page: 'articles', label: 'My Articles', icon: 'book' },
    ] },
    { label: 'Account', items: [
      ...(paymentRequired ? [{ page: 'wallet' as Page, label: 'Earnings', icon: 'wallet' as IconName }] : []),
      { page: 'settings', label: 'Settings', icon: 'settings' },
    ] },
  ];
  const sidebar = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!navOpen) return;
    const previous = document.activeElement as HTMLElement | null;
    sidebar.current?.querySelector<HTMLButtonElement>('button')?.focus();
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape') setNavOpen(false); };
    document.addEventListener('keydown', key);
    return () => { document.removeEventListener('keydown', key); previous?.focus(); };
  }, [navOpen, setNavOpen]);
  const verified = isProfessionallyVerified(profile);
  const selected = page === 'rx' ? 'telehealth' : page;
  return <div className={`am-shell${navOpen ? ' nav-open' : ''}`}>
    <aside ref={sidebar} id="doctorWebSidebar" className="am-sidebar" aria-label="Doctor navigation">
      <div className="am-brand"><span className="am-brand-mark"><img src="/brand/afya_logo.png" alt=""/></span><span><span className="am-brand-name">AfyaMkononi</span><br/><span className="am-brand-sub">Doctor Portal</span></span></div>
      <button className="am-doc-card" style={{ textAlign: 'left', color: 'inherit', font: 'inherit', cursor: 'pointer' }} onClick={onProfile} aria-label="Open professional profile">
        <Avatar name={profile.full_name} src={photo}/>
        <span style={{ minWidth: 0 }}><strong>{profile.full_name}</strong><span>{profile.primary_specialty}</span><br/>{verified ? <span className="am-verified"><Icon name="checkCircle" size={13}/>Verified · KMPDC</span> : <span className="am-verified" style={{ color: '#fde68a' }}>{professionalVerificationLabel(profile)}</span>}</span>
      </button>
      <nav aria-label="Primary">
        {groups.filter(g => g.items.length).map(group => <div key={group.label}>
          <div className="am-nav-label">{group.label}</div>
          {group.items.map(item => <button key={item.page} className="am-nav-item" data-page={item.page} aria-current={selected === item.page ? 'page' : undefined} onClick={() => go(item.page)}>
            <Icon name={item.icon} size={19}/><span>{item.label}</span>{item.count ? <span className="am-nav-count" aria-label={`${item.count} items`}>{item.count > 99 ? '99+' : item.count}</span> : null}
          </button>)}
        </div>)}
      </nav>
      <div className="am-sidebar-foot">All times in East Africa Time (UTC+3).<br/>Patient information is confidential.</div>
    </aside>
    {navOpen && <button className="am-backdrop" aria-label="Close navigation" onClick={() => setNavOpen(false)}/>}
    <div className="am-main">
      {offline && <div className="am-offline" role="status"><Icon name="alert" size={16}/>You are offline. Reconnect to continue working.</div>}
      <header className="am-topbar">
        <button className="am-icon-btn am-menu-toggle" aria-label={navOpen ? 'Close navigation' : 'Open navigation'} aria-expanded={navOpen} aria-controls="doctorWebSidebar" onClick={() => setNavOpen(!navOpen)}><Icon name="menu"/></button>
        <label className="am-search"><Icon name="search" size={17}/><span className="am-sr">Search your appointments</span><input value={search} placeholder="Search your patients or appointment numbers…" onChange={e => onSearch(e.target.value)}/></label>
        <div className="am-topbar-actions">
          <button className={`am-duty${duty.value ? ' on' : ''}`} role="switch" aria-checked={duty.value ?? false} aria-label="On duty" disabled={duty.busy || duty.value === undefined} onClick={onDuty} title={duty.error || undefined}>
            <span className="am-dot"/><span className="am-duty-text">{duty.busy ? 'Updating…' : duty.value ? 'Online & ready' : 'Offline'}</span>
          </button>
          <button className="am-icon-btn" aria-label="Open consultation schedule" title="Schedule" onClick={() => go('schedule')}><Icon name="calendar"/></button>
          <button className="am-icon-btn" aria-label={unread ? `Notifications, ${unread} unread` : 'Notifications'} title="Notifications" onClick={onNotifications}><Icon name="bell"/>{!!unread && <span className="am-badge-count">{unread > 99 ? '99+' : unread}</span>}</button>
          <AccountMenu profile={profile} photo={photo} onProfile={onProfile} onSettings={onSettings} onNotifications={onNotifications} onLogout={onLogout}/>
        </div>
      </header>
      <main className="am-content" data-web-screen={page}>{children}</main>
    </div>
  </div>;
}

function AccountMenu({ profile, photo, onProfile, onSettings, onNotifications, onLogout }: { profile: Profile; photo?: string; onProfile: () => void; onSettings: () => void; onNotifications: () => void; onLogout: () => void }) {
  const ref = useRef<HTMLDetailsElement>(null);
  useEffect(() => { const close = (event: PointerEvent) => { if (ref.current && !ref.current.contains(event.target as Node)) ref.current.open = false; }; document.addEventListener('pointerdown', close); return () => document.removeEventListener('pointerdown', close); }, []);
  function choose(action: () => void) { if (ref.current) ref.current.open = false; action(); }
  function key(event: React.KeyboardEvent<HTMLDetailsElement>) {
    const root = event.currentTarget;
    const items = [...root.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === 'Escape') { event.preventDefault(); root.open = false; root.querySelector('summary')?.focus(); }
    else if (['ArrowDown', 'ArrowUp'].includes(event.key)) { event.preventDefault(); root.open = true; items[event.key === 'ArrowDown' ? (index + 1) % items.length : (index - 1 + items.length) % items.length]?.focus(); }
  }
  return <details ref={ref} className="am-account" onKeyDown={key}>
    <summary aria-label="Open account menu" aria-haspopup="menu"><Avatar name={profile.full_name} size="sm" src={photo}/><span className="am-account-name">{profile.full_name}</span><Icon name="chevronRight" size={15}/></summary>
    <div className="am-account-menu" role="menu" aria-label="Doctor account">
      <div className="am-account-id"><strong>{profile.full_name}</strong><span>{profile.primary_specialty}</span></div>
      <button role="menuitem" className="am-menu-item" onClick={() => choose(onProfile)}><Icon name="user" size={17}/>Professional profile</button>
      <button role="menuitem" className="am-menu-item" onClick={() => choose(onSettings)}><Icon name="settings" size={17}/>Settings</button>
      <button role="menuitem" className="am-menu-item" onClick={() => choose(onNotifications)}><Icon name="bell" size={17}/>Notifications</button>
      <button role="menuitem" className="am-menu-item danger" onClick={() => choose(onLogout)}><Icon name="logout" size={17}/>Log out</button>
    </div>
  </details>;
}
