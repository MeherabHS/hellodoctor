'use client';
import { isProfessionallyVerified, professionalVerificationLabel } from '../lib/professionalVerification';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, errorText, runtimeGeneration, useResource } from '../lib/client';
import { startScheduleSync } from '../lib/schedule-sync';
import { Appointment, Profile, upcomingFirst } from '../lib/contracts';
import { Prototype, hasClass, id, text } from './Prototype';
import { State } from './Common';

export type ClinicalAction = (action: 'summary' | 'history' | 'documents' | 'call' | 'chat' | 'rx', appointment: Appointment) => void;
export type QueueState = ReturnType<typeof useAppointments>;
export function useAppointments(enabled: boolean) {
  const [data,setData] = useState<Appointment[]>([]);
  const [loading,setLoading] = useState(enabled);
  const [error,setError] = useState('');
  const epoch = useRef(0);
  const refresh = useCallback(async () => {
    if (!enabled) return;
    const version = ++epoch.current;
    const generation = runtimeGeneration();
    const current = () => version === epoch.current && generation === runtimeGeneration();
    setError(''); setLoading(true);
    try {
      const results: Appointment[] = [];
      for (let offset = 0; ; offset += 100) {
        const page = await api<Appointment[]>(`doctor/appointments?limit=100&offset=${offset}`);
        if (!current()) return;
        results.push(...page);
        if (page.length < 100) break;
        if (offset >= 9900) throw new Error('Queue exceeds display capacity.');
      }
      if (current()) setData(upcomingFirst(results));
    } catch (e) { if (current() && !(e instanceof DOMException && e.name === 'AbortError')) setError(errorText(e)); }
    finally { if (current()) setLoading(false); }
  }, [enabled]);
  useEffect(() => {
    if (!enabled) { epoch.current++; setData([]); return; }
    void refresh();
    const stop = startScheduleSync(refresh, {window,document,online:()=>navigator.onLine,
      every:(callback,ms)=>window.setInterval(callback,ms),cancel:timer=>window.clearInterval(timer as number)});
    return () => { stop(); epoch.current++; };
  }, [enabled,refresh]);
  return { data,loading,error,refresh };
}
export function initials(name: string) { return name.trim().split(/\s+/).slice(0,2).map(x => x[0]).join('').toUpperCase() || '👤'; }
export function date(value: string | null | undefined) { return value ? new Date(value).toLocaleDateString('en-KE', { day: 'numeric', month: 'short', year: 'numeric', timeZone: 'Africa/Nairobi' }) : '—'; }
export function time(value: string | null | undefined) { return value ? new Date(value).toLocaleTimeString('en-KE', { hour: '2-digit', minute: '2-digit', timeZone: 'Africa/Nairobi' }) : '—'; }
export function dayKey(value: string) { return new Intl.DateTimeFormat('en-CA', { timeZone: 'Africa/Nairobi', year:'numeric',month:'2-digit',day:'2-digit' }).format(new Date(value)); }
export function statusLabel(a: Appointment, nextId?: string, now = Date.now()) {
  if (['COMPLETED','CANCELLED','REFUNDED','NO_SHOW'].includes(a.status) || a.doctor_lifecycle === 'DONE') return a.doctor_lifecycle.replaceAll('_',' ');
  const minutes = Math.ceil((Date.parse(a.start_time) - now) / 60000);
  if (a.status === 'IN_CONSULTATION') return 'In consultation';
  if (minutes > 0 && minutes <= 60) return `${a.id === nextId ? 'Next · ' : ''}Starts in ${minutes} min`;
  return `${a.id === nextId ? 'Next · ' : ''}${dayKey(a.start_time) === dayKey(new Date(now).toISOString()) ? 'Today' : 'Upcoming'}`;
}
export function QueueCard({ appointment: a, nextId, action }: { appointment: Appointment; nextId?: string; action: ClinicalAction }) {
  return <Prototype name="queueCard" action={handler => { if (handler.includes('callPatient')) action('call',a); else if (handler.includes('openDoctorChat')) action('chat',a); else action('rx',a); }} override={n => {
    if (hasClass(n,'queue-card')) return { props: { 'data-appointment-id':a.id,'data-category':a.status, style: { ...n.props.style as object } } };
    if (hasClass(n,'doc-slot-avatar')) return { children:initials(a.patient_display_name) };
    if (hasClass(n,'queue-patient-name')) return { children:<><button className="web-name-button" onClick={() => action('summary',a)}>{a.patient_display_name}</button><span className="badge badge-green" style={{ fontSize:9.5 }}>{a.modality}</span></> };
    if (hasClass(n,'queue-patient-meta')) return { children: <>{a.appointment_number} · {date(a.start_time)} · {statusLabel(a,nextId)}</> };
    if (hasClass(n,'badge') && !hasClass(n,'badge-green')) return { children:<>🕒 {time(a.start_time)} – {time(a.end_time)}</> };
    if (hasClass(n,'queue-complaint')) return { children:<><strong>Chief Complaint:</strong> <span>{a.clinical_outcome ?? 'Clinical intake requires authorized consultation access.'}</span><button className="dw-btn-mini" style={{ marginLeft:8 }} onClick={() => action('summary',a)}>View intake</button></> };
    if (n.tag === 'span' && text(n).includes('Call Patient')) return { children:'Call Patient 📞' };
    return undefined;
  }} />;
}
export function Queue({ queue, profile, duty, onDuty, action, onQna }: { queue: QueueState; profile: Profile; duty: { value?: boolean; busy:boolean; error:string }; onDuty:()=>void; action:ClinicalAction; onQna:()=>void }) {
  const [search,setSearch] = useState('');
  const [filter,setFilter] = useState('all');
  const publicQuestions=useResource<unknown[]>('doctor/qna/questions?status=OPEN&limit=100&offset=0');
  const [visibleCount,setVisibleCount] = useState(25);
  const [,setTick] = useState(0);
  useEffect(() => { const timer=setInterval(()=>setTick(t=>t+1),30000); return ()=>clearInterval(timer); },[]);
  const active = queue.data.filter(a=>!['COMPLETED','CANCELLED','REFUNDED','NO_SHOW'].includes(a.status) && a.doctor_lifecycle!=='DONE');
  const filtered = queue.data.filter(a => (filter==='all' || (filter==='upcoming' ? active.includes(a) : filter==='completed' ? a.status==='COMPLETED' || a.doctor_lifecycle==='DONE' : a.status==='CANCELLED')) && `${a.patient_display_name} ${a.appointment_number} ${a.clinical_outcome ?? ''}`.toLowerCase().includes(search.toLowerCase()));
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
    if (hasClass(n,'doc-filter-strip')) return { children:<><button className={`doc-filter-chip ${filter==='all'?'active':''}`} onClick={()=>{setFilter('all');setVisibleCount(25);}}>All Scheduled ({queue.data.length})</button><button className={`doc-filter-chip ${filter==='upcoming'?'active':''}`} onClick={()=>setFilter('upcoming')}>Upcoming ({active.length})</button><button className={`doc-filter-chip ${filter==='completed'?'active':''}`} onClick={()=>setFilter('completed')}>Completed ({queue.data.filter(a=>a.status==='COMPLETED').length})</button><button className={`doc-filter-chip ${filter==='cancelled'?'active':''}`} onClick={()=>setFilter('cancelled')}>Cancelled</button><button className="doc-filter-chip" style={{background:'var(--accent-light)',color:'#B45309'}} onClick={onQna}>❓ Free Q&amp;A</button></> };
    if (id(n)==='docMobileChipQnaCount') return { children:null };
    return undefined;
  }} />;
}
