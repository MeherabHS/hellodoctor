'use client';
import { tr } from '@/lib/i18n';
import { apiErrorMessage } from '@/lib/i18n/apiErrors';
import {useEffect,useState} from 'react';
import {X} from 'lucide-react';
import {request} from '../../lib/api';
import {Chip} from '../ui/afya';
import {nairobiDate,timeLeft} from './CommandCenter';

/** What `GET /admin/consultations/:id/timeline` returns (only the fields shown here). */
type TimelineEvent={event_type?:string;occurred_at?:string;created_at?:string;participant_role?:string;provider_event_at?:string;received_at?:string};
/** One consultation trace event (the 5-day ledger the grievance investigation shows). */
type TraceEvent={id:string;event_type:string;event_time_server:string;actor_role?:string|null;result?:string|null;reason_code?:string|null;resource_type?:string|null};
export type ConsultationTimelineData={
  session_id:string;status:string;recovery_state:string;started_at:string|null;ended_at:string|null;
  patient_present:boolean;doctor_present:boolean;interruption_reason:string|null;
  events:TimelineEvent[];provider_presence_events:TimelineEvent[];trace_events?:TraceEvent[];
  reconnect_windows:unknown[];related_grievances:unknown[];related_system_errors:unknown[];
  recovery_resolution:{outcome?:string;reason?:string;created_at?:string}|null;
  trace_retention_days?:number;trace_event_count?:number;trace_expires_at?:string;trace_on_hold?:boolean;
};

/** The consultation trace's retention line: kept until / held / deleted (never "nothing happened"). */
export function traceLine(data:ConsultationTimelineData,now:number):string|null{
  if(!data.trace_expires_at)return null;
  const count=data.trace_event_count??0;const days=data.trace_retention_days??5;
  if(data.trace_on_hold)return tr('ops.timelineTraceHeld',{count});
  if(new Date(data.trace_expires_at).getTime()<=now)return tr('ops.timelineTraceDeleted',{date:nairobiDate(data.trace_expires_at),days});
  if(!count)return tr('ops.timelineTraceEmpty',{date:nairobiDate(data.trace_expires_at)});
  return tr('ops.timelineTraceKept',{count,date:nairobiDate(data.trace_expires_at),time:timeLeft(data.trace_expires_at,now)});
}

const words=(code:string|undefined)=>(code??'event').toLowerCase().replaceAll('_',' ').replace(/^\w/,c=>c.toUpperCase());
const at=(value:string|null|undefined)=>value?new Date(value).toLocaleString('en-KE',{timeZone:'Africa/Nairobi',day:'numeric',month:'short',hour:'2-digit',minute:'2-digit',second:'2-digit'}):'—';

/** One merged, time-ordered list: consultation trace, app events and signed Agora presence events. */
export function timelineRows(data:ConsultationTimelineData){
  const rows=[
    ...data.events.map(e=>({time:e.occurred_at??e.created_at??'',label:words(e.event_type),source:tr('ui2.app')})),
    ...data.provider_presence_events.map(e=>({time:e.provider_event_at??e.received_at??'',label:`${words(e.participant_role)} · ${words(e.event_type)}`,source:'Agora'})),
    ...(data.trace_events??[]).map(e=>({time:e.event_time_server,
      label:[e.actor_role?words(e.actor_role):null,words(e.event_type),e.result&&e.result!=='SUCCESS'?words(e.result)+(e.reason_code?` (${words(e.reason_code)})`:''):null].filter(Boolean).join(' · '),
      source:tr('ops.traceSource')})),
  ];
  return rows.sort((a,b)=>a.time.localeCompare(b.time));
}

/** The consultation timeline in a panel over the dashboard, instead of raw JSON in a new tab. */
export function ConsultationTimeline({path,onClose}:{path:string;onClose:()=>void}){
  const [data,setData]=useState<ConsultationTimelineData|null>(null);const [error,setError]=useState('');
  useEffect(()=>{let active=true;request<ConsultationTimelineData>(path).then(d=>{if(active)setData(d);}).catch(e=>{if(active)setError(apiErrorMessage(e, tr('ui.unableToLoadTheTimeline')));});return()=>{active=false;};},[path]);
  useEffect(()=>{const key=(e:KeyboardEvent)=>{if(e.key==='Escape')onClose();};window.addEventListener('keydown',key);return()=>window.removeEventListener('keydown',key);},[onClose]);
  const rows=data?timelineRows(data):[];
  // Read once when the panel opens; the panel is short-lived.
  const [openedAt]=useState(()=>Date.now());
  const trace=data?traceLine(data,openedAt):null;
  return <div className="admin-modal-backdrop" role="presentation" onClick={onClose}>
    <section role="dialog" aria-modal="true" aria-labelledby="timeline-title" className="admin-modal-card afya-timeline-panel" onClick={e=>e.stopPropagation()}>
      <div style={{display:'flex',justifyContent:'space-between',alignItems:'flex-start',gap:12}}>
        <div><h2 id="timeline-title" className="afya-modal-title">{tr('ui.consultationTimeline')}</h2><p className="afya-modal-sub">{tr('ui.timesInAfricaNairobi')}</p></div>
        <button type="button" className="afya-icon-btn" aria-label={tr('ui.closeTimeline')} onClick={onClose}><X size={18}/></button>
      </div>
      {error&&<div role="alert" className="afya-login-error" style={{marginTop:12}}>{error}</div>}
      {!data&&!error&&<p className="afya-stat-note">{tr('ui.loading')}</p>}
      {data&&<>
        <dl className="afya-timeline-summary">
          <div><dt>{tr('ui.status')}</dt><dd><Chip tone={data.recovery_state==='NEEDS_REVIEW'?'warn':'neutral'}>{words(data.status)}</Chip></dd></div>
          <div><dt>{tr('ui.started')}</dt><dd>{at(data.started_at)}</dd></div>
          <div><dt>{tr('ui.ended')}</dt><dd>{data.ended_at?at(data.ended_at):tr('ui2.notEnded')}</dd></div>
          <div><dt>{tr('ui.inTheRoomNow')}</dt><dd>{[data.patient_present&&tr('ui2.patient'),data.doctor_present&&tr('ui2.doctor')].filter(Boolean).join(' and ')||tr('ui2.nobody')}</dd></div>
          <div><dt>{tr('ui.recovery')}</dt><dd>{words(data.recovery_state)}{data.recovery_resolution?.outcome?` · ${words(data.recovery_resolution.outcome)}`:''}</dd></div>
          <div><dt>{tr('ui.reconnectsGrievancesAppErrors')}</dt><dd>{data.reconnect_windows.length} · {data.related_grievances.length} · {data.related_system_errors.length}</dd></div>
          {trace&&<div style={{gridColumn:'1 / -1'}}><dt>{tr('ops.timelineTrace')}</dt><dd>{trace}</dd></div>}
        </dl>
        <h3 className="afya-timeline-heading">{tr('ui.events')}</h3>
        {rows.length?<ol className="afya-timeline-list">{rows.map((r,i)=><li key={i}><span className="afya-timeline-time">{at(r.time)}</span><span>{r.label}</span><Chip tone={r.source==='Agora'?'topic':r.source===tr('ops.traceSource')?'info':'outlined'}>{r.source}</Chip></li>)}</ol>
          :<p className="afya-stat-note">{tr('ops.noEvents')}</p>}
        {!!rows.length&&!data.provider_presence_events.length&&<p className="afya-stat-note">{tr('ops.noPresence')}</p>}
      </>}
    </section>
  </div>;
}
