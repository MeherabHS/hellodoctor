'use client';
import { tr } from '@/lib/i18n';
import {useEffect, useState, type CSSProperties, type ReactNode} from 'react';
import {adminFetch} from '../../lib/adminQueue.mjs';
import type {LucideIcon} from 'lucide-react';
import {Search} from 'lucide-react';

/** Shared building blocks for the AfyaMkononi Admin design (docs/admin-ui/ADMIN_REDESIGN_PLAN.md). */

export type Tone = 'green' | 'blue' | 'teal' | 'amber' | 'red' | 'purple' | 'neutral';
export type ChipTone = 'success' | 'warn' | 'info' | 'danger' | 'neutral' | 'purple' | 'topic' | 'outlined';

export function PageHead({title, subtitle, actions}: {title: string; subtitle?: ReactNode; actions?: ReactNode}) {
  return (
    <div className="afya-page-head">
      <div>
        <h2>{title}</h2>
        {subtitle && <p>{subtitle}</p>}
      </div>
      {actions && <div style={{display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap'}}>{actions}</div>}
    </div>
  );
}

export function StatTile({icon: Icon, tone, value, label, note}: {icon: LucideIcon; tone: Tone; value: ReactNode; label: string; note?: ReactNode}) {
  return (
    <div className="afya-stat">
      <div className={`afya-stat-icon afya-tone-${tone}`}><Icon size={22} aria-hidden /></div>
      <div style={{minWidth: 0}}>
        <div className="afya-stat-value">{value}</div>
        <div className="afya-stat-label">{label}</div>
        {note && <div className="afya-stat-note">{note}</div>}
      </div>
    </div>
  );
}

export function Chip({tone, children, title}: {tone: ChipTone; children: ReactNode; title?: string}) {
  return <span className={`afya-chip ${tone}`} title={title}>{children}</span>;
}

export function Card({title, subtitle, actions, children, padded = true}: {title?: ReactNode; subtitle?: ReactNode; actions?: ReactNode; children: ReactNode; padded?: boolean}) {
  return (
    <section className="admin-card" style={padded ? undefined : {padding: 0}}>
      {(title || actions) && (
        <div className="admin-card-header" style={padded ? undefined : {padding: '20px 22px 0'}}>
          <div>
            {title && <div className="admin-card-title">{title}</div>}
            {subtitle && <div className="admin-card-subtitle">{subtitle}</div>}
          </div>
          {actions && <div style={{display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap'}}>{actions}</div>}
        </div>
      )}
      {children}
    </section>
  );
}

export interface TabItem<T extends string> { id: T; label: string; count?: number }
export function Tabs<T extends string>({items, value, onChange, label}: {items: TabItem<T>[]; value: T; onChange: (id: T) => void; label: string}) {
  return (
    <div className="afya-tabs" role="tablist" aria-label={label}>
      {items.map(item => (
        <button
          key={item.id}
          type="button"
          role="tab"
          aria-selected={value === item.id}
          className={`afya-tab ${value === item.id ? 'active' : ''}`}
          onClick={() => onChange(item.id)}
        >
          {item.label}
          {item.count !== undefined && <span className="count">({item.count})</span>}
        </button>
      ))}
    </div>
  );
}

export function SearchField({value, onChange, placeholder, label, maxLength = 160}: {value: string; onChange: (v: string) => void; placeholder: string; label: string; maxLength?: number}) {
  return (
    <label className="afya-search">
      <Search size={16} color="var(--afya-faint)" aria-hidden />
      <input type="search" aria-label={label} placeholder={placeholder} value={value} maxLength={maxLength} onChange={e => onChange(e.target.value)} />
    </label>
  );
}

export function Pagination({offset, pageSize, total, busy, onChange}: {offset: number; pageSize: number; total: number; busy?: boolean; onChange: (offset: number) => void}) {
  const pages = Math.max(1, Math.ceil(total / pageSize));
  const current = Math.floor(offset / pageSize) + 1;
  // A window of up to five page numbers around the current page.
  const start = Math.max(1, Math.min(current - 2, pages - 4));
  const numbers = Array.from({length: Math.min(5, pages)}, (_, i) => start + i);
  return (
    <div className="afya-pagination">
      <span>{tr('mix.showingRange', { from: total ? offset + 1 : 0, to: Math.min(offset + pageSize, total), total })}</span>
      <div className="pages">
        <button type="button" className="afya-page-btn" aria-label={tr('ui.previousPage')} disabled={busy || current === 1} onClick={() => onChange(offset - pageSize)}>‹</button>
        {numbers.map(n => (
          <button key={n} type="button" className={`afya-page-btn ${n === current ? 'active' : ''}`} aria-current={n === current ? 'page' : undefined} disabled={busy} onClick={() => onChange((n - 1) * pageSize)}>{n}</button>
        ))}
        <button type="button" className="afya-page-btn" aria-label={tr('ui.nextPage')} disabled={busy || current >= pages} onClick={() => onChange(offset + pageSize)}>›</button>
      </div>
    </div>
  );
}

export function EmptyState({title, children}: {title: string; children?: ReactNode}) {
  return (
    <div className="afya-empty" role="status">
      <strong>{title}</strong>
      {children}
    </div>
  );
}

export function Notice({tone = 'warn', title, children}: {tone?: 'warn' | 'info'; title: string; children?: ReactNode}) {
  return (
    <div className={`afya-notice ${tone === 'info' ? 'info' : ''}`} role="status">
      <div><strong>{title}</strong>{children}</div>
    </div>
  );
}

/**
 * An authenticated Admin media image (article cover, facility photo) loaded
 * through the admin request queue, so a list of thumbnails cannot exceed the
 * backend's two-request admin lane. Shown as a data URL (the CSP allows data:,
 * not blob:). Renders `fallback` while loading and when it fails.
 */
export function QueuedImage({src, alt, style, fallback}: {src: string; alt: string; style: CSSProperties; fallback: ReactNode}) {
  // Result is keyed by the source it belongs to, so a new src shows the
  // fallback until its own image arrives.
  const [result, setResult] = useState<{for: string; data: string | null; failed: boolean}>({for: '', data: null, failed: false});
  const data = result.for === src ? result.data : null;
  const failed = result.for === src && result.failed;
  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const response = await adminFetch(src, {credentials: 'include', cache: 'no-store'});
        if (!response.ok || !(response.headers.get('Content-Type') ?? '').startsWith('image/')) throw new Error('image');
        const blob = await response.blob();
        const url = await new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(String(reader.result));
          reader.onerror = () => reject(reader.error);
          reader.readAsDataURL(blob);
        });
        if (active) setResult({for: src, data: url, failed: false});
      } catch {
        if (active) setResult({for: src, data: null, failed: true});
      }
    })();
    return () => { active = false; };
  }, [src]);
  // eslint-disable-next-line @next/next/no-img-element -- data URL of an authenticated same-origin image
  return data && !failed ? <img src={data} alt={alt} style={style} /> : <>{fallback}</>;
}

/** EAT date for tables, e.g. "Nov 13, 2026". */
export function eatDate(value: string | null | undefined): string {
  if (!value) return '—';
  const d = new Date(value);
  return Number.isFinite(d.getTime()) ? d.toLocaleDateString('en-US', {timeZone: 'Africa/Nairobi', month: 'short', day: 'numeric', year: 'numeric'}) : '—';
}
