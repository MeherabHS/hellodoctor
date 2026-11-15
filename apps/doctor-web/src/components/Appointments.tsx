'use client';
import { useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { Appointment, consultationAction, consultationActionLabel } from '../lib/contracts';
import { Avatar, Badge, PageHeader, State, appointmentStatus, demographics } from './Common';
import { Icon } from './Icons';
import { ModalityBadge } from './Dashboard';
import { ClinicalAction, QueueState, date, time } from './Queue';

type Tab = 'today' | 'upcoming';
export function Appointments({ queue, action, search, setSearch, addSlot, schedule }: { queue: QueueState; action: ClinicalAction; search: string; setSearch: (value: string) => void; addSlot: () => void; schedule: () => void }) {
  const [tab, setTab] = useState<Tab>('today');
  const upcoming = useResource<Appointment[]>(tab === 'upcoming' ? 'doctor/appointments?view=schedule&limit=100&offset=0' : null);
  const [more, setMore] = useState({ busy: false, error: '', done: false });
  const today = queue.data.filter(a => a.queue_actionable === true);
  const source = tab === 'today' ? today : upcoming.data ?? [];
  const term = search.trim().toLowerCase();
  const rows = source.filter(a => !term || `${a.patient_display_name} ${a.appointment_number}`.toLowerCase().includes(term));
  const loading = tab === 'today' ? queue.loading && !queue.data.length : upcoming.loading;
  const error = tab === 'today' ? queue.error : upcoming.error || more.error;
  async function loadMore() {
    if (more.busy || !upcoming.data) return;
    setMore({ busy: true, error: '', done: false });
    try { const next = await api<Appointment[]>(`doctor/appointments?view=schedule&limit=100&offset=${upcoming.data.length}`); upcoming.setData(old => [...(old ?? []), ...next]); setMore({ busy: false, error: '', done: next.length < 100 }); }
    catch (e) { setMore({ busy: false, error: errorText(e), done: false }); }
  }
  return <>
    <PageHeader title="Appointments" subtitle="Your booked consultations. Start a consultation when its window opens.">
      <button className="am-btn ghost" onClick={schedule}><Icon name="calendar" size={17}/>Schedule</button>
      <button className="am-btn primary" onClick={addSlot}><Icon name="plus" size={17}/>Add availability</button>
    </PageHeader>
    <section className="am-card">
      <div className="am-card-head">
        <div className="am-tabs" role="tablist" aria-label="Appointment period">
          <button className="am-tab" role="tab" aria-selected={tab === 'today'} onClick={() => setTab('today')}><Icon name="sun" size={15}/>Today<span className="am-tab-count">{today.length}</span></button>
          <button className="am-tab" role="tab" aria-selected={tab === 'upcoming'} onClick={() => setTab('upcoming')}><Icon name="calendar" size={15}/>Upcoming{upcoming.data && <span className="am-tab-count">{upcoming.data.length}{upcoming.data.length >= 100 && !more.done ? '+' : ''}</span>}</button>
        </div>
        <div className="am-row">
          <label className="am-search" style={{ width: 300, maxWidth: '100%' }}><Icon name="search" size={16}/><span className="am-sr">Search appointments</span><input value={search} placeholder="Search patient or appointment no." onChange={e => setSearch(e.target.value)}/></label>
          <button className="am-btn ghost sm" aria-label="Refresh appointments" onClick={() => void (tab === 'today' ? queue.refresh() : upcoming.refresh())}><Icon name="refresh" size={15}/></button>
        </div>
      </div>
      <State loading={loading} error={error} retry={() => void (tab === 'today' ? queue.refresh() : upcoming.refresh())}
        empty={!loading && !error && !rows.length ? (term ? 'No matching appointments' : tab === 'today' ? 'No consultations for the rest of today' : 'No upcoming bookings yet') : undefined}
        hint={term ? 'Try a different name or appointment number.' : 'Bookings appear here as soon as a patient confirms a slot.'}/>
      {!loading && rows.length > 0 && <div className="am-table-wrap"><table className="am-table">
        <thead><tr><th>Time</th><th>Patient</th><th>Type</th><th>Status</th><th style={{ textAlign: 'right' }}>Actions</th></tr></thead>
        <tbody>{rows.map(a => <Row key={a.id} a={a} action={action} showDate={tab === 'upcoming'}/>)}</tbody>
      </table></div>}
      {tab === 'upcoming' && (upcoming.data?.length ?? 0) >= 100 && !more.done && <div className="am-card-body am-row" style={{ justifyContent: 'center' }}><button className="am-btn ghost" disabled={more.busy} onClick={() => void loadMore()}>{more.busy ? 'Loading…' : 'Load more'}</button></div>}
    </section>
  </>;
}

function Row({ a, action, showDate }: { a: Appointment; action: ClinicalAction; showDate: boolean }) {
  const status = appointmentStatus(a);
  const primary = consultationAction(a);
  return <tr data-appointment-id={a.id}>
    <td className="nowrap"><strong className="am-strong">{time(a.start_time)}</strong><div className="am-faint">{showDate ? date(a.start_time) : `until ${time(a.end_time)}`}</div></td>
    <td><div className="am-person"><Avatar name={a.patient_display_name}/><div style={{ minWidth: 0 }}><strong>{a.patient_display_name}</strong><span>{[demographics(a), a.appointment_number].filter(Boolean).join(' · ')}</span></div></div></td>
    <td><ModalityBadge modality={a.modality}/></td>
    <td><Badge tone={status.tone} dot>{status.label}</Badge>{!primary && ['CONFIRMED', 'WAITING'].includes(a.status) && <div className="am-faint" style={{ marginTop: 4 }}>{consultationActionLabel(a)}</div>}</td>
    <td><div className="am-row end" style={{ flexWrap: 'nowrap' }}>
      {primary ? <button className="am-btn primary sm" onClick={() => action(primary, a)}><Icon name={primary === 'chat' ? 'chat' : 'video'} size={15}/>Start</button> : null}
      <button className="am-btn ghost sm" onClick={() => action('summary', a)}><Icon name="eye" size={15}/>Details</button>
      <button className="am-btn ghost sm" aria-label={`Messages with ${a.patient_display_name}`} title="Messages" onClick={() => action('chat', a)}><Icon name="chat" size={15}/></button>
    </div></td>
  </tr>;
}
