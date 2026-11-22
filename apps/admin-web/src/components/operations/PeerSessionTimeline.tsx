'use client';
import {useEffect, useState} from 'react';
import {X} from 'lucide-react';
import {tr} from '@/lib/i18n';
import {apiErrorMessage} from '@/lib/i18n/apiErrors';
import {request} from '../../lib/api';
import {Chip} from '../ui/afya';

/** What `GET /admin/peer-sessions/:id/timeline` returns: who acted and when, never content. */
export type PeerTimelineData = {request_id: string; retention_days: number; events: {at: string; event: string; actor: 'PATIENT' | 'PEER' | 'SYSTEM' | string}[]};

const words = (code: string) => code.toLowerCase().replaceAll('_', ' ').replace(/^\w/, c => c.toUpperCase());
const at = (value: string) => new Date(value).toLocaleString('en-KE', {timeZone: 'Africa/Nairobi', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit'});
const actorLabel = (actor: string) => actor === 'PATIENT' ? tr('ui2.patient') : actor === 'PEER' ? tr('ops.peer') : tr('ui2.system');

/** One peer session's timeline in a panel over the dashboard. */
export function PeerSessionTimeline({path, onClose}: {path: string; onClose: () => void}) {
  const [data, setData] = useState<PeerTimelineData | null>(null);
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    request<PeerTimelineData>(path).then(d => { if (active) setData(d); }).catch(e => { if (active) setError(apiErrorMessage(e, tr('ui.unableToLoadTheTimeline'))); });
    return () => { active = false; };
  }, [path]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [onClose]);
  return <div className="admin-modal-backdrop" role="presentation" onClick={onClose}>
    <section role="dialog" aria-modal="true" aria-labelledby="peer-timeline-title" className="admin-modal-card afya-timeline-panel" onClick={e => e.stopPropagation()}>
      <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 12}}>
        <div><h2 id="peer-timeline-title" className="afya-modal-title">{tr('ops.peerTimeline')}</h2><p className="afya-modal-sub">{tr('ops.peerTimelineSub')}</p></div>
        <button type="button" className="afya-icon-btn" aria-label={tr('ui.closeTimeline')} onClick={onClose}><X size={18}/></button>
      </div>
      {error && <div role="alert" className="afya-login-error" style={{marginTop: 12}}>{error}</div>}
      {!data && !error && <p className="afya-stat-note">{tr('ui.loading')}</p>}
      {data && <>
        <h3 className="afya-timeline-heading">{tr('ui.events')}</h3>
        <ol className="afya-timeline-list">{data.events.map((e, i) => <li key={i}>
          <span className="afya-timeline-time">{at(e.at)}</span>
          <span>{actorLabel(e.actor)} · {words(e.event)}</span>
          <Chip tone={e.actor === 'PEER' ? 'purple' : e.actor === 'PATIENT' ? 'info' : 'outlined'}>{actorLabel(e.actor)}</Chip>
        </li>)}</ol>
      </>}
    </section>
  </div>;
}
