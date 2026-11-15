'use client';
import {useEffect,useState} from 'react';
import {X} from 'lucide-react';
import {request} from '../../lib/api';
import {Chip} from '../ui/afya';

/** What `GET /admin/consultations/:id/timeline` returns (only the fields shown here). */
type TimelineEvent={event_type?:string;occurred_at?:string;created_at?:string;participant_role?:string;provider_event_at?:string;received_at?:string};
export type ConsultationTimelineData={
  session_id:string;status:string;recovery_state:string;started_at:string|null;ended_at:string|null;
  patient_present:boolean;doctor_present:boolean;interruption_reason:string|null;
  events:TimelineEvent[];provider_presence_events:TimelineEvent[];
  reconnect_windows:unknown[];related_grievances:unknown[];related_system_errors:unknown[];
  recovery_resolution:{outcome?:string;reason?:string;created_at?:string}|null;
};

const words=(code:string|undefined)=>(code??'event').toLowerCase().replaceAll('_',' ').replace(/^\w/,c=>c.toUpperCase());
const at=(value:string|null|undefined)=>value?new Date(value).toLocaleString('en-KE',{timeZone:'Africa/Nairobi',day:'numeric',month:'short',hour:'2-digit',minute:'2-digit',second:'2-digit'}):'—';

/** One merged, time-ordered list: app events and signed Agora presence events. */
export function timelineRows(data:ConsultationTimelineData){
  const rows=[
    ...data.events.map(e=>({time:e.occurred_at??e.created_at??'',label:words(e.event_type),source:'App'})),
    ...data.provider_presence_events.map(e=>({time:e.provider_event_at??e.received_at??'',label:`${words(e.participant_role)} · ${words(e.event_type)}`,source:'Agora'})),
  ];
  return rows.sort((a,b)=>a.time.localeCompare(b.time));
}

/** The consultation timeline in a panel over the dashboard, instead of raw JSON in a new tab. */
export function ConsultationTimeline({path,onClose}:{path:string;onClose:()=>void}){
  const [data,setData]=useState<ConsultationTimelineData|null>(null);const [error,setError]=useState('');
  useEffect(()=>{let active=true;request<ConsultationTimelineData>(path).then(d=>{if(active)setData(d);}).catch(e=>{if(active)setError(e instanceof Error?e.message:'Unable to load the timeline.');});return()=>{active=false;};},[path]);
  useEffect(()=>{const key=(e:KeyboardEvent)=>{if(e.key==='Escape')onClose();};window.addEventListener('keydown',key);return()=>window.removeEventListener('keydown',key);},[onClose]);
  const rows=data?timelineRows(data):[];
  return <div className="admin-modal-backdrop" role="presentation" onClick={onClose}>
    <section role="dialog" aria-modal="true" aria-labelledby="timeline-title" className="admin-modal-card afya-timeline-panel" onClick={e=>e.stopPropagation()}>
      <div style={{display:'flex',justifyContent:'space-between',alignItems:'flex-start',gap:12}}>
        <div><h2 id="timeline-title" className="afya-modal-title">Consultation timeline</h2><p className="afya-modal-sub">Times in Africa/Nairobi</p></div>
        <button type="button" className="afya-icon-btn" aria-label="Close timeline" onClick={onClose}><X size={18}/></button>
      </div>
      {error&&<div role="alert" className="afya-login-error" style={{marginTop:12}}>{error}</div>}
      {!data&&!error&&<p className="afya-stat-note">Loading…</p>}
      {data&&<>
        <dl className="afya-timeline-summary">
          <div><dt>Status</dt><dd><Chip tone={data.recovery_state==='NEEDS_REVIEW'?'warn':'neutral'}>{words(data.status)}</Chip></dd></div>
          <div><dt>Started</dt><dd>{at(data.started_at)}</dd></div>
          <div><dt>Ended</dt><dd>{data.ended_at?at(data.ended_at):'Not ended'}</dd></div>
          <div><dt>In the room now</dt><dd>{[data.patient_present&&'Patient',data.doctor_present&&'Doctor'].filter(Boolean).join(' and ')||'Nobody'}</dd></div>
          <div><dt>Recovery</dt><dd>{words(data.recovery_state)}{data.recovery_resolution?.outcome?` · ${words(data.recovery_resolution.outcome)}`:''}</dd></div>
          <div><dt>Reconnects · grievances · app errors</dt><dd>{data.reconnect_windows.length} · {data.related_grievances.length} · {data.related_system_errors.length}</dd></div>
        </dl>
        <h3 className="afya-timeline-heading">Events</h3>
        {rows.length?<ol className="afya-timeline-list">{rows.map((r,i)=><li key={i}><span className="afya-timeline-time">{at(r.time)}</span><span>{r.label}</span><Chip tone={r.source==='Agora'?'topic':'outlined'}>{r.source}</Chip></li>)}</ol>
          :<p className="afya-stat-note">No events yet. Nobody has joined this consultation’s room.</p>}
      </>}
    </section>
  </div>;
}
