'use client';
import { consultationAction, consultationActionLabel } from '../lib/contracts';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, ApiError, errorText, runtimeGeneration } from '../lib/client';
import { Appointment, Slot } from '../lib/contracts';
import { loadDoctorSlots } from '../lib/slot-list';
import { startScheduleSync } from '../lib/schedule-sync';
import { Avatar, Badge, Modal, PageHeader, State, type Tone } from './Common';
import { Icon } from './Icons';
import { ClinicalAction, date, dayKey, time } from './Queue';
import { EAT_NOTE } from '../lib/eat';

type Intent = 'block' | 'unblock' | 'cancel';
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
  const todayKey=dayKey(new Date().toISOString());
  const [filter,setFilter]=useState('all');const [selectedDay,setDay]=useState(initialDate||todayKey);const slots=useScheduleSlots(selectedDay);
  const [pending,setPending]=useState('');const lock=useRef(false);const [confirm,setConfirm]=useState<{slot:Slot;intent:Intent}|null>(null);const [error,setError]=useState('');const [visible,setVisible]=useState(50);
  const close=useCallback(()=>{if(!lock.current)setConfirm(null);},[]);
  useEffect(()=>setVisible(50),[filter,selectedDay]);
  const shift=(days:number)=>{const d=new Date(`${selectedDay}T12:00:00+03:00`);d.setUTCDate(d.getUTCDate()+days);setDay(dayKey(d.toISOString()));};
  const strip=Array.from({length:7},(_,i)=>{const d=new Date(`${selectedDay}T12:00:00+03:00`);d.setUTCDate(d.getUTCDate()+i-3);return dayKey(d.toISOString());});
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
  const ofDay=slots.data.filter(s=>dayKey(s.start_time)===selectedDay);
  const count=(status:string)=>ofDay.filter(s=>s.status===status).length;
  const active=ofDay.filter(s=>s.status!=='CANCELLED');
  const booked=count('BOOKED')+count('LOCKED_IN_PAYMENT');
  const utilization=active.length?Math.round(booked/active.length*100):0;
  const filtered=ofDay.filter(s=>filter==='all'||s.status.toLowerCase()===filter);
  const availableSelected=count('AVAILABLE');
  return <>
    <PageHeader title="Consultation schedule" subtitle={<>Open, block or cancel your availability. Booked slots are locked. All times in {EAT_NOTE}.</>}>
      <button className="am-btn ghost" disabled={!!pending||slots.loading} onClick={()=>void slots.refresh()}><Icon name="refresh" size={16}/>Refresh</button>
      <button className="am-btn primary" disabled={!!pending} onClick={addSlot}><Icon name="plus" size={17}/>Add availability</button>
    </PageHeader>
    <section className="am-card pad am-stack" style={{gap:18}}>
      <div className="am-row between">
        <div className="am-row">
          <button className="am-btn ghost sm" aria-label="Previous day" onClick={()=>shift(-1)}><Icon name="chevronLeft" size={16}/></button>
          <label><span className="am-sr">Date</span><input className="am-input" type="date" value={selectedDay} style={{minHeight:36,padding:'4px 10px'}} onChange={e=>{if(e.target.value)setDay(e.target.value);}}/></label>
          <button className="am-btn ghost sm" aria-label="Next day" onClick={()=>shift(1)}><Icon name="chevronRight" size={16}/></button>
          <button className="am-btn soft sm" disabled={selectedDay===todayKey} onClick={()=>setDay(todayKey)}>Today</button>
        </div>
        <span className="am-faint" role="status" aria-live="polite">{slots.loading?'Refreshing schedule…':slots.error?'Schedule may be out of date':slots.updatedAt?`Last synced ${time(slots.updatedAt)} EAT`:''}</span>
      </div>
      <div className="am-days">{strip.map(d=>{const day=new Date(d+'T12:00:00Z');return <button key={d} className="am-day" aria-pressed={selectedDay===d} aria-label={date(d+'T12:00:00Z')} onClick={()=>setDay(d)}><small>{d===todayKey?'Today':day.toLocaleDateString('en-KE',{weekday:'short',timeZone:'Africa/Nairobi'})}</small><strong>{day.toLocaleDateString('en-KE',{day:'numeric',timeZone:'Africa/Nairobi'})}</strong></button>;})}</div>
      <div className="am-row between" style={{paddingTop:16,borderTop:'1px solid var(--am-line-soft)'}}>
        <div className="am-mini-stats">
          <div className="am-mini-stat"><strong>{active.length}</strong><span>Total slots</span></div>
          <div className="am-mini-stat"><strong style={{color:'var(--am-blue-fg)'}}>{booked}</strong><span>Booked</span></div>
          <div className="am-mini-stat"><strong style={{color:'var(--am-green)'}} data-testid="available-selected-date">{slots.loading?'—':availableSelected}</strong><span>Available</span></div>
          <div className="am-mini-stat"><strong style={{color:'var(--am-amber-fg)'}}>{count('BLOCKED')}</strong><span>Blocked</span></div>
        </div>
        <div style={{minWidth:220,flex:'0 1 300px'}}><div className="am-row between" style={{marginBottom:6}}><span className="am-label">Booked share</span><strong className="am-strong">{utilization}%</strong></div><div className="am-progress" role="progressbar" aria-valuenow={utilization} aria-valuemin={0} aria-valuemax={100} aria-label="Booked share of open slots"><i style={{width:`${utilization}%`}}/></div></div>
      </div>
    </section>
    <State error={error}/>
    {confirm&&<Modal title={confirm.intent==='block'?'Block this slot?':confirm.intent==='unblock'?'Make this slot available?':'Cancel this slot?'} onClose={close}><div className="am-stack">
      <div className="am-callout"><Icon name="clock"/><div><strong className="am-strong">{time(confirm.slot.start_time)} – {time(confirm.slot.end_time)}</strong><p className="am-muted">{date(confirm.slot.start_time)}</p></div></div>
      <p className="am-muted">{confirm.intent==='block'?'Patients will not be able to book this slot until you make it available again.':confirm.intent==='unblock'?'Patients will be able to book this slot again.':'This removes the slot from your schedule. To pause it temporarily, block it instead.'}</p>
      <div className="am-row end"><button className="am-btn ghost" onClick={close} disabled={!!pending}>Keep unchanged</button><button className={`am-btn ${confirm.intent==='cancel'?'danger-solid':'primary'}`} onClick={()=>void mutate(confirm.slot,confirm.intent)} disabled={!!pending}>{pending?'Updating…':confirm.intent==='block'?'Block slot':confirm.intent==='unblock'?'Make available':'Cancel slot'}</button></div>
    </div></Modal>}
    <section className="am-card">
      <div className="am-card-head"><h2><Icon name="clock"/>{date(selectedDay+'T12:00:00Z')}</h2>
        <div className="am-tabs" aria-label="Filter slots">{[['all','All'],['available','Available'],['booked','Booked'],['blocked','Blocked'],['cancelled','Cancelled']].map(([value,label])=><button key={value} className="am-tab" aria-pressed={filter===value} onClick={()=>setFilter(value)}>{label}</button>)}</div>
      </div>
      <State loading={slots.loading&&!slots.data.length} error={slots.error} retry={()=>void slots.refresh()} empty={!slots.loading&&!slots.error&&filtered.length===0?(filter==='all'?'No slots on this day':'No slots match this filter'):undefined} hint={filter==='all'?'Add availability so patients can book you.':undefined}/>
      {filtered.length>0&&<div className="am-table-wrap"><table className="am-table">
        <thead><tr><th>Time</th><th>Status</th><th>Patient / details</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
        <tbody>{filtered.slice(0,visible).map(s=><SlotRow key={s.id} s={s} a={appointments.find(a=>a.id===s.appointment_id)} pending={!!pending} confirm={intent=>setConfirm({slot:s,intent})} action={action} gap={gap}/>)}</tbody>
      </table></div>}
      {filtered.length>visible&&<div className="am-card-body am-row" style={{justifyContent:'center'}}><button className="am-btn ghost" onClick={()=>setVisible(v=>v+50)}>Show more slots</button></div>}
    </section>
  </>;
}
const slotTone:Record<string,[string,Tone]>={AVAILABLE:['Available','green'],BOOKED:['Booked','blue'],BLOCKED:['Blocked','amber'],LOCKED_IN_PAYMENT:['Payment pending','violet'],CANCELLED:['Cancelled','slate']};
function SlotRow({s,a,pending,confirm,action,gap}:{s:Slot;a?:Appointment;pending:boolean;confirm:(intent:Intent)=>void;action:ClinicalAction;gap:(message:string)=>void}) {
  const [label,tone]=slotTone[s.status]??[s.status.replaceAll('_',' '),'slate'];
  const detail=s.status==='AVAILABLE'?(s.bookable?'Open for booking':'Start time has passed'):s.status==='BLOCKED'?'Not bookable until you reopen it':s.status==='LOCKED_IN_PAYMENT'?'Reserved while a booking completes':s.status==='CANCELLED'?'Removed from your schedule':null;
  return <tr data-slot-id={s.id} data-slot-status={s.status} style={s.status==='CANCELLED'?{opacity:.65}:undefined}>
    <td className="nowrap"><strong className="am-strong">{time(s.start_time)} – {time(s.end_time)}</strong></td>
    <td><Badge tone={tone} dot>{label}</Badge></td>
    <td>{s.status==='BOOKED'&&s.patient_display_name?<div className="am-person"><Avatar name={s.patient_display_name} size="sm"/><div><strong>{s.patient_display_name}</strong><span>{a?`${a.modality==='VIDEO'?'Video':'Chat'} consultation · ${a.appointment_number}`:'Booked consultation'}</span></div></div>:<span className="am-muted">{detail}</span>}</td>
    <td><div className="am-row end" style={{flexWrap:'nowrap'}}>
      {(s.status==='AVAILABLE'||s.status==='BLOCKED')&&<><button className="am-btn ghost sm" disabled={pending} onClick={()=>confirm(s.status==='AVAILABLE'?'block':'unblock')}><Icon name={s.status==='AVAILABLE'?'ban':'check'} size={15}/>{s.status==='AVAILABLE'?'Block':'Reopen'}</button><button className="am-btn danger sm" disabled={pending} onClick={()=>confirm('cancel')}>Cancel</button></>}
      {s.status==='BOOKED'&&<button className="am-btn primary sm" disabled={!a||!consultationAction(a)} title={a?consultationActionLabel(a):undefined} onClick={()=>a&&consultationAction(a)?action(consultationAction(a)!,a):gap('Refresh appointments to load this consultation.')}><Icon name="video" size={15}/>{a&&consultationAction(a)?'Start':'Not open yet'}</button>}
      {s.status==='BOOKED'&&a&&<button className="am-btn ghost sm" onClick={()=>action('summary',a)}>Details</button>}
    </div></td>
  </tr>;
}
