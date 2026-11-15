'use client';
import { useMemo, useState } from 'react';
import { Appointment, consultationAction } from '../lib/contracts';
import { Avatar, Badge, PageHeader, State, appointmentStatus, demographics, outcomeLabel } from './Common';
import { Icon } from './Icons';
import { ModalityBadge } from './Dashboard';
import { ClinicalAction, QueueState, date, time } from './Queue';

type PatientRow = { id: string; name: string; latest: Appointment; visits: Appointment[]; next?: Appointment; historyContext?: Appointment };
/** Only patients this doctor has consulted, grouped from the doctor's own appointment list.
 * No medical conditions, allergies or cohorts: none are exposed to doctors. */
function group(appointments: Appointment[], now: number): PatientRow[] {
  const map = new Map<string, Appointment[]>();
  for (const a of appointments) map.set(a.patient_id, [...(map.get(a.patient_id) ?? []), a]);
  return [...map.entries()].map(([id, list]) => {
    const visits = list.slice().sort((x, y) => Date.parse(y.start_time) - Date.parse(x.start_time));
    const next = visits.filter(a => ['CONFIRMED', 'WAITING', 'IN_CONSULTATION'].includes(a.status) && Date.parse(a.end_time) > now).pop();
    return { id, name: visits[0].patient_display_name, latest: visits[0], visits, next, historyContext: visits.find(a => a.history_access_active) };
  }).sort((x, y) => Date.parse(y.latest.start_time) - Date.parse(x.latest.start_time));
}

export function Patients({ queue, action, search }: { queue: QueueState; action: ClinicalAction; search: string }) {
  const [query, setQuery] = useState(search); const [selectedId, setSelectedId] = useState<string | null>(null);
  const now = Date.now();
  const patients = useMemo(() => group(queue.data, now), [queue.data]);
  const term = query.trim().toLowerCase();
  const rows = patients.filter(p => !term || `${p.name} ${p.visits.map(v => v.appointment_number).join(' ')}`.toLowerCase().includes(term));
  const selected = patients.find(p => p.id === selectedId) ?? null;
  return <>
    <PageHeader title="My patients" subtitle="People you have consulted. Clinical history opens only while a consultation with them is active."/>
    <div className="am-callout blue"><Icon name="shield"/><span>For patient privacy, this list shows only patients from your own appointments. Clinical records stay tied to each consultation&apos;s access window.</span></div>
    <div className={selected ? 'am-grid main-side' : 'am-stack'} style={{ alignItems: 'start' }}>
      <section className="am-card">
        <div className="am-card-head"><h2><Icon name="users"/>{patients.length} patient{patients.length === 1 ? '' : 's'}{queue.hasMore ? '+' : ''}</h2><label className="am-search" style={{ width: 300, maxWidth: '100%' }}><Icon name="search" size={16}/><span className="am-sr">Search patients</span><input value={query} placeholder="Search by name or appointment no." onChange={e => setQuery(e.target.value)}/></label></div>
        <State loading={queue.loading && !queue.data.length} error={queue.error} retry={() => void queue.refresh()} empty={!queue.loading && !queue.error && !rows.length ? (term ? 'No matching patients' : 'No patients yet') : undefined} icon="users" hint={term ? undefined : 'Patients appear here after they book a consultation with you.'}/>
        {rows.length > 0 && <div className="am-table-wrap"><table className="am-table">
          <thead><tr><th>Patient</th><th>Visits</th><th>Last consultation</th><th>Next</th><th style={{ textAlign: 'right' }}>Actions</th></tr></thead>
          <tbody>{rows.map(p => { const st = appointmentStatus(p.latest); return <tr key={p.id} aria-selected={selectedId === p.id} style={{ cursor: 'pointer' }} onClick={() => setSelectedId(p.id)}>
            <td><div className="am-person"><Avatar name={p.name}/><div style={{ minWidth: 0 }}><strong>{p.name}</strong><span>{demographics(p.latest) || 'Patient'}</span></div></div></td>
            <td><strong className="am-strong">{p.visits.length}</strong></td>
            <td className="nowrap"><span className="am-strong">{date(p.latest.start_time)}</span><div><Badge tone={st.tone}>{st.label}</Badge></div></td>
            <td>{p.next ? <><span className="am-strong">{date(p.next.start_time)}</span><div className="am-faint">{time(p.next.start_time)}</div></> : <span className="am-faint">—</span>}</td>
            <td><div className="am-row end" style={{ flexWrap: 'nowrap' }}>
              <button className="am-btn ghost sm" disabled={!p.historyContext} title={p.historyContext ? 'Open patient history' : 'History opens during an active consultation'} onClick={e => { e.stopPropagation(); if (p.historyContext) action('history', p.historyContext); }}><Icon name="history" size={15}/>History</button>
              <button className="am-btn ghost sm" aria-label={`Messages with ${p.name}`} onClick={e => { e.stopPropagation(); action('chat', p.latest); }}><Icon name="chat" size={15}/></button>
            </div></td>
          </tr>; })}</tbody>
        </table></div>}
      </section>
      {selected && <section className="am-card" style={{ position: 'sticky', top: 'calc(var(--am-topbar-h) + 20px)' }} aria-label="Patient profile">
        <div className="am-card-head"><h2>Patient profile</h2><button className="am-close" aria-label="Close profile" onClick={() => setSelectedId(null)}><Icon name="close"/></button></div>
        <div className="am-card-body am-stack" style={{ gap: 16 }}>
          <div className="am-person"><Avatar name={selected.name} size="lg"/><div><strong style={{ fontSize: 17 }}>{selected.name}</strong><span>{demographics(selected.latest) || 'Patient'}</span></div></div>
          {selected.next && <div className="am-callout"><Icon name="calendar"/><div className="am-grow"><strong className="am-strong">Next consultation</strong><p className="am-muted">{date(selected.next.start_time)} at {time(selected.next.start_time)}</p></div>{consultationAction(selected.next) && <button className="am-btn primary sm" onClick={() => action(consultationAction(selected.next!)!, selected.next!)}>Start</button>}</div>}
          <div className="am-row"><button className="am-btn ghost sm" disabled={!selected.historyContext} onClick={() => selected.historyContext && action('history', selected.historyContext)}><Icon name="history" size={15}/>Patient history</button><button className="am-btn ghost sm" onClick={() => action('chat', selected.latest)}><Icon name="chat" size={15}/>Messages</button></div>
          {!selected.historyContext && <p className="am-faint">Clinical history opens 15 minutes before a consultation and stays open for 24 hours after it is completed.</p>}
          <div><span className="am-label">Consultations with you</span>
            <div className="am-timeline" style={{ marginTop: 12 }}>{selected.visits.map(v => { const st = appointmentStatus(v); return <div key={v.id} className="am-timeline-item">
              <div className="am-row between"><strong className="am-strong">{date(v.start_time)}, {time(v.start_time)}</strong><Badge tone={st.tone}>{st.label}</Badge></div>
              <div className="am-row" style={{ marginTop: 4, gap: 8 }}><ModalityBadge modality={v.modality}/><span className="am-faint">{v.appointment_number}</span></div>
              {outcomeLabel(v.clinical_outcome) && <p className="am-faint" style={{ marginTop: 4 }}>{outcomeLabel(v.clinical_outcome)}</p>}
              <button className="am-link" style={{ marginTop: 6 }} onClick={() => action('summary', v)}>Details<Icon name="arrowRight" size={14}/></button>
            </div>; })}</div>
          </div>
        </div>
      </section>}
    </div>
  </>;
}
