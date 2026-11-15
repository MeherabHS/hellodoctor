'use client';
import { useEffect, useState } from 'react';
import { useResource } from '../lib/client';
import { Appointment, Profile, Question, RecordDto, consultationAction, consultationActionLabel, money } from '../lib/contracts';
import { Avatar, Badge, State, Stat, appointmentStatus, demographics } from './Common';
import { Icon } from './Icons';
import { ClinicalAction, QueueState, date, dayKey, time } from './Queue';
import type { Page } from './Shell';

export const PLATFORM_NOTE = 'Total calculation is based including the platform charge 20%';
function greeting(now: number) {
  const hour = Number(new Intl.DateTimeFormat('en-GB', { hour: 'numeric', hour12: false, timeZone: 'Africa/Nairobi' }).format(new Date(now)));
  return hour < 12 ? 'Good morning' : hour < 17 ? 'Good afternoon' : 'Good evening';
}
function relative(start: string, now: number) {
  const minutes = Math.round((Date.parse(start) - now) / 60000);
  if (minutes <= 0) return 'Now';
  if (minutes < 60) return `In ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `In ${hours} h ${minutes % 60 ? `${minutes % 60} min` : ''}`.trim() : date(start);
}
export function ModalityBadge({ modality }: { modality: string }) {
  return modality === 'CHAT' ? <Badge tone="violet"><Icon name="chat" size={13}/>Chat</Badge> : <Badge tone="teal"><Icon name="video" size={13}/>Video</Badge>;
}

export function Dashboard({ profile, queue, duty, go, action, paymentRequired, openQuestions, search }: {
  profile: Profile; queue: QueueState; duty: { value?: boolean }; go: (page: Page) => void; action: ClinicalAction; paymentRequired: boolean;
  openQuestions: { data?: Question[]; loading: boolean; error: string }; search: () => void;
}) {
  const all = useResource<Appointment[]>('doctor/appointments?view=all&limit=100&offset=0');
  const wallet = useResource<RecordDto>(paymentRequired ? 'doctor/wallet/summary' : null);
  const [now, setNow] = useState(Date.now());
  useEffect(() => { const timer = setInterval(() => setNow(Date.now()), 30000); return () => clearInterval(timer); }, []);
  const today = dayKey(new Date(now).toISOString());
  const startOfToday = Date.parse(`${today}T00:00:00+03:00`);
  const rows = all.data ?? [];
  // Today's counts are exact only when the page reached rows older than today
  // (past rows are returned newest-first after all current and future ones).
  const complete = !!all.data && (rows.length < 100 || Date.parse(rows[rows.length - 1].start_time) < startOfToday);
  const todays = rows.filter(a => dayKey(a.start_time) === today && !['CANCELLED', 'REFUNDED', 'EXPIRED', 'PENDING_PAYMENT'].includes(a.status));
  const completedToday = todays.filter(a => a.status === 'COMPLETED').length;
  const live = queue.data.filter(a => a.queue_actionable === true);
  const upcoming = rows.filter(a => ['CONFIRMED', 'WAITING', 'IN_CONSULTATION'].includes(a.status) && (Date.parse(a.end_time) > now || a.status === 'IN_CONSULTATION'));
  const next = live.find(a => a.status === 'IN_CONSULTATION') ?? live[0] ?? upcoming.find(a => Date.parse(a.start_time) >= now);
  const loading = all.loading && !all.data;
  const value = (n: number) => loading || all.error ? '—' : complete ? n : '—';
  const agenda = todays.length ? todays.slice().sort((a, b) => Date.parse(a.start_time) - Date.parse(b.start_time)) : live;
  const w = wallet.data;
  return <>
    <section className="am-hero">
      <div style={{ position: 'relative', zIndex: 1 }}>
        <h1>{greeting(now)}, Dr. {profile.full_name.replace(/^Dr\.?\s+/i, '').split(' ')[0]}</h1>
        <p>{next ? `Your next consultation is ${relative(next.start_time, now).toLowerCase()}. Take care of yourself, too.` : 'Your consultation list is clear for now. A calm moment to catch up.'}</p>
        <div className="am-hero-meta">
          <span className="am-chip-glass"><Icon name="calendar" size={15}/>{new Date(now).toLocaleDateString('en-KE', { weekday: 'long', day: 'numeric', month: 'long', timeZone: 'Africa/Nairobi' })}</span>
          <span className="am-chip-glass"><Icon name={duty.value ? 'checkCircle' : 'clock'} size={15}/>{duty.value ? 'Online & ready' : 'Currently offline'}</span>
          <span className="am-chip-glass"><Icon name="stethoscope" size={15}/>{profile.primary_specialty}</span>
        </div>
      </div>
      <div className="am-hero-actions">
        {next && consultationAction(next) ? <button className="am-btn white lg" onClick={() => action(consultationAction(next)!, next)}><Icon name={next.modality === 'CHAT' ? 'chat' : 'video'}/>{consultationActionLabel(next)}</button>
          : <button className="am-btn white lg" onClick={() => go('schedule')}><Icon name="calendarPlus"/>Manage availability</button>}
        <button className="am-btn ghost lg" onClick={() => go('appointments')}>View appointments</button>
      </div>
    </section>

    <div className="am-grid stats">
      <Stat icon="calendar" label="Today's consultations" value={value(todays.length)} sub={complete || loading ? 'Scheduled for today, East Africa Time' : 'Open Appointments for the full list'}/>
      <Stat icon="checkCircle" tone="teal" label="Completed today" value={value(completedToday)} sub={todays.length && complete ? `${Math.round(completedToday / todays.length * 100)}% of today's list` : 'Finished consultations'}/>
      <Stat icon="clock" tone="amber" label="Upcoming" value={queue.loading && !queue.data.length ? '—' : upcoming.length} sub={next ? `Next at ${time(next.start_time)}` : 'Nothing booked yet'}/>
      <Stat icon="help" tone="violet" label="Open health questions" value={openQuestions.data ? `${openQuestions.data.length}${openQuestions.data.length === 100 ? '+' : ''}` : '—'} sub="Anonymous community questions"/>
    </div>

    <div className="am-grid main-side align-start">
      <section className="am-card">
        <div className="am-card-head"><div><h2><Icon name="calendar"/>Today&apos;s appointments</h2><p>{date(new Date(now).toISOString())} · times in EAT</p></div><button className="am-link" onClick={() => go('appointments')}>View all<Icon name="arrowRight" size={15}/></button></div>
        <State loading={loading} error={all.error} retry={() => void all.refresh()} empty={!loading && !all.error && !agenda.length ? 'No consultations today' : undefined} hint="New bookings appear here automatically. You can open more slots from your schedule."/>
        {!loading && !all.error && agenda.length > 0 && <div className="am-list">{agenda.slice(0, 7).map(a => <AgendaRow key={a.id} a={a} now={now} next={next?.id === a.id} action={action}/>)}</div>}
      </section>

      <div className="am-stack" style={{ gap: 18 }}>
        <section className="am-card">
          <div className="am-card-head"><h2><Icon name="sparkle"/>Next consultation</h2>{next && <Badge tone="green" dot>{relative(next.start_time, now)}</Badge>}</div>
          <div className="am-card-body">
            {!next ? <State compact icon="calendar" empty="No upcoming consultation" hint="Open slots so patients can book you."/> : <div className="am-stack">
              <div className="am-person"><Avatar name={next.patient_display_name} size="lg"/><div><strong>{next.patient_display_name}</strong><span>{[demographics(next), next.appointment_number].filter(Boolean).join(' · ')}</span></div></div>
              <div className="am-row"><ModalityBadge modality={next.modality}/><Badge tone="blue"><Icon name="clock" size={13}/>{time(next.start_time)} – {time(next.end_time)}</Badge></div>
              <div className="am-row">
                <button className="am-btn primary" disabled={!consultationAction(next)} onClick={() => consultationAction(next) && action(consultationAction(next)!, next)}><Icon name={next.modality === 'CHAT' ? 'chat' : 'video'} size={16}/>{consultationAction(next) ? 'Start consultation' : 'Not open yet'}</button>
                <button className="am-btn ghost" onClick={() => action('summary', next)}>View details</button>
              </div>
              {!consultationAction(next) && <p className="am-faint">{consultationActionLabel(next)}</p>}
            </div>}
          </div>
        </section>

        {paymentRequired && <section className="am-card">
          <div className="am-card-head"><h2><Icon name="wallet"/>Eligible earnings</h2><button className="am-link" onClick={() => go('wallet')}>Details<Icon name="arrowRight" size={15}/></button></div>
          <div className="am-card-body">
            {w ? <div className="am-stack" style={{ gap: 8 }}>
              <div className="am-row between"><span className="am-muted">1. Gross earnings</span><strong className="am-strong">{money(w.total_gross)}</strong></div>
              <div className="am-row between"><span className="am-muted">2. Platform charge (20%)</span><strong style={{ color: 'var(--am-red-fg)' }}>− {money(w.withheld_fee)}</strong></div>
              <div className="am-row between" style={{ paddingTop: 8, borderTop: '1px dashed var(--am-line)' }}><span className="am-strong">3. Net take-home</span><strong style={{ color: 'var(--am-green-deep)', fontSize: 20 }}>{money(w.final_net)}</strong></div>
              <p className="am-faint">{PLATFORM_NOTE}</p>
            </div> : <State compact loading={wallet.loading} error={wallet.error} retry={() => void wallet.refresh()}/>}
          </div>
        </section>}

        <section className="am-card">
          <div className="am-card-head"><h2><Icon name="sparkle"/>Quick actions</h2></div>
          <div className="am-card-body am-stack" style={{ gap: 8 }}>
            {[['calendarPlus', 'Add availability', () => go('schedule')], ['search', 'Find a patient', search], ['help', 'Answer health questions', () => go('qna')], ['pen', 'Write an article', () => go('articles')]].map(([icon, label, run]) =>
              <button key={label as string} className="am-btn soft block" style={{ justifyContent: 'space-between' }} onClick={run as () => void}><span className="am-row" style={{ gap: 10 }}><Icon name={icon as 'search'} size={17}/>{label as string}</span><Icon name="chevronRight" size={16}/></button>)}
          </div>
        </section>
      </div>
    </div>
  </>;
}

function AgendaRow({ a, now, next, action }: { a: Appointment; now: number; next: boolean; action: ClinicalAction }) {
  const status = appointmentStatus(a);
  const primary = consultationAction(a, now);
  return <div className="am-list-row" data-appointment-id={a.id} style={next ? { background: 'var(--am-green-mist)' } : undefined}>
    <div className="am-time"><strong>{time(a.start_time)}</strong><span>{time(a.end_time)}</span></div>
    <div className="am-person am-grow"><Avatar name={a.patient_display_name}/><div style={{ minWidth: 0 }}><strong>{a.patient_display_name}</strong><span>{[demographics(a), a.appointment_number].filter(Boolean).join(' · ')}</span></div></div>
    <ModalityBadge modality={a.modality}/>
    <Badge tone={status.tone} dot>{status.label}</Badge>
    {primary ? <button className="am-btn primary sm" onClick={() => action(primary, a)}><Icon name={primary === 'chat' ? 'chat' : 'video'} size={15}/>Start</button>
      : <button className="am-btn ghost sm" onClick={() => action('summary', a)}>View</button>}
  </div>;
}
