'use client';
import { useMemo, useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { Appointment, RecordDto } from '../lib/contracts';
import { Avatar, Badge, PageHeader, State, Stat, appointmentStatus, demographics, outcomeLabel } from './Common';
import { Icon } from './Icons';
import { FILES_ACCESS_ENDED, FileRow, consultationFiles } from './Clinical';
import { ModalityBadge } from './Dashboard';
import { ClinicalAction, date, dayKey, time } from './Queue';

type Filter = 'all' | 'completed' | 'cancelled' | 'other';
const cancelled = ['CANCELLED', 'REFUNDED'];
function bucket(a: Appointment): Filter { return a.status === 'COMPLETED' ? 'completed' : cancelled.includes(a.status) ? 'cancelled' : 'other'; }
function duration(start?: unknown, end?: unknown) {
  if (typeof start !== 'string' || typeof end !== 'string') return null;
  const seconds = Math.max(0, Math.round((Date.parse(end) - Date.parse(start)) / 1000));
  return `${Math.floor(seconds / 60)} min ${seconds % 60} sec`;
}

export function ConsultationHistory({ search, onDocument, action }: { search: string; onDocument: (appointmentId: string, document: RecordDto, current?: string) => void; action: ClinicalAction }) {
  const history = useResource<Appointment[]>('doctor/appointments?view=history&limit=100&offset=0');
  const [more, setMore] = useState(true); const [busy, setBusy] = useState(false); const [error, setError] = useState('');
  const [filter, setFilter] = useState<Filter>('all'); const [query, setQuery] = useState(search); const [from, setFrom] = useState(''); const [to, setTo] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  async function loadMore() { if (busy || !history.data) return; setBusy(true); try { const next = await api<Appointment[]>(`doctor/appointments?view=history&limit=100&offset=${history.data.length}`); history.setData(old => [...(old ?? []), ...next]); setMore(next.length === 100); } catch (e) { setError(errorText(e)); } finally { setBusy(false); } }
  const all = useMemo(() => history.data ?? [], [history.data]);
  const partial = more && all.length >= 100;
  const term = query.trim().toLowerCase();
  const rows = all.filter(a => (filter === 'all' || bucket(a) === filter) && (!term || `${a.patient_display_name} ${a.appointment_number}`.toLowerCase().includes(term)) && (!from || dayKey(a.start_time) >= from) && (!to || dayKey(a.start_time) <= to));
  const counts = { all: all.length, completed: all.filter(a => bucket(a) === 'completed').length, cancelled: all.filter(a => bucket(a) === 'cancelled').length, other: all.filter(a => bucket(a) === 'other').length };
  const shown = (n: number) => history.loading && !history.data ? '—' : `${n}${partial ? '+' : ''}`;
  const selected = all.find(a => a.id === selectedId) ?? null;
  return <>
    <PageHeader title="Consultation history" subtitle="Your past consultations, outcomes, patient uploads and prescriptions.">
      <button className="am-btn ghost" onClick={() => void history.refresh()}><Icon name="refresh" size={16}/>Refresh</button>
    </PageHeader>
    <div className="am-grid stats">
      <Stat icon="stethoscope" label="Total consultations" value={shown(counts.all)} sub={partial ? 'Load more to count older ones' : 'In your history'}/>
      <Stat icon="checkCircle" tone="teal" label="Completed" value={shown(counts.completed)}/>
      <Stat icon="x" tone="amber" label="Cancelled or refunded" value={shown(counts.cancelled)}/>
      <Stat icon="clock" tone="red" label="Expired or ended early" value={shown(counts.other)}/>
    </div>
    <div className={selected ? 'am-grid main-side' : 'am-stack'} style={{ alignItems: 'start' }}>
      <section className="am-card">
        <div className="am-card-head" style={{ alignItems: 'flex-end' }}>
          <div className="am-row" style={{ gap: 10 }}>
            <label className="am-search" style={{ width: 260 }}><Icon name="search" size={16}/><span className="am-sr">Search history</span><input value={query} placeholder="Patient or appointment no." onChange={e => setQuery(e.target.value)}/></label>
            <label className="am-field"><span className="am-faint">From</span><input className="am-input" type="date" value={from} style={{ minHeight: 38, padding: '6px 10px' }} onChange={e => setFrom(e.target.value)}/></label>
            <label className="am-field"><span className="am-faint">To</span><input className="am-input" type="date" value={to} style={{ minHeight: 38, padding: '6px 10px' }} onChange={e => setTo(e.target.value)}/></label>
          </div>
          <div className="am-tabs" aria-label="Status filter">{([['all', 'All'], ['completed', 'Completed'], ['cancelled', 'Cancelled'], ['other', 'Other']] as [Filter, string][]).map(([value, label]) => <button key={value} className="am-tab" aria-pressed={filter === value} onClick={() => setFilter(value)}>{label}<span className="am-tab-count">{counts[value]}</span></button>)}</div>
        </div>
        <State loading={history.loading && !history.data} error={history.error || error} retry={() => void history.refresh()} empty={history.data && !rows.length ? (all.length ? 'No consultations match these filters' : 'No consultation history yet') : undefined} icon="history"/>
        {rows.length > 0 && <div className="am-table-wrap"><table className="am-table">
          <thead><tr><th>Date & time</th><th>Patient</th><th>Type</th><th>Status</th><th>Outcome</th><th aria-label="Open"/></tr></thead>
          <tbody>{rows.map(a => { const status = appointmentStatus(a); return <tr key={a.id} aria-selected={selectedId === a.id} style={{ cursor: 'pointer' }} onClick={() => setSelectedId(a.id)}>
            <td className="nowrap"><strong className="am-strong">{date(a.start_time)}</strong><div className="am-faint">{time(a.start_time)}</div></td>
            <td><div className="am-person"><Avatar name={a.patient_display_name} size="sm"/><div style={{ minWidth: 0 }}><strong>{a.patient_display_name}</strong><span>{[demographics(a), a.appointment_number].filter(Boolean).join(' · ')}</span></div></div></td>
            <td><ModalityBadge modality={a.modality}/></td>
            <td><Badge tone={status.tone} dot>{status.label}</Badge></td>
            <td>{a.clinical_outcome === 'COMPLETED_WITH_RX' ? <Badge tone="red"><Icon name="rx" size={13}/>Rx issued</Badge> : <span className="am-muted">{outcomeLabel(a.clinical_outcome) ?? '—'}</span>}</td>
            <td><button className="am-btn ghost sm" aria-label={`Open consultation with ${a.patient_display_name}`} onClick={e => { e.stopPropagation(); setSelectedId(a.id); }}><Icon name="chevronRight" size={15}/></button></td>
          </tr>; })}</tbody>
        </table></div>}
        {partial && <div className="am-card-body am-row between"><span className="am-faint">Showing the {all.length} most recent consultations.</span><button className="am-btn ghost" disabled={busy} onClick={() => void loadMore()}>{busy ? 'Loading…' : 'Load older consultations'}</button></div>}
      </section>
      {selected && <Detail key={selected.id} a={selected} onClose={() => setSelectedId(null)} onDocument={onDocument} action={action}/>}
    </div>
  </>;
}

function Detail({ a, onClose, onDocument, action }: { a: Appointment; onClose: () => void; onDocument: (appointmentId: string, document: RecordDto, current?: string) => void; action: ClinicalAction }) {
  const [tab, setTab] = useState<'summary' | 'files'>('summary');
  const summary = useResource<RecordDto>(`doctor/consultations/${a.id}/summary`);
  const s = summary.data; const files = consultationFiles(s); const status = appointmentStatus(a);
  return <section className="am-card" style={{ position: 'sticky', top: 'calc(var(--am-topbar-h) + 20px)' }} aria-label="Consultation details">
    <div className="am-card-head"><h2>Consultation details</h2><button className="am-close" aria-label="Close details" onClick={onClose}><Icon name="close"/></button></div>
    <div className="am-card-body am-stack" style={{ gap: 16 }}>
      <div className="am-row between"><div className="am-person"><Avatar name={a.patient_display_name} size="lg"/><div><strong>{a.patient_display_name}</strong><span>{[demographics(a), a.appointment_number].filter(Boolean).join(' · ')}</span></div></div><Badge tone={status.tone} dot>{status.label}</Badge></div>
      <div className="am-grid two" style={{ gap: 10 }}>
        {[['calendar', date(a.start_time), time(a.start_time)], [a.modality === 'CHAT' ? 'chat' : 'video', a.modality === 'CHAT' ? 'Chat' : 'Video', 'Consultation'], ['clock', duration(s?.started_at, s?.ended_at) ?? '—', 'Duration'], ['file', a.appointment_number, 'Appointment']].map(([icon, value, label]) =>
          <div key={label} className="am-file" style={{ padding: 10 }}><span className="am-file-icon" style={{ width: 34, height: 34 }}><Icon name={icon as 'calendar'} size={17}/></span><div style={{ minWidth: 0 }}><strong className="am-strong" style={{ display: 'block', fontSize: 13 }}>{value}</strong><span className="am-faint">{label}</span></div></div>)}
      </div>
    </div>
    <div className="am-underline-tabs" role="tablist"><button role="tab" className="am-underline-tab" aria-selected={tab === 'summary'} onClick={() => setTab('summary')}>Encounter summary</button><button role="tab" className="am-underline-tab" aria-selected={tab === 'files'} onClick={() => setTab('files')}>Files ({files.intake.length + files.rx.length})</button></div>
    <div className="am-card-body am-stack">
      <State compact loading={summary.loading} error={summary.error} retry={() => void summary.refresh()}/>
      {s && tab === 'summary' && <>
        <section><span className="am-label">Reason for consultation</span><p className="am-pre" style={{ marginTop: 4 }}>{String(s.chief_complaint ?? 'The patient did not add a reason.')}</p></section>
        {s.clinical_notes ? <section><span className="am-label">Patient&apos;s intake notes</span><p className="am-pre am-muted" style={{ marginTop: 4 }}>{String(s.clinical_notes)}</p></section> : null}
        <section><span className="am-label">Outcome</span><p style={{ marginTop: 4 }}>{outcomeLabel(a.clinical_outcome) ?? 'No clinical outcome recorded.'}</p></section>
      </>}
      {s && tab === 'files' && <>
        {!files.intake.length && !files.rx.length && <p className="am-faint">{files.accessEnded ? FILES_ACCESS_ENDED : 'No files for this consultation.'}</p>}
        {files.rx.map(d => <FileRow key={String(d.id)} rx title={`Prescription page ${String(d.page_number ?? 1)}`} meta={`Issued ${date(d.created_at as string)}`} onOpen={() => onDocument(a.id, d, a.id)}/>)}
        {files.intake.map((d, i) => <FileRow key={String(d.id)} title={`Patient upload ${i + 1}`} meta={`Before the consultation · ${date(d.created_at as string)}`} onOpen={() => onDocument(a.id, d, a.id)}/>)}
      </>}
      <div className="am-row" style={{ paddingTop: 8, borderTop: '1px solid var(--am-line-soft)' }}>
        <button className="am-btn ghost sm" onClick={() => action('chat', a)}><Icon name="chat" size={15}/>Chat transcript</button>
        <button className="am-btn ghost sm" onClick={() => action('history', a)}><Icon name="history" size={15}/>Patient history</button>
      </div>
    </div>
  </section>;
}
