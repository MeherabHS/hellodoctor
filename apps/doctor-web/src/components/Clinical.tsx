'use client';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, ApiError, authorizedDocument, errorText, post, runtimeGeneration, useResource } from '../lib/client';
import { Appointment, RecordDto, consultationAction, consultationActionLabel, money, rxCorrectable, rxDispatchAllowed } from '../lib/contracts';
import { Avatar, Badge, Modal, State, appointmentStatus, demographics, outcomeLabel } from './Common';
import { Icon } from './Icons';
import { ModalityBadge, PLATFORM_NOTE } from './Dashboard';
import { ClinicalAction, date, time } from './Queue';

type OnDocument = (appointmentId: string, document: RecordDto, current?: string) => void;
function duration(start?: unknown, end?: unknown) {
  if (typeof start !== 'string' || typeof end !== 'string') return null;
  const seconds = Math.max(0, Math.round((Date.parse(end) - Date.parse(start)) / 1000));
  return `${Math.floor(seconds / 60)} min ${seconds % 60} sec`;
}
export function FileRow({ title, meta, rx, onOpen }: { title: string; meta: string; rx?: boolean; onOpen: () => void }) {
  return <div className="am-file"><span className={`am-file-icon${rx ? ' rx' : ''}`}><Icon name={rx ? 'rx' : 'file'} size={20}/></span>
    <div className="am-grow"><strong className="am-strong" style={{ display: 'block' }}>{title}</strong><span className="am-faint">{meta}</span></div>
    <button className="am-btn ghost sm" onClick={onOpen}><Icon name="eye" size={15}/>View</button></div>;
}
/** A doctor can open a consultation's files for 20 days after it ends; after
 *  that the server withholds them and reports `files_access_ended`. */
export const FILES_ACCESS_ENDED = 'Access to this consultation’s files ended 20 days after it.';
export function consultationFiles(summary: RecordDto | undefined) {
  const intake = ((summary?.documents as RecordDto[]) ?? []).filter(d => d.security_status !== 'INFECTED');
  const rx = ((summary?.prescriptions as RecordDto[]) ?? []).filter(d => !d.is_no_rx_required);
  return { intake, rx, accessEnded: summary?.files_access_ended === true };
}
const kind = (mime: unknown) => String(mime ?? '').includes('pdf') ? 'PDF' : 'Image';

export function Summary({ appointment: a, history, documents, complete, action, onDocument }: { appointment: Appointment; history: () => void; documents: () => void; complete: () => void; action: ClinicalAction; onDocument: OnDocument }) {
  const summary = useResource<RecordDto>(`doctor/consultations/${a.id}/summary`);
  const s = summary.data;
  const status = appointmentStatus(a);
  const primary = consultationAction(a);
  const files = consultationFiles(s);
  const open = ['CONFIRMED', 'WAITING', 'IN_CONSULTATION'].includes(a.status);
  return <div className="am-stack" style={{ gap: 18 }}>
    <div className="am-row between">
      <div className="am-person"><Avatar name={a.patient_display_name} size="lg"/><div><strong style={{ fontSize: 17 }}>{a.patient_display_name}</strong><span>{[demographics(a), a.appointment_number].filter(Boolean).join(' · ')}</span></div></div>
      <Badge tone={status.tone} dot>{status.label}</Badge>
    </div>
    <dl className="am-kv" style={{ gridTemplateColumns: '1fr 1fr' }}>
      <dt>Date</dt><dd>{date(a.start_time)}, {time(a.start_time)}</dd>
      <dt>Type</dt><dd><ModalityBadge modality={a.modality}/></dd>
      {duration(s?.started_at, s?.ended_at) && <><dt>Duration</dt><dd>{duration(s?.started_at, s?.ended_at)}</dd></>}
      {outcomeLabel(a.clinical_outcome) && <><dt>Outcome</dt><dd>{outcomeLabel(a.clinical_outcome)}</dd></>}
    </dl>
    <State loading={summary.loading} error={summary.error} retry={() => void summary.refresh()}/>
    {s && <>
      <section className="am-stack" style={{ gap: 8 }}><h3 className="am-row" style={{ fontSize: 14, gap: 8 }}><Icon name="stethoscope" size={17}/>Reason for consultation</h3><p className="am-pre">{String(s.chief_complaint ?? 'The patient did not add a reason.')}</p></section>
      {s.clinical_notes ? <section className="am-stack" style={{ gap: 8 }}><h3 className="am-row" style={{ fontSize: 14, gap: 8 }}><Icon name="pen" size={17}/>Patient&apos;s intake notes</h3><p className="am-pre am-muted">{String(s.clinical_notes)}</p></section> : null}
      <section className="am-stack" style={{ gap: 8 }}>
        <h3 className="am-row" style={{ fontSize: 14, gap: 8 }}><Icon name="paperclip" size={17}/>Files</h3>
        {!files.intake.length && !files.rx.length && <p className="am-faint">{files.accessEnded ? FILES_ACCESS_ENDED : 'No uploads or prescriptions for this consultation.'}</p>}
        {files.intake.map((d, i) => <FileRow key={String(d.id)} title={`Patient upload ${i + 1}`} meta={`${kind(d.mime_type)} · uploaded before the consultation · ${date(d.created_at as string)}`} onOpen={() => onDocument(a.id, d, a.id)}/>)}
        {files.rx.map(d => <FileRow key={String(d.id)} rx title={`Prescription page ${String(d.page_number ?? 1)}`} meta={`${kind(d.mime_type)} · issued ${date(d.created_at as string)}`} onOpen={() => onDocument(a.id, d, a.id)}/>)}
      </section>
    </>}
    <div className="am-row" style={{ paddingTop: 6, borderTop: '1px solid var(--am-line-soft)' }}>
      {primary && <button className="am-btn primary" onClick={() => action(primary, a)}><Icon name={primary === 'chat' ? 'chat' : 'video'} size={16}/>{consultationActionLabel(a)}</button>}
      {rxDispatchAllowed(a) && <button className="am-btn soft" onClick={() => action('rx', a)}><Icon name="rx" size={16}/>Write prescription</button>}
      {rxCorrectable(a) && <button className="am-btn ghost" onClick={() => action('rx', a)}><Icon name="pen" size={16}/>Add correction note</button>}
      <button className="am-btn ghost" onClick={history}><Icon name="history" size={16}/>Patient history</button>
      <button className="am-btn ghost" onClick={documents}><Icon name="file" size={16}/>All documents</button>
      <button className="am-btn ghost" onClick={() => action('chat', a)}><Icon name="chat" size={16}/>Messages</button>
      {open && <button className="am-btn ghost" onClick={complete}><Icon name="checkCircle" size={16}/>Complete</button>}
    </div>
  </div>;
}

const outcomes: [string, string, string][] = [
  ['COMPLETED_WITH_RX', 'Completed with prescription', 'A prescription has been, or will be, uploaded.'],
  ['COMPLETED_NO_RX', 'Completed without prescription', 'Advice given; no medicine prescribed.'],
  // No referral or escalation outcomes (product owner, 2026-11-24).
];
export function Completion({ appointment: a, onCompleted, onClose, onView }: { appointment: Appointment; onCompleted: () => void; onClose: () => void; onView: () => void }) {
  const [outcome, setOutcome] = useState('COMPLETED_WITH_RX'); const [reason, setReason] = useState(''); const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [confirmed, setConfirmed] = useState(false);
  const key = useRef(crypto.randomUUID()); const submitted = useRef(false); const previous = useRef('');
  async function submit(e: React.FormEvent) { e.preventDefault(); if (submitted.current) return; submitted.current = true; setBusy(true); setError(''); const body = outcome === 'COMPLETED_NO_RX' ? { reason } : { outcome }; const serialized = JSON.stringify(body); if (previous.current && previous.current !== serialized) key.current = crypto.randomUUID(); previous.current = serialized; try { await post(`consultations/${a.id}/${outcome === 'COMPLETED_NO_RX' ? 'complete-no-rx' : 'complete'}`, body, key.current); setConfirmed(true); onCompleted(); } catch (e) { setError(errorText(e)); } finally { submitted.current = false; setBusy(false); } }
  if (confirmed) return <div className="am-success" role="status">
    <span className="am-success-mark"><Icon name="check" size={38} stroke={2.6}/></span>
    <h2 style={{ fontSize: 22 }}>Consultation completed</h2>
    <p className="am-muted">The consultation with {a.patient_display_name} has been recorded. Thank you for your care.</p>
    <dl className="am-kv" style={{ margin: '8px 0' }}><dt>Appointment</dt><dd>{a.appointment_number}</dd><dt>Outcome</dt><dd>{outcomeLabel(outcome)}</dd></dl>
    <div className="am-row" style={{ width: '100%' }}><button className="am-btn ghost am-grow" onClick={onView}>View consultation</button><button className="am-btn primary am-grow" onClick={onClose}>Done</button></div>
  </div>;
  return <form className="am-form" onSubmit={submit}>
    <div><h2 style={{ fontSize: 20 }}>Complete consultation</h2><p className="am-muted" style={{ marginTop: 4 }}>Choose the outcome for {a.patient_display_name}. It is recorded once and cannot be changed afterwards.</p></div>
    <fieldset className="am-stack" style={{ border: 0, padding: 0, margin: 0, gap: 10 }}><legend className="am-label" style={{ marginBottom: 8 }}>Clinical outcome</legend>
      {outcomes.map(([value, label, help]) => <label key={value} className="am-file" style={{ cursor: 'pointer', borderColor: outcome === value ? 'var(--am-green)' : undefined, background: outcome === value ? 'var(--am-green-mist)' : undefined }}>
        <input type="radio" name="outcome" value={value} checked={outcome === value} disabled={busy} onChange={() => setOutcome(value)} style={{ accentColor: 'var(--am-green)', width: 18, height: 18 }}/>
        <span className="am-grow"><strong className="am-strong" style={{ display: 'block' }}>{label}</strong><span className="am-faint">{help}</span></span>
      </label>)}
    </fieldset>
    {outcome === 'COMPLETED_NO_RX' && <label className="am-field"><span>Why no prescription?</span><textarea className="am-textarea" required maxLength={1000} rows={3} value={reason} disabled={busy} placeholder="For example: advice and follow-up only" onChange={e => setReason(e.target.value)}/></label>}
    <State error={error}/>
    <div className="am-row end"><button type="button" className="am-btn ghost" onClick={onClose} disabled={busy}>Cancel</button><button className="am-btn primary" type="submit" disabled={busy || (outcome === 'COMPLETED_NO_RX' && !reason.trim())}><Icon name="check" size={16}/>{busy ? 'Completing…' : 'Complete consultation'}</button></div>
  </form>;
}

export function SlotForm({ onCreated, onClose }: { onCreated: (date: string) => void; onClose: () => void }) {
  const [error, setError] = useState(''); const [busy, setBusy] = useState(false); const [done, setDone] = useState(false); const [batch, setBatch] = useState(true);
  const [selectedDate, setDate] = useState(''); const [startTime, setStart] = useState('08:00'); const [endTime, setEnd] = useState('09:00'); const [weeks, setWeeks] = useState(1);
  const lock = useRef(false); const [created, setCreated] = useState<string[]>([]);
  const pending = useRef<{ ranges: Date[][]; keys: string[]; saved: string[] } | null>(null);
  const start = new Date(`${selectedDate}T${startTime}:00+03:00`), end = new Date(`${selectedDate}T${endTime}:00+03:00`);
  const perDay = Number.isFinite(start.getTime()) && end > start ? (batch ? Math.ceil((end.getTime() - start.getTime()) / 600000) : 1) : 0;
  const count = perDay * weeks;
  async function submit() {
    if (lock.current || done) return; setError('');
    if (!perDay) { setError('Choose a date and an end time after the start time.'); return; }
    if (start.getTime() <= Date.now()) { setError('Choose a future start time in East Africa Time (UTC+3).'); return; }
    if (perDay > 24) { setError('Choose an availability block of at most four hours.'); return; }
    lock.current = true; setBusy(true);
    const week = 7 * 24 * 3600000;
    const ranges = Array.from({ length: weeks }, (_, w) => (batch ? Array.from({ length: perDay }, (_, i) => [new Date(start.getTime() + w * week + i * 600000), new Date(Math.min(end.getTime(), start.getTime() + (i + 1) * 600000) + w * week)]) : [[new Date(start.getTime() + w * week), new Date(end.getTime() + w * week)]])).flat();
    pending.current ??= { ranges, keys: ranges.map(() => crypto.randomUUID()), saved: [] }; const job = pending.current; const generation = runtimeGeneration();
    try { for (let i = job.saved.length; i < job.ranges.length; i++) { if (generation !== runtimeGeneration()) return; const [from, to] = job.ranges[i]; const dto = await post<RecordDto>('doctor/slots', { start_time: from.toISOString(), end_time: to.toISOString() }, job.keys[i]); if (generation !== runtimeGeneration()) return; job.saved.push(String(dto.id)); setCreated([...job.saved]); } setDone(true); onCreated(selectedDate); }
    catch (e) { if (!job.saved.length && e instanceof ApiError && [409, 422].includes(e.status)) pending.current = null; setError((job.saved.length ? `${job.saved.length} slots were created. Retry continues with the remaining slots. ` : '') + errorText(e)); if (job.saved.length) onCreated(selectedDate); } finally { lock.current = false; setBusy(false); }
  }
  const frozen = busy || done || !!pending.current;
  if (done) return <div className="am-success" role="status"><span className="am-success-mark"><Icon name="calendarPlus" size={34}/></span><h2 style={{ fontSize: 20 }}>Availability published</h2><p className="am-muted">{created.length} slot{created.length === 1 ? '' : 's'} are now open for booking.</p><button className="am-btn primary" onClick={onClose}>Back to schedule</button></div>;
  return <div className="am-form">
    <p className="am-muted">Patients can book these times. All times are East Africa Time (UTC+3).</p>
    <label className="am-field"><span>Date</span><input className="am-input" aria-label="Slot date" type="date" required value={selectedDate} disabled={frozen} onChange={e => setDate(e.target.value)}/></label>
    <div className="am-form-grid">
      <label className="am-field"><span>Start time</span><input className="am-input" type="time" aria-label="Start time" value={startTime} disabled={frozen} onChange={e => setStart(e.target.value)}/></label>
      <label className="am-field"><span>End time</span><input className="am-input" type="time" aria-label="End time" value={endTime} disabled={frozen} onChange={e => setEnd(e.target.value)}/></label>
    </div>
    <div className="am-form-grid">
      <label className="am-field"><span>Slot length</span><select className="am-select" value={batch ? 'split' : 'one'} disabled={frozen} onChange={e => setBatch(e.target.value === 'split')}><option value="split">10-minute consultations</option><option value="one">One slot for the whole block</option></select></label>
      <label className="am-field"><span>Repeat</span><select className="am-select" value={weeks} disabled={frozen} onChange={e => setWeeks(Number(e.target.value))}><option value={1}>Does not repeat</option><option value={2}>Weekly for 2 weeks</option><option value={3}>Weekly for 3 weeks</option><option value={4}>Weekly for 4 weeks</option></select></label>
    </div>
    <div className="am-callout"><Icon name="calendar"/><span>{count ? <><strong>{count} slot{count === 1 ? '' : 's'}</strong>{weeks > 1 && selectedDate ? ` · every ${new Date(`${selectedDate}T12:00:00+03:00`).toLocaleDateString('en-KE', { weekday: 'long', timeZone: 'Africa/Nairobi' })} for ${weeks} weeks` : ''}</> : 'Choose a date and time to preview your slots.'}</span></div>
    {busy && <div><progress className="am-meter" max={count} value={created.length} aria-label="Publishing slots"/><p className="am-faint">Publishing {created.length} of {count}…</p></div>}
    <State error={error}/>
    <div className="am-row end"><button className="am-btn ghost" onClick={onClose} disabled={busy}>Cancel</button><button className="am-btn primary" onClick={() => void submit()} disabled={busy}><Icon name="check" size={16}/>{busy ? 'Publishing…' : pending.current ? 'Retry remaining slots' : 'Save & publish'}</button></div>
  </div>;
}

export function History({ appointment: a, onDocument }: { appointment: Appointment; onDocument: OnDocument }) {
  const history = useResource<RecordDto[]>(`doctor/appointments/${a.id}/patient-history?limit=25&offset=0`);
  const [episode, setEpisode] = useState<string | null>(null); const [paging, setPaging] = useState(false); const [error, setError] = useState(''); const [hasMore, setHasMore] = useState(true);
  async function more() { if (!history.data || paging) return; setPaging(true); try { const rows = await api<RecordDto[]>(`doctor/appointments/${a.id}/patient-history?limit=25&offset=${history.data.length}`); history.setData([...history.data, ...rows]); setHasMore(rows.length === 25); } catch (e) { setError(errorText(e)); history.setData(undefined); } finally { setPaging(false); } }
  if (episode) return <div className="am-stack"><button className="am-crumb" onClick={() => setEpisode(null)}><Icon name="arrowLeft" size={15}/>All encounters</button><Episode key={episode} appointment={a} episode={episode} onDocument={onDocument}/></div>;
  return <div className="am-stack">
    <div className="am-callout"><Icon name="shield"/><span>You can see this history because of your consultation {a.appointment_number}. Access ends when that consultation&apos;s window closes.</span></div>
    <State loading={history.loading} error={history.error || error} retry={() => void history.refresh()} empty={!history.loading && !history.error && history.data?.length === 0 ? 'No previous encounters' : undefined} icon="history"/>
    {!!history.data?.length && <div className="am-timeline">{history.data.map(row => <article key={String(row.historical_appointment_id)} className="am-timeline-item am-card pad" style={{ padding: 16 }}>
      <div className="am-row between"><strong className="am-strong">{date(row.date as string)}</strong><Badge tone="green">{String(row.status).replaceAll('_', ' ').toLowerCase()}</Badge></div>
      <p className="am-faint" style={{ margin: '2px 0 8px' }}>{String(row.doctor_name)}{row.specialty ? ` · ${String(row.specialty)}` : ''}</p>
      <p className="am-clamp">{String(row.presenting_problem ?? 'No reason recorded')}</p>
      <div className="am-row between" style={{ marginTop: 10 }}><span className="am-faint">{row.files_access_ended ? 'Files: access ended 20 days after this consultation' : `${String(row.document_count ?? 0)} document(s)${row.prescription_available ? ' · prescription' : ''}`}</span>{row.historical_consultation_id ? <button className="am-link" onClick={() => setEpisode(String(row.historical_consultation_id))}>Open encounter<Icon name="arrowRight" size={15}/></button> : <span className="am-faint">No consultation record</span>}</div>
    </article>)}</div>}
    {hasMore && (history.data?.length ?? 0) >= 25 && <button className="am-btn ghost" disabled={paging} onClick={() => void more()}>{paging ? 'Loading…' : 'Load earlier encounters'}</button>}
  </div>;
}
function Episode({ appointment: a, episode, onDocument }: { appointment: Appointment; episode: string; onDocument: OnDocument }) {
  const detail = useResource<RecordDto>(`doctor/appointments/${a.id}/patient-history/${episode}`);
  const d = detail.data;
  const documents = d?.files_access_ended === true ? [] : [...((d?.documents as RecordDto[]) ?? []), ...((d?.prescription_pages as RecordDto[]) ?? [])].filter(row => !row.is_no_rx_required);
  return <><State loading={detail.loading} error={detail.error} retry={() => void detail.refresh()}/>{d && <div className="am-stack">
    <div><h3 style={{ fontSize: 17 }}>{date(d.date as string)}</h3><p className="am-faint">{String(d.doctor_name)}{d.specialty ? ` · ${String(d.specialty)}` : ''}</p></div>
    <section><span className="am-label">Reason for consultation</span><p className="am-pre" style={{ marginTop: 4 }}>{String(d.presenting_problem ?? 'No reason recorded')}</p></section>
    {d.clinical_notes ? <section><span className="am-label">Patient&apos;s intake notes</span><p className="am-pre am-muted" style={{ marginTop: 4 }}>{String(d.clinical_notes)}</p></section> : null}
    <section className="am-stack" style={{ gap: 8 }}><span className="am-label">Files</span>{!documents.length && <p className="am-faint">{d.files_access_ended === true ? FILES_ACCESS_ENDED : 'No files for this encounter.'}</p>}{documents.map((doc, index) => <FileRow key={String(doc.id)} rx={doc.page_number !== undefined} title={doc.page_number !== undefined ? `Prescription page ${String(doc.page_number)}` : `Document ${index + 1}`} meta={kind(doc.mime_type)} onOpen={() => onDocument(String(d.historical_appointment_id), doc, a.id)}/>)}</section>
  </div>}</>;
}

export function Documents({ appointment: a, onDocument }: { appointment: Appointment; onDocument: OnDocument }) {
  const summary = useResource<RecordDto>(`doctor/consultations/${a.id}/summary`);
  const files = consultationFiles(summary.data);
  return <div className="am-stack">
    <div className="am-callout"><Icon name="lock"/><span>Files open through a short-lived secure link and are never stored in your browser.</span></div>
    <State loading={summary.loading} error={summary.error} retry={() => void summary.refresh()} empty={summary.data && !files.intake.length && !files.rx.length ? (files.accessEnded ? FILES_ACCESS_ENDED : 'No documents for this consultation') : undefined} icon="file"/>
    {files.intake.length > 0 && <section className="am-stack" style={{ gap: 8 }}><span className="am-label">Patient uploads (before the consultation)</span>{files.intake.map((d, i) => <FileRow key={String(d.id)} title={`Patient upload ${i + 1}`} meta={`${kind(d.mime_type)} · ${date(d.created_at as string)}`} onOpen={() => onDocument(a.id, d, a.id)}/>)}</section>}
    {files.rx.length > 0 && <section className="am-stack" style={{ gap: 8 }}><span className="am-label">Prescription</span>{files.rx.map(d => <FileRow key={String(d.id)} rx title={`Prescription page ${String(d.page_number ?? 1)}`} meta={`${kind(d.mime_type)} · ${date(d.created_at as string)}`} onOpen={() => onDocument(a.id, d, a.id)}/>)}</section>}
  </div>;
}

export function DocumentViewer({ current, appointmentId, document, onClose }: { current: string; appointmentId: string; document: RecordDto; onClose: () => void }) {
  const [url, setUrl] = useState(''); const [mime, setMime] = useState(''); const [error, setError] = useState(''); const [busy, setBusy] = useState(true); const blob = useRef('');
  const revoke = useCallback(() => { if (blob.current) URL.revokeObjectURL(blob.current); blob.current = ''; setUrl(''); }, []);
  const load = useCallback(async () => { revoke(); setBusy(true); setError(''); try { const file = await authorizedDocument(`/api/document/${appointmentId}/${String(document.id)}?current_appointment_id=${current}`); setMime(file.type); blob.current = URL.createObjectURL(file); setUrl(blob.current); } catch (e) { if (!(e instanceof DOMException && e.name === 'AbortError')) setError(errorText(e)); } finally { setBusy(false); } }, [appointmentId, document.id, current, revoke]);
  useEffect(() => { void load(); const timer = setTimeout(() => { revoke(); setError('Secure access expired. Open the document again to view it.'); }, 60000); return () => { clearTimeout(timer); if (blob.current) URL.revokeObjectURL(blob.current); }; }, [load, revoke]);
  return <Modal title="Secure document viewer" onClose={onClose} wide><div className="am-stack">
    <State loading={busy} error={error} retry={() => void load()}/>
    {url && <>{mime.startsWith('image/') ? <img src={url} alt="Clinical document" style={{ width: '100%', maxHeight: '64dvh', objectFit: 'contain', borderRadius: 14, background: 'var(--am-head)' }}/> : <div className="am-callout"><Icon name="file"/><span>This is a PDF document. Open it with your browser&apos;s PDF viewer.</span></div>}
      <div className="am-row between"><span className="am-faint">Access expires after one minute.</span><a className="am-btn primary" href={url} download={mime === 'application/pdf' ? 'clinical-document.pdf' : 'clinical-document.jpg'}><Icon name="download" size={16}/>Download</a></div></>}
  </div></Modal>;
}

export function Receipt({ row }: { row: RecordDto }) {
  return <div className="am-stack">
    <div className="am-person"><Avatar name={String(row.patient_display_name ?? 'Patient')}/><div><strong>{String(row.patient_display_name ?? 'Patient')}</strong><span>Ref {String(row.transaction_number)} · {date(String(row.created_at))}</span></div></div>
    <dl className="am-kv">
      <dt>1. Gross consultation fee</dt><dd>{money(row.gross_amount)}</dd>
      <dt>2. Platform charge (20%)</dt><dd style={{ color: 'var(--am-red-fg)' }}>− {money(row.platform_fee_amount)}</dd>
      <dt>3. Your net earning</dt><dd style={{ color: 'var(--am-green-deep)' }}>{money(row.net_amount)}</dd>
    </dl>
    <p className="am-faint">{PLATFORM_NOTE}. Financial state: {String(row.payment_status).replaceAll('_', ' ').toLowerCase()}. An eligible earning does not confirm a payout.</p>
  </div>;
}
