'use client';
import { isProfessionallyVerified, professionalVerificationLabel } from '../lib/professionalVerification';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, errorText, runtimeGeneration, useResource } from '../lib/client';
import { startScheduleSync } from '../lib/schedule-sync';
import { Appointment, Profile, upcomingFirst, consultationAction, consultationActionLabel } from '../lib/contracts';
import { Prototype, hasClass, id, text } from './Prototype';
import { State } from './Common';

export type ClinicalAction = (action: 'summary' | 'history' | 'documents' | 'call' | 'chat' | 'rx', appointment: Appointment) => void;
export type QueueState = ReturnType<typeof useAppointments>;
export function useAppointments(enabled: boolean, view: 'upcoming'|'all'='upcoming', live=true) {
  const [data,setData] = useState<Appointment[]>([]);
  const [loading,setLoading] = useState(enabled);
  const [error,setError] = useState('');
  const epoch = useRef(0);
  const loadedPages=useRef(1);
  const [hasMore,setHasMore]=useState(false);
  const [moreBusy,setMoreBusy]=useState(false);
  const refresh = useCallback(async () => {
    if (!enabled) return;
    const version = ++epoch.current;
    const generation = runtimeGeneration();
    const current = () => version === epoch.current && generation === runtimeGeneration();
    setError(''); setLoading(true);
    try {
      const results:Appointment[]=[];let more=false;
      for(let page=0;page<loadedPages.current;page++){const batch=await api<Appointment[]>(`doctor/appointments?view=${view}&limit=100&offset=${page*100}`);if(!current())return;results.push(...batch);more=batch.length===100;if(!more)break;}
      if(current()){setData(upcomingFirst(results));setHasMore(more);}
    } catch (e) { if (current() && !(e instanceof DOMException && e.name === 'AbortError')) setError(errorText(e)); }
    finally { if (current()) setLoading(false); }
  }, [enabled,view]);
  useEffect(() => {
    if (!enabled) { epoch.current++; setData([]); return; }
    loadedPages.current=1;void refresh();
    const stop = live ? startScheduleSync(refresh, {window,document,online:()=>navigator.onLine,
      every:(callback,ms)=>window.setInterval(callback,ms),cancel:timer=>window.clearInterval(timer as number)}) : ()=>{};
    return () => { stop(); epoch.current++; };
  }, [enabled,refresh,live]);
  const loadMore=async()=>{
    if(!enabled||moreBusy||!hasMore)return;
    const version=epoch.current,generation=runtimeGeneration();setMoreBusy(true);
    try{const next=await api<Appointment[]>(`doctor/appointments?view=${view}&limit=100&offset=${data.length}`);
     if(version===epoch.current&&generation===runtimeGeneration()){loadedPages.current++;setData(old=>upcomingFirst([...new Map([...old,...next].map(a=>[a.id,a])).values()]));setHasMore(next.length===100);}
    }catch(e){if(version===epoch.current)setError(errorText(e));}finally{setMoreBusy(false);}
  };
  return { data,loading,error,refresh,hasMore,loadMore,moreBusy };
}
export function initials(name: string) { return name.trim().split(/\s+/).slice(0,2).map(x => x[0]).join('').toUpperCase() || '👤'; }
export function date(value: string | null | undefined) { return value ? new Date(value).toLocaleDateString('en-KE', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'Africa/Nairobi' }) : '—'; }
// Every HelloDoctor surface (Patient/Doctor Flutter, Admin Web, Doctor Web)
// renders the same fixed Africa/Nairobi wall-clock for a given appointment
// regardless of the viewer's own device timezone — this is a single-country
// deployment, so "9:00 AM" must mean the same instant to everyone, never a
// value that silently shifts if a Doctor happens to be travelling abroad.
// Only the 12-hour/AM-PM *presentation* (the `en-US` locale, `hour12`) is a
// readability choice; it never changes which Nairobi instant is shown.
export function time(value: string | null | undefined) { return value ? new Date(value).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit', hour12: true, timeZone: 'Africa/Nairobi' }) : '—'; }
export function dayKey(value: string) { return new Intl.DateTimeFormat('en-CA', { timeZone: 'Africa/Nairobi', year:'numeric',month:'2-digit',day:'2-digit' }).format(new Date(value)); }
export function statusLabel(a: Appointment, nextId?: string, now = Date.now()) {
  if (['COMPLETED','CANCELLED','REFUNDED','EXPIRED','PREMATURE_TERMINATION','DISPUTED'].includes(a.status) || a.doctor_lifecycle === 'DONE') return a.doctor_lifecycle.replaceAll('_',' ');
  const minutes = Math.ceil((Date.parse(a.start_time) - now) / 60000);
  if (a.status === 'IN_CONSULTATION') return 'In consultation';
  if (minutes > 0 && minutes <= 60) return `${a.id === nextId ? 'Next · ' : ''}Starts in ${minutes} min`;
  return `${a.id === nextId ? 'Next · ' : ''}${dayKey(a.start_time) === dayKey(new Date(now).toISOString()) ? 'Today' : 'Upcoming'}`;
}
export function QueueCard({ appointment: a, nextId, action }: { appointment: Appointment; nextId?: string; action: ClinicalAction }) {
  const primary = consultationAction(a);
  return <article className="queue-card" data-appointment-id={a.id} data-category={a.status}>
    <div className="web-action-row"><div className="doc-slot-avatar">{initials(a.patient_display_name)}</div><div>
      <button className="web-name-button" onClick={()=>action('summary',a)}>{a.patient_display_name}</button>
      <p className="sheet-desc">{a.modality==='CHAT'?'Chat consultation':'Video consultation'}</p>
      <p className="sheet-desc">{date(a.start_time)} · {time(a.start_time)} · {statusLabel(a,nextId)}</p>
    </div></div>
    {a.clinical_outcome&&<p className="queue-complaint">{a.clinical_outcome}</p>}
    <div className="web-action-row">
      <button className="dw-btn-primary" disabled={!primary} onClick={()=>primary&&action(primary,a)}>{consultationActionLabel(a)}</button>
      {a.modality==='VIDEO'&&<button className="dw-btn-outline-sm" onClick={()=>action('chat',a)}>Follow-up messages</button>}
      {primary&&<button className="dw-btn-outline-sm" onClick={()=>action('rx',a)}>Write prescription</button>}
    </div>
  </article>;
}
export function Queue({ queue, profile, duty, onDuty, action, onQna }: { queue: QueueState; profile: Profile; duty: { value?: boolean; busy:boolean; error:string }; onDuty:()=>void; action:ClinicalAction; onQna:()=>void }) {
  const [search,setSearch] = useState('');
  const [filter,setFilter] = useState('upcoming');
  const publicQuestions=useResource<unknown[]>('doctor/qna/questions?status=OPEN&limit=100&offset=0');
  const [visibleCount,setVisibleCount] = useState(25);
  const [,setTick] = useState(0);
  useEffect(() => { const timer=setInterval(()=>setTick(t=>t+1),30000); return ()=>clearInterval(timer); },[]);
  const active = queue.data.filter(a=>a.queue_actionable === true);
  const filtered = active.filter(a => `${a.patient_display_name} ${a.appointment_number} ${a.clinical_outcome ?? ''}`.toLowerCase().includes(search.toLowerCase()));

  return <Prototype name="doc-view-0" action={(handler,event) => {
    if (handler.includes('toggleDoctorDuty')) onDuty();
    else if (handler.includes('filterDoctorQueue')) setSearch((event.target as HTMLInputElement).value);
    else if (handler.includes('filterQueueList')) setFilter(handler.match(/'([^']+)'/)?.[1] ?? 'all');
    else if (handler.includes('switchDoctorTab(5)')) onQna();
    else void queue.refresh();
  }} override={n => {
    if (id(n)==='doc-view-0') return { props: { className:'screen-panel active', style:{display:'flex',flexDirection:'column',flex:1} } };
    if (hasClass(n,'doc-header-avatar')) return { children:initials(profile.full_name) };
    if (hasClass(n,'doc-name-txt')) return { children:profile.full_name };
    if (hasClass(n,'doc-registration-badge')) return { children:`${professionalVerificationLabel(profile)} · ${profile.license_number.startsWith('KMPDC')?'':'KMPDC '}${profile.license_number}` };
    if (n.tag==='span'&&text(n).includes('Unanswered')) return {children:publicQuestions.data?`${publicQuestions.data.length}${publicQuestions.data.length===100?'+':''} Unanswered`:'—'};
    if (id(n)==='docDutySwitch') return { tag:'button', props:{className:`duty-switch ${duty.value?'online':'offline'}`,disabled:duty.busy || duty.value===undefined,'aria-pressed':duty.value},children:<><div className="duty-dot"/><span id="dutyLabel">{duty.busy?'Updating…':duty.value?'Online':'Offline'}</span></> };
    if (id(n)==='doctorQueueList') return { children:<><State loading={queue.loading} error={queue.error || duty.error} empty={!queue.loading&&!queue.error&&filtered.length===0?'No waiting patients found':undefined} retry={()=>void queue.refresh()} />{filtered.slice(0,visibleCount).map(a=><QueueCard key={a.id} appointment={a} nextId={active[0]?.id} action={action} />)}{filtered.length>visibleCount&&<button className="slot-btn sec" onClick={()=>setVisibleCount(c=>c+25)}>Load more consultations</button>}</> };
    if (hasClass(n,'doc-filter-strip')) return { children:<><span>Current and remaining today ({active.length})</span><button className="doc-filter-chip" onClick={onQna}>Free Q&amp;A</button></> };

    if (id(n)==='docMobileChipQnaCount') return { children:null };
    return undefined;
  }} />;
}
