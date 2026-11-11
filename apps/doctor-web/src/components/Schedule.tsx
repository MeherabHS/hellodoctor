'use client';
import { consultationAction, consultationActionLabel } from '../lib/contracts';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, ApiError, errorText, runtimeGeneration } from '../lib/client';
import { Appointment, Slot } from '../lib/contracts';
import { loadDoctorSlots } from '../lib/slot-list';
import { startScheduleSync } from '../lib/schedule-sync';
import { Modal, State } from './Common';
import { ClinicalAction, date, dayKey, initials, time } from './Queue';
import { EAT_NOTE, eatRange } from '../lib/eat';

type Intent = 'block' | 'unblock' | 'cancel';
function SlotIcon({status}:{status:string}) {
  return <span className="web-slot-icon" aria-hidden="true"><svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
    {status==='AVAILABLE'?<><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></>:status==='LOCKED_IN_PAYMENT'?<path d="M6 3h12M6 21h12M7 3v4l5 5 5-5V3M7 21v-4l5-5 5 5v4"/>:status==='CANCELLED'?<><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M7 3v4M17 3v4M3 10h18M9 13l6 5M15 13l-6 5"/></>:<><rect x="4" y="10" width="16" height="11" rx="2"/><path d="M8 10V6a4 4 0 0 1 8 0v4M12 14v3"/></>}
  </svg></span>;
}
function useScheduleSlots(day: string) {
  const [data,setData]=useState<Slot[]>([]);const [loading,setLoading]=useState(true);const [error,setError]=useState('');const [updatedAt,setUpdatedAt]=useState('');const epoch=useRef(0);
  const refresh=useCallback(async()=>{
    const job=++epoch.current;const version=runtimeGeneration();const current=()=>job===epoch.current&&version===runtimeGeneration();
    setLoading(true);setError('');
    try {const rows=await loadDoctorSlots(path=>api<Slot[]>(path),current,day);if(current()){setData(rows);setUpdatedAt(new Date().toISOString());}}
    catch(e){if(current()&&!(e instanceof DOMException&&e.name==='AbortError'))setError('Schedule refresh failed. Displayed slots may be out of date. '+errorText(e));}
    finally {if(current())setLoading(false);}
  },[day]);
  useEffect(()=>{
    void refresh();
    const stop=startScheduleSync(refresh,{window,document,online:()=>navigator.onLine,
      every:(callback,ms)=>window.setInterval(callback,ms),cancel:timer=>window.clearInterval(timer as number)});
    return()=>{stop();epoch.current++;};
  },[refresh]);
  return {data,loading,error,updatedAt,refresh};
}
export function Schedule({ appointments, action, addSlot, gap, initialDate }: { appointments:Appointment[]; action:ClinicalAction; addSlot:()=>void; gap:(message:string)=>void; initialDate?:string }) {
  const [filter,setFilter]=useState('all');const [selectedDay,setDay]=useState(initialDate||dayKey(new Date().toISOString()));const slots=useScheduleSlots(selectedDay);
  const [pending,setPending]=useState('');const lock=useRef(false);const [confirm,setConfirm]=useState<{slot:Slot;intent:Intent}|null>(null);const [error,setError]=useState('');const [visible,setVisible]=useState(50);
  const close=useCallback(()=>{if(!lock.current)setConfirm(null);},[]);
  useEffect(()=>setVisible(50),[filter,selectedDay]);
  const dates=Array.from({length:14},(_,i)=>{const d=new Date();d.setUTCDate(d.getUTCDate()+i);return dayKey(d.toISOString());});
  async function mutate(slot:Slot,intent:Intent){
    if(lock.current)return;lock.current=true;setPending(slot.id);setError('');const version=runtimeGeneration();
    try {
      await api(`doctor/slots/${slot.id}`,intent==='cancel'?{method:'DELETE'}:{method:'PATCH',body:JSON.stringify({status:intent==='block'?'BLOCKED':'AVAILABLE'})});
      if(version!==runtimeGeneration())return;
      setConfirm(null);await slots.refresh();
    } catch(e){
      if(version!==runtimeGeneration())return;
      if(e instanceof ApiError&&e.status===409){setConfirm(null);setError((intent==='block'?'This slot is no longer available to block.':intent==='unblock'?'This slot can no longer be made available.':'This slot can no longer be cancelled.')+' Refreshing schedule.'+(e.reference?` Reference: ${e.reference}`:''));}
      else setError(errorText(e));
      await slots.refresh();
    } finally {lock.current=false;if(version===runtimeGeneration())setPending('');}
  }
  const filtered=slots.data.filter(s=>dayKey(s.start_time)===selectedDay&&(filter==='all'||s.status.toLowerCase()===filter));
  const availableSelected=slots.data.filter(s=>dayKey(s.start_time)===selectedDay&&s.status==='AVAILABLE').length;
  return <section className="dw-page-view active" id="webScheduleWorkspace">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>Consultation Schedule</h2><p>Availability &middot; East Africa Time &middot; Booked slots are locked</p></div><div className="web-action-row"><button className="dw-btn-outline" disabled={!!pending||slots.loading} onClick={()=>void slots.refresh()}>Refresh</button><button className="dw-btn-primary" disabled={!!pending} onClick={addSlot}>+ Add Schedule Slot</button></div></header>
    <p className="sheet-desc" role="status" aria-live="polite">{slots.loading?'Refreshing schedule…':slots.error?'Schedule may be out of date':slots.updatedAt?`Last synced ${date(slots.updatedAt)} ${time(slots.updatedAt)} EAT`:''}. Available means open for booking; it is not a patient booking. All times are shown in {EAT_NOTE}.</p>
    <div className="dw-section-card"><div className="web-section-body"><div className="web-action-row"><label>Date<input className="auth-input" type="date" value={selectedDay} onChange={e=>{if(e.target.value)setDay(e.target.value);}}/></label><strong>{date(selectedDay+'T12:00:00Z')}</strong><span className="badge badge-green" data-testid="available-selected-date">{slots.loading?'Loading availability…':slots.error?'Availability could not be loaded':`${availableSelected} available on this date`}</span></div><div className="web-date-strip web-action-row">{dates.map(d=><button key={d} className={`dw-filter-pill ${selectedDay===d?'active':''}`} onClick={()=>setDay(d)} aria-pressed={selectedDay===d}>{new Date(d+'T12:00:00Z').toLocaleDateString('en-KE',{weekday:'short',day:'numeric',timeZone:'Africa/Nairobi'})}</button>)}</div></div></div>
    <div className="dw-filter-pills">{['all','available','blocked','booked','cancelled'].map(value=><button key={value} className={`dw-filter-pill ${filter===value?'active':''}`} aria-pressed={filter===value} onClick={()=>setFilter(value)}>{value[0].toUpperCase()+value.slice(1)}</button>)}</div>
    <State error={error}/>
    {confirm&&<Modal title={confirm.intent==='block'?'Block this slot?':confirm.intent==='unblock'?'Make this slot available?':'Cancel this slot?'} onClose={close}><p className="sheet-desc">{time(confirm.slot.start_time)}–{time(confirm.slot.end_time)} · {date(confirm.slot.start_time)}</p><p className="sheet-desc">{confirm.intent==='block'?'Patients will not be able to book this slot until you make it available again.':confirm.intent==='unblock'?'Patients will be able to book this slot again.':'This removes the slot from your active schedule. Cancellation is separate from temporarily blocking availability.'}</p><div className="web-action-row"><button className="dw-btn-outline" onClick={close} disabled={!!pending}>Keep unchanged</button><button className={`dw-btn-primary ${confirm.intent==='cancel'?'web-slot-cancel-action':''}`} onClick={()=>void mutate(confirm.slot,confirm.intent)} disabled={!!pending}>{pending?'Updating…':confirm.intent==='block'?'Block slot':confirm.intent==='unblock'?'Make available':'Confirm cancellation'}</button></div></Modal>}
    <div className="dw-card-grid"><State loading={slots.loading} error={slots.error} empty={!slots.loading&&!slots.error&&filtered.length===0?'No slots for this date':undefined} retry={()=>void slots.refresh()}/>{filtered.slice(0,visible).map(s=>{
      const a=appointments.find(a=>a.id===s.appointment_id);
      const meaning=s.status==='AVAILABLE'?(s.bookable?'Open for booking':'Past opening'):s.status==='BLOCKED'?'Blocked – unavailable for booking':s.status==='LOCKED_IN_PAYMENT'?'Booking in progress':s.status==='CANCELLED'?'Slot cancelled':s.patient_display_name??'Booked consultation';
      return <article className={`dw-pt-card web-slot-${s.status.toLowerCase()}`} key={s.id} data-slot-id={s.id} data-slot-status={s.status}><div className="web-slot-heading"><strong>{eatRange(s.start_time,s.end_time)}</strong><span className={`badge web-slot-badge-${s.status.toLowerCase()}`}>{s.status==='LOCKED_IN_PAYMENT'?'PAYMENT PENDING':s.status==='BOOKED'?'BOOKED / LOCKED':s.status.replaceAll('_',' ')}</span></div><div className="web-slot-context">{s.status==='BOOKED'&&s.patient_display_name?<span className="dw-doc-avatar-img">{initials(s.patient_display_name)}</span>:<SlotIcon status={s.status}/>}<div><strong>{meaning}</strong><p className="sheet-desc">{date(s.start_time)}</p>{s.status==='BOOKED'&&a&&<p className="sheet-desc">{a.modality==='VIDEO'?'Video consultation':'Consultation chat'} · {a.appointment_number}</p>}</div></div>{s.status==='LOCKED_IN_PAYMENT'&&<p className="sheet-desc">This slot is temporarily reserved while booking is completed.</p>}<div className="web-action-row">{s.status==='AVAILABLE'||s.status==='BLOCKED'?<><button className="dw-btn-outline-sm" disabled={!!pending} onClick={()=>setConfirm({slot:s,intent:s.status==='AVAILABLE'?'block':'unblock'})}>{s.status==='AVAILABLE'?'Block slot':'Make available'}</button><button className="dw-btn-outline-sm web-slot-cancel-link" onClick={()=>setConfirm({slot:s,intent:'cancel'})} disabled={!!pending}>Cancel slot</button></>:s.status==='BOOKED'?<><button className="dw-btn-primary" disabled={!a || !consultationAction(a)} onClick={()=>a&&consultationAction(a)?action(consultationAction(a)!,a):gap('Refresh appointments to load the authorized consultation context.')}>{a?consultationActionLabel(a):'Load consultation'}</button>{a?.modality==='VIDEO'&&<button className="dw-btn-outline-sm" onClick={()=>action('chat',a)}>Follow-up messages</button>}</>:null}</div></article>;
    })}</div>
    {filtered.length>visible&&<button className="dw-btn-outline" onClick={()=>setVisible(v=>v+50)}>Load more slots</button>}
  </section>;
}

