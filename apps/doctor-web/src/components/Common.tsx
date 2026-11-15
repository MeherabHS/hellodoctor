'use client';
import { useEffect, useRef } from 'react';
import { Icon, type IconName } from './Icons';
import type { Appointment } from '../lib/contracts';

export function State({ loading, error, empty, retry, compact, icon='calendar', hint }: { loading?: boolean; error?: string; empty?: string; retry?: () => void; compact?: boolean; icon?: IconName; hint?: string }) {
  if (!loading && !error && !empty) return null;
  return <div className={`web-state${error ? ' error' : ''}${compact ? ' compact' : ''}`} role={error ? 'alert' : 'status'} aria-live="polite">
    {loading ? <span data-testid="web-state-spinner" className="web-state-spinner" aria-hidden="true"/> : <span className="am-state-icon" aria-hidden="true"><Icon name={error ? 'alert' : icon} size={26}/></span>}
    <strong>{loading ? 'Loading…' : error || empty}</strong>
    {!loading && !error && hint && <p className="am-faint">{hint}</p>}
    {error && retry && <button className="am-btn ghost sm" onClick={retry}><Icon name="refresh" size={15}/>Retry</button>}
  </div>;
}

function useDialogFocus(onClose: () => void) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const root = ref.current;
    root?.querySelector<HTMLElement>('input,textarea,select,button:not(.am-close),[tabindex="0"]')?.focus();
    function handle(event: KeyboardEvent) {
      const dialogs = document.querySelectorAll('[aria-modal="true"]');
      if (root !== dialogs[dialogs.length - 1]) return;
      if (event.key === 'Escape') onClose();
      if (event.key !== 'Tab' || !root) return;
      const list = [...root.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),a[href],[tabindex="0"]')].filter(el => el.getClientRects().length);
      const first = list[0], last = list[list.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
    document.addEventListener('keydown', handle);
    return () => { document.removeEventListener('keydown', handle); previous?.focus(); };
  }, [onClose]);
  return ref;
}

export function Modal({ title, children, onClose, wide, bare }: { title: string; children: React.ReactNode; onClose: () => void; wide?: boolean; bare?: boolean }) {
  const ref = useDialogFocus(onClose);
  return <div className="am-modal" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <div ref={ref} className={`am-dialog${wide ? ' wide' : ''}`} role="dialog" aria-modal="true" aria-label={title}>
      {bare ? <div className="am-row end" style={{ padding: '14px 14px 0' }}><button className="am-close" aria-label="Close dialog" onClick={onClose}><Icon name="close"/></button></div>
        : <div className="am-dialog-head"><h2>{title}</h2><button className="am-close" aria-label="Close dialog" onClick={onClose}><Icon name="close"/></button></div>}
      <div className="am-dialog-body">{children}</div>
    </div>
  </div>;
}

export function Drawer({ title, subtitle, children, footer, onClose }: { title: string; subtitle?: React.ReactNode; children: React.ReactNode; footer?: React.ReactNode; onClose: () => void }) {
  const ref = useDialogFocus(onClose);
  return <div className="am-drawer-wrap" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <div ref={ref} className="am-drawer" role="dialog" aria-modal="true" aria-label={title}>
      <div className="am-dialog-head"><div><h2>{title}</h2>{subtitle && <p className="am-faint">{subtitle}</p>}</div><button className="am-close" aria-label="Close panel" onClick={onClose}><Icon name="close"/></button></div>
      <div className="am-dialog-body">{children}</div>
      {footer && <div className="am-drawer-foot">{footer}</div>}
    </div>
  </div>;
}

export function PageHeader({ title, subtitle, back, children }: { title: string; subtitle?: React.ReactNode; back?: { label: string; onClick: () => void }; children?: React.ReactNode }) {
  return <header className="am-page-head">
    <div>{back && <button className="am-crumb" onClick={back.onClick}><Icon name="arrowLeft" size={15}/>{back.label}</button>}<h1>{title}</h1>{subtitle && <p>{subtitle}</p>}</div>
    {children && <div className="am-row">{children}</div>}
  </header>;
}

const tones = ['', 't1', 't2', 't3', 't4'];
export function Avatar({ name, size, src }: { name: string; size?: 'sm' | 'lg' | 'xl'; src?: string }) {
  const tone = tones[[...name].reduce((sum, ch) => sum + ch.charCodeAt(0), 0) % tones.length];
  return <span className={`am-avatar ${size ?? ''} ${tone}`} aria-hidden="true">{src ? <img src={src} alt=""/> : initials(name)}</span>;
}
export function initials(name: string) { return name.trim().split(/\s+/).slice(0, 2).map(x => x[0]).join('').toUpperCase() || '?'; }

export function Stat({ icon, tone, label, value, sub }: { icon: IconName; tone?: 'teal' | 'amber' | 'violet' | 'red'; label: string; value: React.ReactNode; sub?: React.ReactNode }) {
  return <section className="am-card am-stat" aria-label={label}>
    <span className={`am-stat-icon ${tone ?? ''}`}><Icon name={icon} size={22}/></span>
    <div className="am-grow"><div className="am-stat-label">{label}</div><div className="am-stat-value">{value}</div>{sub && <div className="am-stat-sub">{sub}</div>}</div>
  </section>;
}

export type Tone = 'green' | 'amber' | 'red' | 'blue' | 'violet' | 'teal' | 'slate';
export function Badge({ tone = 'slate', dot, children }: { tone?: Tone; dot?: boolean; children: React.ReactNode }) {
  return <span className={`am-badge ${tone}`}>{dot && <i className="am-dot"/>}{children}</span>;
}

/** Shared appointment status presentation. Labels are display only; logic stays on server fields. */
export function appointmentStatus(a: Pick<Appointment, 'status' | 'doctor_lifecycle'>): { label: string; tone: Tone } {
  switch (a.status) {
    case 'IN_CONSULTATION': return { label: 'In progress', tone: 'green' };
    case 'WAITING': return { label: 'Patient waiting', tone: 'teal' };
    case 'CONFIRMED': return { label: 'Scheduled', tone: 'blue' };
    case 'COMPLETED': return { label: 'Completed', tone: 'green' };
    case 'CANCELLED': return { label: 'Cancelled', tone: 'amber' };
    case 'REFUNDED': return { label: 'Refunded', tone: 'amber' };
    case 'EXPIRED': return { label: 'Expired', tone: 'red' };
    case 'PREMATURE_TERMINATION': return { label: 'Ended early', tone: 'red' };
    case 'DISPUTED': return { label: 'Disputed', tone: 'violet' };
    case 'PENDING_PAYMENT': return { label: 'Awaiting payment', tone: 'slate' };
    default: return { label: a.status.replaceAll('_', ' ').toLowerCase(), tone: 'slate' };
  }
}
export function sexLabel(value?: string | null) { return value === 'MALE' ? 'Male' : value === 'FEMALE' ? 'Female' : value === 'OTHER' ? 'Other' : null; }
export function demographics(a: { patient_age?: number | null; patient_sex?: string | null }) {
  return [sexLabel(a.patient_sex), typeof a.patient_age === 'number' ? `${a.patient_age} yrs` : null].filter(Boolean).join(', ');
}
export function outcomeLabel(value?: string | null) {
  return ({ COMPLETED_WITH_RX: 'Completed with prescription', COMPLETED_NO_RX: 'Completed, no prescription', REFERRED: 'Referred', ESCALATED: 'Escalated' } as Record<string, string>)[value ?? ''] ?? null;
}
