'use client';
import {useCallback, useEffect, useRef, useState} from 'react';
import {Bell, X} from 'lucide-react';
import {request} from '../../lib/api';
import {describeNotice, type AdminNotice, type NoticePage, type NoticeView} from '../../lib/notificationText';
import {isNoticeNew, MAX_VIEWED, withViewed} from '../../lib/noticeSeen';

/**
 * Admin notifications (product owner, 2026-11-14): every Admin gets a pop-up
 * when something happens that they should know about: another Admin's
 * action, a new doctor application, a new grievance, a new app incident or a
 * security event. The bell keeps the recent list with an unread count.
 *
 * Polls GET /admin/notifications every 15 s while the tab is visible, using
 * the server's own time as the cursor (no device-clock dependency). The first
 * load fills the list without pop-ups. Items carry no patient details; every
 * value is rendered as text.
 */
const POLL_MS = 15_000;
const POPUP_MS = 9_000;
const MAX_POPUPS = 3;
const SEEN_KEY = 'afya-admin-notifications-seen';

interface Popup {id: string; view: NoticeView}
type Feed = {items: AdminNotice[]; server_time: string};

function readSeen(): number {
  try { return Number(window.localStorage.getItem(SEEN_KEY)) || 0; } catch { return 0; }
}
function writeSeen(value: number) {
  try { window.localStorage.setItem(SEEN_KEY, String(value)); } catch { /* storage unavailable: unread resets on reload */ }
}
// Notices opened one at a time from their pop-up. Opening the bell still marks
// everything up to the newest notice seen; this covers a pop-up's "View", which
// otherwise left that notice counted as new until the bell was opened.
const VIEWED_KEY = 'afya-admin-notifications-viewed';
function readViewed(): string[] {
  try {
    const v: unknown = JSON.parse(window.localStorage.getItem(VIEWED_KEY) ?? '[]');
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').slice(-MAX_VIEWED) : [];
  } catch { return []; }
}
function writeViewed(ids: string[]) {
  try { window.localStorage.setItem(VIEWED_KEY, JSON.stringify(ids.slice(-MAX_VIEWED))); } catch { /* storage unavailable */ }
}
const TIME = new Intl.DateTimeFormat('en-GB', {timeZone: 'Africa/Nairobi', day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', hour12: false});

export function NotificationCenter({pendingApplications, onOpen}: {pendingApplications: number; onOpen: (page: NoticePage) => void}) {
  const [items, setItems] = useState<AdminNotice[]>([]);
  const [popups, setPopups] = useState<Popup[]>([]);
  const [open, setOpen] = useState(false);
  // No items exist until the first fetch, so reading storage here cannot
  // change the server-rendered output.
  const [seen, setSeen] = useState(() => (typeof window === 'undefined' ? 0 : readSeen()));
  const [viewed, setViewed] = useState<string[]>(() => (typeof window === 'undefined' ? [] : readViewed()));
  const cursor = useRef<string | null>(null);
  const known = useRef(new Set<string>());
  const panel = useRef<HTMLDivElement>(null);

  const dismiss = useCallback((id: string) => setPopups(p => p.filter(x => x.id !== id)), []);
  const pop = useCallback((fresh: AdminNotice[]) => {
    const shown = fresh.slice(0, MAX_POPUPS).map(n => ({id: n.id, view: describeNotice(n)}));
    if (fresh.length > MAX_POPUPS) {
      shown.push({id: `more-${fresh[0].id}`, view: {title: `${fresh.length - MAX_POPUPS} more notifications`, body: 'Open the bell to see them all.', page: 'activity', tone: 'info'}});
    }
    setPopups(p => [...shown, ...p].slice(0, MAX_POPUPS + 1));
    for (const s of shown) setTimeout(() => dismiss(s.id), POPUP_MS);
  }, [dismiss]);

  useEffect(() => {
    let active = true;
    async function load() {
      if (cursor.current !== null && document.visibilityState !== 'visible') return;
      try {
        const since = cursor.current;
        const feed = await request<Feed>(`/admin/notifications${since ? `?since=${encodeURIComponent(since)}` : ''}`);
        if (!active) return;
        cursor.current = feed.server_time;
        const fresh = feed.items.filter(i => !known.current.has(i.id));
        for (const i of fresh) known.current.add(i.id);
        if (since === null) { setItems(feed.items); return; }
        if (fresh.length) {
          pop(fresh);
          setItems(prev => [...fresh, ...prev].slice(0, 100));
        }
      } catch { /* the next poll retries; a sign-out is handled by request() */ }
    }
    void load();
    const timer = setInterval(() => void load(), POLL_MS);
    return () => { active = false; clearInterval(timer); };
  }, [pop]);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => { if (panel.current && !panel.current.contains(e.target as Node)) setOpen(false); };
    const esc = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false); };
    document.addEventListener('mousedown', close);
    document.addEventListener('keydown', esc);
    return () => { document.removeEventListener('mousedown', close); document.removeEventListener('keydown', esc); };
  }, [open]);

  const isNew = (i: AdminNotice) => isNoticeNew(i.at, i.id, seen, viewed);
  const unread = items.filter(isNew).length;
  const badge = unread + (pendingApplications > 0 ? 1 : 0);

  function toggle() {
    const next = !open;
    setOpen(next);
    if (next && items.length) {
      const newest = Math.max(...items.map(i => new Date(i.at).getTime()));
      setSeen(newest); writeSeen(newest);
    }
  }
  function go(page: NoticePage, popupId?: string) {
    if (popupId) {
      dismiss(popupId);
      if (items.some(i => i.id === popupId) && !viewed.includes(popupId)) {
        const next = withViewed(viewed, popupId);
        setViewed(next); writeViewed(next);
      }
    }
    setOpen(false);
    onOpen(page);
  }

  return (
    <div className="afya-notify" ref={panel}>
      <button type="button" className="admin-top-icon-btn" title="Notifications" aria-haspopup="true" aria-expanded={open}
        aria-label={badge > 0 ? `Notifications: ${badge} new` : 'Notifications'} onClick={toggle}>
        <Bell size={18} aria-hidden />
        {badge > 0 && <span className="afya-notify-count" aria-hidden>{badge > 99 ? '99+' : badge}</span>}
      </button>

      {open && (
        <div className="afya-notify-panel" role="dialog" aria-label="Notifications">
          <div className="afya-notify-head"><strong>Notifications</strong><span>Last 24 hours · live</span></div>
          {pendingApplications > 0 && (
            <button type="button" className="afya-notify-item tone-warning" onClick={() => go('verification')}>
              <span className="afya-notify-dot" aria-hidden />
              <span><strong>{pendingApplications} doctor application{pendingApplications === 1 ? '' : 's'} waiting</strong><small>Review KMPDC registrations</small></span>
            </button>
          )}
          <div className="afya-notify-list">
            {items.length === 0 && pendingApplications === 0 && <p className="afya-notify-empty">Nothing new. You will see a pop-up here when something happens.</p>}
            {items.map(n => {
              const v = describeNotice(n);
              return (
                <button type="button" key={n.id} className={`afya-notify-item tone-${v.tone}${isNew(n) ? ' is-new' : ''}`} onClick={() => go(v.page)}>
                  <span className="afya-notify-dot" aria-hidden />
                  <span><strong>{v.title}</strong><small>{v.body} · {TIME.format(new Date(n.at))} EAT</small></span>
                </button>
              );
            })}
          </div>
          <button type="button" className="afya-notify-foot" onClick={() => go('activity')}>Open the Activity Log</button>
        </div>
      )}

      <div className="afya-notify-popups" aria-live="polite">
        {popups.map(p => (
          <div key={p.id} className={`afya-notify-popup tone-${p.view.tone}`} role="status">
            <span className="afya-notify-dot" aria-hidden />
            <div><strong>{p.view.title}</strong><span>{p.view.body}</span></div>
            <button type="button" className="afya-notify-view" onClick={() => go(p.view.page, p.id)}>View</button>
            <button type="button" className="afya-notify-close" aria-label="Dismiss" onClick={() => dismiss(p.id)}><X size={14} aria-hidden /></button>
          </div>
        ))}
      </div>
    </div>
  );
}
