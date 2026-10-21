'use client';
import { useCallback, useEffect, useState } from 'react';
import { investigationEvents } from '@/lib/investigation';
import { getGrievanceInvestigation, GrievanceInvestigation as Investigation, rescheduleAppointment, getBookableDoctorSlots, BookableDoctorSlot } from '@/lib/api';

function value(data: unknown): string {
  if (data === null || data === undefined) return 'Not recorded';
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
  useEffect(()=>{if(!doctorId||!['CONFIRMED','WAITING'].includes(String(data?.facts.status)))return;let active=true;getBookableDoctorSlots(String(doctorId)).then(rows=>{if(active)setSlots(rows);}).catch(()=>{if(active)setError('Unable to load replacement availability. Refresh evidence to retry.');});return()=>{active=false;};},[doctorId,data?.facts.status,data?.event_page?.snapshot_at]);
  const load = useCallback(async () => {
    setBusy(true); setError('');
    try { setData(await getGrievanceInvestigation(grievanceId)); }
    catch (e) { setError(e instanceof Error ? e.message : 'Unable to retrieve investigation evidence.'); }
    finally { setBusy(false); }
  }, [grievanceId]);
  useEffect(() => { let active = true;
    getGrievanceInvestigation(grievanceId).then(result => { if (active) setData(result); })
      .catch(e => { if (active) setError(e instanceof Error ? e.message : 'Unable to retrieve investigation evidence.'); })
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
    }catch{setError('Unable to load the next evidence page. Please retry.');}finally{setBusy(false);}
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
    }catch{setError('Unable to load the next record-reference page. Please retry.');}finally{setBusy(false);}
  }
  async function reschedule() {
    if (!data || busy) return;
    setBusy(true); setError('');
    try {
      await rescheduleAppointment(String(data.facts.appointment_id), slot.trim(), reason.trim(), grievanceId);
      setSlot(''); setReason(''); await load();
    } catch (e) { setError(e instanceof Error ? e.message : 'Reschedule was not confirmed.'); }
    finally { setBusy(false); }
  }
  return <section aria-label="Consultation investigation timeline">
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
      <h3>Consultation investigation timeline</h3>
      <button className="admin-btn-sec" disabled={busy} onClick={() => void load()}>Refresh evidence</button>
    </div>
    {busy && <p role="status">Retrieving authoritative evidence…</p>}
    {error && <p role="alert">{error}</p>}
    {data && <>
      <p>{data.semantics}</p>
      <p>Trace preservation: {data.preservation_holds.some(h => !h.release_at) ? 'Active grievance hold — automatic cleanup is suspended for this consultation.' : 'No active hold recorded.'}</p>
      <details open><summary>Authoritative consultation facts</summary><dl>
        {Object.entries(data.facts).map(([key, fact]) => <div key={key} style={{ overflowWrap: 'anywhere', margin: '8px 0' }}><dt>{key.replaceAll('_', ' ')}</dt><dd>{value(fact)}</dd></div>)}
      </dl></details>
      <p>{events.length} of {data.event_page?.total ?? events.length} recorded trace and audit events loaded. Times below are server timestamps. Missing events are not evidence that an action occurred.</p>
      <ol style={{ paddingLeft: 20 }}>{events.map(e => <li key={`${e.source}:${e.id}`} style={{ margin: '12px 0', padding: 12, borderBottom: '1px solid #E2E8F0', overflowWrap: 'anywhere' }}>
        <strong>{e.event_type || e.action}</strong> · {e.result || 'Recorded'}
        <div><time>{e.event_time_server || e.created_at}</time> · {e.source}</div>
        <div>Account: {e.actor_user_id || 'System'} · Role: {e.actor_role || 'Not recorded'}</div>
        <div>Session: {e.auth_session_id || 'Not recorded'}</div>
        <div>Request: {e.request_id || 'Not recorded'} · Trace: {e.trace_id || 'Not recorded'}</div>
        <div>Resource: {e.resource_type || 'Not recorded'} · {e.resource_id || 'Not recorded'}</div>
        {e.reason_code && <div>Reason: {e.reason_code}</div>}
        {(e.client_type || e.client_build) && <div>Client: {e.client_type || 'Unreported'} · Build: {e.client_build || 'Unreported'}</div>}
        {e.safe_metadata && <pre style={{ whiteSpace: 'pre-wrap' }}>{JSON.stringify(e.safe_metadata, null, 2)}</pre>}
        {e.expires_at && <div>Normal trace expiry: {e.expires_at} (subject to preservation hold)</div>}
        <div>Event ID: {e.id}</div>
        <details><summary>Complete event details</summary><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify(e, null, 2)}</pre></details>
      </li>)}</ol>
      {data.event_page?.has_more&&<button className="admin-btn-sec" disabled={busy} onClick={()=>void moreEvents()}>Load more timeline events</button>}
      <h4>Persisted record references</h4>
      {Object.entries(data.records).map(([kind, rows]) => <details key={kind}><summary>{kind.replaceAll('_', ' ')} ({rows.length} of {data.record_page?.totals[kind] ?? rows.length})</summary>
        {rows.length ? rows.map((row, i) => <dl key={i} style={{ padding: 10, overflowWrap: 'anywhere' }}>{Object.entries(row).map(([key, entry]) => <div key={key}><dt>{key.replaceAll('_', ' ')}</dt><dd>{value(entry)}</dd></div>)}</dl>) : <p>No records.</p>}
      </details>)}
      {Object.entries(data.record_page?.totals??{}).some(([kind,total])=>(data.records[kind]?.length??0)<total)&&<button className="admin-btn-sec" disabled={busy} onClick={()=>void moreReferences()}>Load more record references</button>}
      <details><summary>Preservation hold records</summary><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify(data.preservation_holds, null, 2)}</pre></details>
      <details><summary>Client-reported telemetry and correlated error references</summary><p>Client telemetry is not proof of provider joins, media, delivery, or human comprehension.</p><pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{JSON.stringify(data.telemetry, null, 2)}</pre></details>
      {['CONFIRMED', 'WAITING'].includes(String(data.facts.status)) && <form onSubmit={e => { e.preventDefault(); void reschedule(); }}>
        <h4>Admin-controlled reschedule</h4>
        <p>The original and replacement time, your account, and this grievance are preserved in the change history.</p>
        <label>Replacement appointment time<select className="auth-input" required value={slot} onChange={e=>setSlot(e.target.value)}><option value="">Choose an available slot</option>{slots.map(s=><option key={s.id} value={s.id}>{new Date(s.start_time).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})}</option>)}</select></label>
        <label>Reason<textarea className="auth-input" required minLength={8} maxLength={300} value={reason} onChange={e => setReason(e.target.value)} /></label>
        <button className="admin-btn-pri" disabled={busy}>Reschedule consultation</button>
      </form>}
    </>}
  </section>;
}
