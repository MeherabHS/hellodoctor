'use client';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, errorText, runtimeGeneration } from '../lib/client';
import { startScheduleSync } from '../lib/schedule-sync';
import { Appointment, upcomingFirst } from '../lib/contracts';
export { initials } from './Common';

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
