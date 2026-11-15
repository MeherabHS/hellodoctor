'use client';
import { useRef, useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { DoctorNotification } from '../lib/contracts';
import { State } from './Common';
import { Icon } from './Icons';
import { date, time } from './Queue';

const openable = ['DAILY_CONSULTATION_SUMMARY', 'CONSULTATION_REMINDER_15M', 'EARNING_CREDITED', 'PAYOUT_PAID', 'REGISTRATION_VERIFIED', 'REGISTRATION_REJECTED', 'ARTICLE_STATUS_CHANGED'];
export function Notifications({ onChanged, onOpen }: { onChanged: () => void; onOpen: (notice: DoctorNotification) => void }) {
  const inbox = useResource<DoctorNotification[]>('notifications?limit=25&offset=0');
  const [busy, setBusy] = useState(''); const [error, setError] = useState(''); const [more, setMore] = useState(true); const lock = useRef(false);
  async function mark(notice?: DoctorNotification, open = false) {
    if (lock.current) return; lock.current = true; setBusy(notice?.id ?? 'all'); setError('');
    try { if (!notice || !notice.read_at) await api(notice ? `notifications/${notice.id}/read` : 'notifications/read-all', { method: 'POST' }); await inbox.refresh(); setMore(true); onChanged(); if (notice && open) onOpen(notice); }
    catch (e) { setError(errorText(e)); } finally { lock.current = false; setBusy(''); }
  }
  async function loadMore() {
    if (lock.current || !inbox.data) return; lock.current = true; setBusy('more'); setError('');
    try { const next = await api<DoctorNotification[]>(`notifications?limit=25&offset=${inbox.data.length}`); inbox.setData([...inbox.data, ...next]); setMore(next.length === 25); }
    catch (e) { setError(errorText(e)); } finally { lock.current = false; setBusy(''); }
  }
  const unread = inbox.data?.filter(n => !n.read_at).length ?? 0;
  return <section aria-label="Notification inbox" className="am-stack" style={{ gap: 6 }}>
    <div className="am-row between"><span className="am-faint">{unread ? `${unread} unread` : 'You are all caught up'}</span><div className="am-row"><button className="am-btn ghost sm" disabled={!!busy} aria-label="Refresh notifications" onClick={() => { setMore(true); void inbox.refresh(); onChanged(); }}><Icon name="refresh" size={15}/></button><button className="am-btn soft sm" disabled={!!busy || !unread} onClick={() => void mark()}><Icon name="check" size={15}/>Mark all read</button></div></div>
    <State loading={inbox.loading && !inbox.data} error={inbox.error || error} empty={!inbox.loading && !inbox.error && !inbox.data?.length ? 'No notifications yet' : undefined} retry={() => void inbox.refresh()} icon="bell"/>
    {inbox.data?.map(notice => <article className={`am-notice${notice.read_at ? '' : ' unread'}`} key={notice.id} data-notification-id={notice.id}>
      <span className="am-notice-dot" aria-hidden="true"/>
      <div className="am-grow">
        <strong className="am-strong" style={{ display: 'block' }}>{notice.title}</strong>
        <p className="am-pre am-muted" style={{ fontSize: 13.5, marginTop: 2 }}>{notice.body}</p>
        <div className="am-row between" style={{ marginTop: 8 }}><time className="am-faint">{date(notice.created_at)} · {time(notice.created_at)}</time>
          <div className="am-row" style={{ gap: 6 }}>{!notice.read_at && <button className="am-btn ghost sm" disabled={!!busy} onClick={() => void mark(notice)}>{busy === notice.id ? 'Marking…' : 'Mark read'}</button>}{openable.includes(notice.notification_type) && <button className="am-btn soft sm" disabled={!!busy} onClick={() => void mark(notice, true)}>Open</button>}</div></div>
      </div>
    </article>)}
    {more && (inbox.data?.length ?? 0) >= 25 && <button className="am-btn ghost" disabled={!!busy} onClick={() => void loadMore()}>{busy === 'more' ? 'Loading…' : 'Load more'}</button>}
  </section>;
}
