'use client';
import { useEffect, useRef, useState } from 'react';
import { useResource } from '../lib/client';
import { Appointment, Conversation, Profile, RecordDto, consultationAction, consultationIntent, rxDispatchAllowed } from '../lib/contracts';
import { useAgoraCall } from '../lib/useAgoraCall';
import { callStatusText, callStatusTone } from '../lib/call-status';
import { joinGate, lockedMessage, formatCountdown, eatDateTime, type JoinInfo } from '../lib/join-window';
import { Avatar, Badge, PageHeader, State, appointmentStatus, demographics } from './Common';
import { Icon } from './Icons';
import { ChatThread } from './Chat';
import { FileRow, consultationFiles } from './Clinical';
import { ModalityBadge } from './Dashboard';
import { ClinicalAction, date, time } from './Queue';

type Tab = 'patient' | 'uploads' | 'chat' | 'history';
export function Telehealth({ appointment: a, appointments, select, profile, photo, action, onDocument, complete, gap }: {
  appointment: Appointment | null; appointments: Appointment[]; select: (a: Appointment) => void; profile: Profile; photo?: string; action: ClinicalAction;
  onDocument: (appointmentId: string, document: RecordDto, current?: string) => void; complete: () => void; gap: (message: string) => void;
}) {
  const [tab, setTab] = useState<Tab>('patient');
  // Server-derived join eligibility: the list row's `join` first, then a fresh
  // `join-eligibility` read on focus, visibility, a 30 s poll and when the countdown
  // reaches zero. joinGate() corrects the device clock with server time.
  const eligibilityRes = useResource<JoinInfo>(a && a.modality === 'VIDEO' ? `consultations/${a.id}/join-eligibility` : null);
  const joinInfo = eligibilityRes.data ?? a?.join;
  const [now, setNow] = useState(() => Date.now());
  const gate = a && a.modality === 'VIDEO' ? joinGate(joinInfo, now) : null;
  const gateState = gate?.state;
  const apptId = a?.id, apptVideo = a?.modality === 'VIDEO';
  const refreshEligibility = eligibilityRes.refresh;
  useEffect(() => {
    if (!apptId || !apptVideo) return;
    const tick = setInterval(() => setNow(Date.now()), 1000);
    const revalidate = () => { if (document.visibilityState === 'visible') void refreshEligibility(); };
    const poll = setInterval(revalidate, 30000);
    document.addEventListener('visibilitychange', revalidate); window.addEventListener('focus', revalidate);
    return () => { clearInterval(tick); clearInterval(poll); document.removeEventListener('visibilitychange', revalidate); window.removeEventListener('focus', revalidate); };
  }, [apptId, apptVideo, refreshEligibility]);
  useEffect(() => { if (gateState === 'open') void refreshEligibility(); }, [gateState, refreshEligibility]);
  // Before the room opens no consultation session exists, so history is "not open yet";
  // authorization is never bypassed, we just avoid asking a question the server can only 404.
  const recordsLocked = gateState === 'locked';
  const lockText = a && gate && gate.state !== 'open' && consultationIntent(a, now) === 'call' ? lockedMessage(gate, now) : null;
  const summary = useResource<RecordDto>(a && (tab === 'patient' || tab === 'uploads') ? `doctor/consultations/${a.id}/summary` : null);
  const history = useResource<RecordDto[]>(a && tab === 'history' && !recordsLocked ? `doctor/appointments/${a.id}/patient-history?limit=25&offset=0` : null);
  const chats = useResource<Conversation[]>(a && tab === 'chat' ? 'doctor/chats?limit=100&offset=0' : null);
  const chat = chats.data?.find(row => row.appointment_id === a?.id);
  const eligible = a !== null && consultationAction({ ...a, join: joinInfo }, now) === 'call';
  const call = useAgoraCall(eligible ? a!.id : null, gate?.opensAt);
  const callActive = ['connecting', 'waiting-for-remote', 'connected', 'reconnecting'].includes(call.state.phase);
  const [connectedAt, setConnectedAt] = useState<number | null>(null);
  useEffect(() => { if (call.state.phase === 'connected' && call.state.remotePresent) setConnectedAt(t => t ?? Date.now()); if (!callActive) setConnectedAt(null); }, [call.state.phase, call.state.remotePresent, callActive]);
  const stage = useRef<HTMLDivElement>(null);
  const tone = callStatusTone(call.state.phase, eligible);
  const files = consultationFiles(summary.data);
  const elapsed = connectedAt ? Math.floor((now - connectedAt) / 1000) : 0;

  if (!a) return <>
    <PageHeader title="Consultation room" subtitle="Start a video consultation when its window opens."/>
    <section className="am-card"><State empty="No consultation selected" icon="video" hint="Choose a consultation from Appointments or your dashboard to open the room."/></section>
  </>;
  const status = appointmentStatus(a);
  return <>
    <PageHeader title="Consultation room" subtitle={<>{a.appointment_number} · {date(a.start_time)}, {time(a.start_time)} – {time(a.end_time)}</>}>
      {appointments.length > 1 && <label><span className="am-sr">Switch consultation</span><select className="am-select" style={{ minWidth: 240 }} value={a.id} disabled={callActive} onChange={e => { const next = appointments.find(row => row.id === e.target.value); if (next) select(next); }}>{appointments.map(row => <option key={row.id} value={row.id}>{time(row.start_time)} · {row.patient_display_name}</option>)}</select></label>}
      <button className="am-btn soft" disabled={!rxDispatchAllowed(a)} title={rxDispatchAllowed(a) ? undefined : 'Available once the consultation has started'} onClick={() => action('rx', a)}><Icon name="rx" size={16}/>Prescription</button>
      <button className="am-btn primary" disabled={!['CONFIRMED', 'WAITING', 'IN_CONSULTATION'].includes(a.status)} onClick={() => { call.leave(); complete(); }}><Icon name="checkCircle" size={16}/>Complete</button>
    </PageHeader>
    <div className="am-call">
      <div ref={stage} className="am-stage">
        <div className="am-stage-top">
          <span className={`am-glass ${tone}`}><i className="am-dot"/>{lockText ?? callStatusText(a, eligible, call.state.phase, call.state.remotePresent)}</span>
          {connectedAt && <span className="am-glass red"><i className="am-dot"/>{String(Math.floor(elapsed / 60)).padStart(2, '0')}:{String(elapsed % 60).padStart(2, '0')}</span>}
        </div>
        {call.state.phase === 'connected' && call.state.remotePresent && <div ref={call.remoteVideoRef} className="am-remote"/>}
        {!(call.state.phase === 'connected' && call.state.remotePresent) && <div className="am-stage-center">
          <span className="am-avatar xl on-dark" aria-hidden="true">{a.patient_display_name.trim().split(/\s+/).slice(0, 2).map(x => x[0]).join('').toUpperCase()}</span>
          <h2>{a.patient_display_name}</h2>
          {a.modality === 'CHAT' ? <><p>This is a chat consultation.</p><button className="am-btn white lg" style={{ background: '#fff', color: 'var(--am-green-deep)' }} onClick={() => action('chat', a)}><Icon name="chat"/>Open consultation chat</button></>
            : lockText && gate ? <div data-testid="join-locked" className="am-stack" style={{ alignItems: 'center', gap: 6 }}><p>{lockText}</p>{gate.state === 'locked' && <strong style={{ fontSize: 28, color: '#fff' }}>{formatCountdown(gate.msUntilOpen ?? 0)}</strong>}{gate.state === 'locked' && <p style={{ fontSize: 13 }}>The room opens automatically. You can review the patient&apos;s uploads meanwhile.</p>}</div>
            : eligible && (call.state.phase === 'idle' || call.state.phase === 'failed') ? <div className="am-stack" style={{ alignItems: 'center', gap: 8 }}>
              {call.state.error && <p style={{ color: '#fecaca', fontWeight: 600 }}>{call.state.error}</p>}
              {call.state.diagnostic && <p data-testid="call-diagnostic" style={{ fontSize: 12 }}>Diagnostic: {call.state.diagnostic}</p>}
              <button className="am-btn lg" style={{ background: '#fff', color: 'var(--am-green-deep)' }} onClick={call.join}><Icon name="video"/>{call.state.phase === 'failed' ? 'Try joining again' : 'Join video consultation'}</button>
              <p style={{ fontSize: 13 }}>Your camera and microphone will be requested when you join.</p>
            </div>
            : <p>{callStatusText(a, eligible, call.state.phase, call.state.remotePresent)}</p>}
        </div>}
        <div className="am-pip">{callActive ? <div ref={call.localVideoRef} className="am-pip-local"/> : <span className="am-avatar lg on-dark">{photo ? <img src={photo} alt=""/> : profile.full_name.trim().split(/\s+/).slice(0, 2).map(x => x[0]).join('').toUpperCase()}</span>}<span className="am-pip-label">You</span></div>
        <div className="am-controls">
          <button className="am-ctrl" disabled={!callActive} aria-pressed={!call.state.micEnabled} aria-label={call.state.micEnabled ? 'Mute microphone' : 'Unmute microphone'} title={call.state.micEnabled ? 'Mute' : 'Unmute'} onClick={call.toggleMic}><Icon name={call.state.micEnabled ? 'mic' : 'micOff'} size={21}/></button>
          <button className="am-ctrl" disabled={!callActive} aria-pressed={!call.state.cameraEnabled} aria-label={call.state.cameraEnabled ? 'Turn camera off' : 'Turn camera on'} title={call.state.cameraEnabled ? 'Camera off' : 'Camera on'} onClick={call.toggleCamera}><Icon name={call.state.cameraEnabled ? 'video' : 'videoOff'} size={21}/></button>
          <button className="am-ctrl" disabled={!callActive} aria-pressed={call.state.screenSharing} aria-label={call.state.screenSharing ? 'Stop sharing screen' : 'Share screen'} title="Share screen" onClick={call.toggleScreenShare}><Icon name="screen" size={21}/></button>
          <button className="am-ctrl" aria-label="Full screen" title="Full screen" onClick={() => { if (document.fullscreenElement) void document.exitFullscreen(); else void stage.current?.requestFullscreen?.(); }}><Icon name="fullscreen" size={20}/></button>
          <button className="am-ctrl end" disabled={!callActive} onClick={() => call.leave()}><Icon name="phoneOff" size={19}/>Leave call</button>
        </div>
      </div>

      <section className="am-card am-side">
        <div className="am-underline-tabs" role="tablist" aria-label="Consultation panel">
          {([['patient', 'Patient'], ['uploads', `Uploads${files.intake.length ? ` (${files.intake.length})` : ''}`], ['chat', 'Chat'], ['history', 'History']] as [Tab, string][]).map(([value, label]) => <button key={value} role="tab" className="am-underline-tab" aria-selected={tab === value} onClick={() => setTab(value)}>{label}</button>)}
        </div>
        <div className="am-scroll" style={tab === 'chat' ? { display: 'flex', flexDirection: 'column', minHeight: 420 } : { padding: 20 }}>
          {tab === 'patient' && <div className="am-stack">
            <div className="am-person"><Avatar name={a.patient_display_name} size="lg"/><div><strong>{a.patient_display_name}</strong><span>{demographics(a) || 'Patient'}</span></div></div>
            <div className="am-row"><ModalityBadge modality={a.modality}/><Badge tone={status.tone} dot>{status.label}</Badge></div>
            <State compact loading={summary.loading} error={summary.error} retry={() => void summary.refresh()}/>
            {summary.data && <>
              <section><span className="am-label">Reason for consultation</span><p className="am-pre" style={{ marginTop: 4 }}>{String(summary.data.chief_complaint ?? 'The patient did not add a reason.')}</p></section>
              {summary.data.clinical_notes ? <section><span className="am-label">Patient&apos;s intake notes</span><p className="am-pre am-muted" style={{ marginTop: 4 }}>{String(summary.data.clinical_notes)}</p></section> : null}
            </>}
            <div className="am-stack" style={{ gap: 8 }}>
              <button className="am-btn soft block" disabled={!rxDispatchAllowed(a)} title={rxDispatchAllowed(a) ? undefined : 'Available once the consultation has started'} onClick={() => action('rx', a)}><Icon name="rx" size={16}/>Add prescription (Rx)</button>
              <button className="am-btn ghost block" onClick={() => action('history', a)}><Icon name="history" size={16}/>Full patient history</button>
              <button className="am-btn ghost block" onClick={() => action('documents', a)}><Icon name="file" size={16}/>All documents</button>
            </div>
          </div>}
          {tab === 'uploads' && <div className="am-stack" style={{ gap: 10 }}>
            <p className="am-faint">Files the patient uploaded before the consultation.</p>
            <State compact loading={summary.loading} error={summary.error} retry={() => void summary.refresh()} empty={summary.data && !files.intake.length ? 'No uploads from the patient' : undefined} icon="upload"/>
            {files.intake.map((d, i) => <FileRow key={String(d.id)} title={`Patient upload ${i + 1}`} meta={`${String(d.mime_type ?? '').includes('pdf') ? 'PDF' : 'Image'} · ${date(d.created_at as string)}`} onOpen={() => onDocument(a.id, d, a.id)}/>)}
            {files.rx.length > 0 && <><span className="am-label" style={{ marginTop: 8 }}>Your prescription</span>{files.rx.map(d => <FileRow key={String(d.id)} rx title={`Prescription page ${String(d.page_number ?? 1)}`} meta="Issued in this consultation" onOpen={() => onDocument(a.id, d, a.id)}/>)}</>}
          </div>}
          {tab === 'chat' && (chat ? <ChatThread chat={chat} profile={profile} appointment={a} onBack={() => setTab('patient')} action={action} gap={gap} compact/> : <div style={{ padding: 20 }}><State compact loading={chats.loading} error={chats.error} retry={() => void chats.refresh()} empty={!chats.loading && !chats.error ? 'No chat for this consultation yet' : undefined} icon="chat"/></div>)}
          {tab === 'history' && <div className="am-stack">
            <State compact loading={history.loading} error={history.error} retry={() => void history.refresh()} empty={recordsLocked && gate?.opensAt ? `Patient records open with the consultation room (${eatDateTime(gate.opensAt, now)}).` : !history.loading && !history.error && history.data?.length === 0 ? 'No previous encounters' : undefined} icon="history"/>
            {!!history.data?.length && <div className="am-timeline">{history.data.map(row => <div key={String(row.historical_appointment_id)} className="am-timeline-item"><strong className="am-strong">{date(row.date as string)}</strong><p className="am-faint">{String(row.doctor_name)}{row.specialty ? ` · ${String(row.specialty)}` : ''}</p><p className="am-clamp" style={{ fontSize: 13 }}>{String(row.presenting_problem ?? 'No reason recorded')}</p></div>)}</div>}
            {!!history.data?.length && <button className="am-btn ghost block" onClick={() => action('history', a)}>Open full history</button>}
          </div>}
        </div>
      </section>
    </div>
  </>;
}
