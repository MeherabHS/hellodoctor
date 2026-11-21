'use client';
import { tr } from '@/lib/i18n';
import { apiErrorMessage } from '@/lib/i18n/apiErrors';
import { useCallback, useEffect, useState } from 'react';
import { investigationEvents } from '@/lib/investigation';
import { getGrievanceInvestigation, GrievanceInvestigation as Investigation, rescheduleAppointment, getBookableDoctorSlots, BookableDoctorSlot } from '@/lib/api';

function value(data: unknown): string {
  if (data === null || data === undefined) return tr('ui2.notRecorded');
  return typeof data === 'object' ? JSON.stringify(data) : String(data);
}

// All evidence comes from the authorized investigation endpoint. In particular,
// client acknowledgement and telemetry never imply human reading or RTC media.
export function GrievanceInvestigation({ grievanceId }: { grievanceId: string }) {
  return <InvestigationEvidence key={grievanceId} grievanceId={grievanceId} />;
}

function InvestigationEvidence({ grievanceId }: { grievanceId: string }) {
  const [data, setData] = useState<Investigation | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(true);
  const [slot, setSlot] = useState('');
  const [reason, setReason] = useState('');
  const [slots,setSlots]=useState<BookableDoctorSlot[]>([]);
  const doctorId=data?.facts.doctor_id;
  useEffect(()=>{if(!doctorId||!['CONFIRMED','WAITING'].includes(String(data?.facts.status)))return;let active=true;getBookableDoctorSlots(String(doctorId)).then(rows=>{if(active)setSlots(rows);}).catch(()=>{if(active)setError(tr('ui.unableToLoadReplacementAvailabilityRefre'));});return()=>{active=false;};},[doctorId,data?.facts.status,data?.event_page?.snapshot_at]);
  const load = useCallback(async () => {
    setBusy(true); setError('');
    try { setData(await getGrievanceInvestigation(grievanceId)); }
    catch (e) { setError(apiErrorMessage(e, tr('ui.unableToRetrieveInvestigationEvidence'))); }
    finally { setBusy(false); }
  }, [grievanceId]);
  useEffect(() => { let active = true;
    getGrievanceInvestigation(grievanceId).then(result => { if (active) setData(result); })
      .catch(e => { if (active) setError(apiErrorMessage(e, tr('ui.unableToRetrieveInvestigationEvidence'))); })
      .finally(() => { if (active) setBusy(false); });
    return () => { active = false; };
  }, [grievanceId]);
  const events = data ? investigationEvents(data.trace_events, data.audit_events) : [];
  async function moreEvents(){
    const cursor=data?.event_page?.next;if(!data||!cursor||busy)return;
    setBusy(true);setError('');
    try{const next=await getGrievanceInvestigation(grievanceId,{snapshot_at:data.event_page!.snapshot_at,after_time:cursor.time,after_id:cursor.id,after_source:cursor.source,include_records:false});
     setData(previous=>previous?.grievance_id===grievanceId?{...previous,event_page:next.event_page,
      trace_events:[...new Map([...previous.trace_events,...next.trace_events].map(e=>[e.id,e])).values()],
      audit_events:[...new Map([...previous.audit_events,...next.audit_events].map(e=>[e.id,e])).values()]}:previous);
    }catch{setError(tr('ui.unableToLoadTheNextEvidence'));}finally{setBusy(false);}
  }
  async function moreReferences(){
    if(!data?.record_page||!data.event_page||busy)return;
    setBusy(true);setError('');
    try{const next=await getGrievanceInvestigation(grievanceId,{snapshot_at:data.event_page.snapshot_at,include_events:false,record_offset:data.record_page.offset+data.record_page.limit});
      setData(previous=>{
       if(previous?.grievance_id!==grievanceId)return previous;
       const records={...previous.records};for(const [kind,rows]of Object.entries(next.records))records[kind]=[...new Map([...(records[kind]??[]),...rows].map(row=>[String(row.id??`${row.message_id}:${row.user_id}`),row])).values()];
       return {...previous,records,record_page:next.record_page};
      });
    }catch{setError(tr('ui.unableToLoadTheNextRecord'));}finally{setBusy(false);}
  }
  async function reschedule() {
    if (!data || busy) return;
    setBusy(true); setError('');
    try {
      await rescheduleAppointment(String(data.facts.appointment_id), slot.trim(), reason.trim(), grievanceId);
      setSlot(''); setReason(''); await load();
    } catch (e) { setError(apiErrorMessage(e, tr('ui.rescheduleWasNotConfirmed'))); }
    finally { setBusy(false); }
  }
  return <section aria-label={tr('ui.consultationInvestigationTimeline')}>
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
      <h3>{tr('ui.consultationInvestigationTimeline')}</h3>
      <button className="admin-btn-sec" disabled={busy} onClick={() => void load()}>{tr('ui.refreshEvidence')}</button>
    </div>
    {busy && <p role="status">{tr('ui.retrievingAuthoritativeEvidence')}</p>}
    {error && <p role="alert">{error}</p>}
    {data && <>
      <p>{data.semantics}</p>
      <p>{tr('mix.tracePreservation')}{data.preservation_holds.some(h => !h.release_at) ? tr('ui2.activeGrievanceHoldAutomaticCleanupIs') : tr('ui2.noActiveHoldRecorded')}</p>
      <details open><summary>{tr('ui.authoritativeConsultationFacts')}</summary><dl>
        {Object.entries(data.facts).map(([key, fact]) => <div key={key} style={{ overflowWrap: 'anywhere', margin: '8px 0' }}><dt>{key.replaceAll('_', ' ')}</dt><dd>{value(fact)}</dd></div>)}
      </dl></details>
      <p>{tr('mix.eventsLoaded', { count: events.length, total: data.event_page?.total ?? events.length })}</p>
      <ol style={{ paddingLeft: 20 }}>{events.map(e => <li key={`${e.source}:${e.id}`} style={{ margin: '12px 0', padding: 12, borderBottom: '1px solid #E2E8F0', overflowWrap: 'anywhere' }}>
        <strong>{e.event_type || e.action}</strong> · {e.result || tr('ui2.recorded')}
        <div><time>{e.event_time_server || e.created_at}</time> · {e.source}</div>
        <div>{tr('mix.accountRole', { account: e.actor_user_id || tr('ui2.system'), role: e.actor_role || tr('ui2.notRecorded') })}</div>
        <div>{tr('mix.sessionLabel')}{e.auth_session_id || tr('ui2.notRecorded')}</div>
        <div>{tr('mix.requestLabel')}{e.request_id || tr('ui2.notRecorded')} · Trace: {e.trace_id || tr('ui2.notRecorded')}</div>
        <div>{tr('mix.resourceLabel')}{e.resource_type || tr('ui2.notRecorded')} · {e.resource_id || tr('ui2.notRecorded')}</div>
        {e.reason_code && <div>{tr('mix.reasonLabel')}{e.reason_code}</div>}
        {(e.client_type || e.client_build) && <div>{tr('mix.clientBuild', { client: e.client_type || tr('ui2.unreported'), build: e.client_build || tr('ui2.unreported') })}</div>}
        {e.safe_metadata && <pre style={{ whiteSpace: 'pre-wrap' }}>{JSON.stringify(e.safe_metadata, null, 2)}</pre>}
        {e.expires_at && <div>{tr('mix.traceExpiry', { at: e.expires_at })}</div>}
        <div>{tr('mix.eventId', { id: e.id })}</div>
        <details><summary>{tr('ui.completeEventDetails')}</summary><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify(e, null, 2)}</pre></details>
      </li>)}</ol>
      {data.event_page?.has_more&&<button className="admin-btn-sec" disabled={busy} onClick={()=>void moreEvents()}>{tr('ui.loadMoreTimelineEvents')}</button>}
      <h4>{tr('ui.persistedRecordReferences')}</h4>
      {Object.entries(data.records).map(([kind, rows]) => <details key={kind}><summary>{kind.replaceAll('_', ' ')} ({rows.length} of {data.record_page?.totals[kind] ?? rows.length})</summary>
        {rows.length ? rows.map((row, i) => <dl key={i} style={{ padding: 10, overflowWrap: 'anywhere' }}>{Object.entries(row).map(([key, entry]) => <div key={key}><dt>{key.replaceAll('_', ' ')}</dt><dd>{value(entry)}</dd></div>)}</dl>) : <p>{tr('ui.noRecords')}</p>}
      </details>)}
      {Object.entries(data.record_page?.totals??{}).some(([kind,total])=>(data.records[kind]?.length??0)<total)&&<button className="admin-btn-sec" disabled={busy} onClick={()=>void moreReferences()}>{tr('ui.loadMoreRecordReferences')}</button>}
      <details><summary>{tr('ui.preservationHoldRecords')}</summary><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify(data.preservation_holds, null, 2)}</pre></details>
      <details><summary>{tr('ui.clientReportedTelemetryAndCorrelatedErro')}</summary><p>{tr('ui.clientTelemetryIsNotProofOf')}</p><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify(data.telemetry, null, 2)}</pre></details>
      <details><summary>{tr('ui.providerConfirmedPresenceEvidence')}</summary><p>{tr('ui.signedAgoraProviderCallbacksAreAuthorita')}</p><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify({
        recovery_state: data.telemetry?.recovery_state,
        empty_room_deadline: data.telemetry?.empty_room_deadline,
        interruption_reason: data.telemetry?.interruption_reason,
        patient_present: data.telemetry?.patient_present,
        doctor_present: data.telemetry?.doctor_present,
        patient_presence_sequence: data.telemetry?.patient_presence_sequence,
        doctor_presence_sequence: data.telemetry?.doctor_presence_sequence,
        provider_presence_events: data.telemetry?.provider_presence_events,
        recovery_resolution: data.telemetry?.recovery_resolution,
      }, null, 2)}</pre></details>
      {['CONFIRMED', 'WAITING'].includes(String(data.facts.status)) && <form onSubmit={e => { e.preventDefault(); void reschedule(); }}>
        <h4>{tr('ui.adminControlledReschedule')}</h4>
        <p>{tr('ui.theOriginalAndReplacementTimeYour')}</p>
        <label>{tr('ui.replacementAppointmentTime')}<select className="auth-input" required value={slot} onChange={e=>setSlot(e.target.value)}><option value="">{tr('ui.chooseAnAvailableSlot')}</option>{slots.map(s=><option key={s.id} value={s.id}>{new Date(s.start_time).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})}</option>)}</select></label>
        <label>{tr('ui.reason')}<textarea className="auth-input" required minLength={8} maxLength={300} value={reason} onChange={e => setReason(e.target.value)} /></label>
        <button className="admin-btn-pri" disabled={busy}>{tr('ui.rescheduleConsultation')}</button>
      </form>}
    </>}
  </section>;
}
