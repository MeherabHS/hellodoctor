'use client';
import { tr, type MessageKey } from '@/lib/i18n';
import {useEffect,useState} from 'react';
import {Activity,MessageSquare,Users,Video} from 'lucide-react';
import {ConsultationTimeline} from './ConsultationTimeline';
import type {AdminOverview,CommandTelemetry} from '../../lib/api';
import {Card,Chip,EmptyState,Notice,PageHead,StatTile} from '../ui/afya';

/**
 * Dashboard → Operations & Live Command Center (Operations board, panel 1).
 * Platform uptime, per-service latency and day-on-day trends are not shown:
 * the backend does not measure them (product owner, 2026-11-14).
 * No elapsed timer (it read as "the room is live" when nobody was in it) and
 * no emergency-helpline tile (product owner, 2026-11-16).
 * Each session shows how long its consultation trace is still kept (the
 * N-day retention countdown), with a note on why (product owner, 2026-11-22).
 */
type Stream=CommandTelemetry['active_streams'][number];

/** "4 d 3 h", "3 h 12 min", "12 min" — time left until `until`. */
export function timeLeft(until:string,now:number):string{
  const mins=Math.max(0,Math.floor((new Date(until).getTime()-now)/60000));
  const d=Math.floor(mins/1440),h=Math.floor((mins%1440)/60),m=mins%60;
  return d?`${d} d ${h} h`:h?`${h} h ${m} min`:`${m} min`;
}
export const nairobiDate=(value:string)=>new Date(value).toLocaleString('en-KE',{timeZone:'Africa/Nairobi',day:'numeric',month:'short',hour:'2-digit',minute:'2-digit'});

/** The 5-day consultation trace rule, explained in full (collapsible so the table stays in view). */
function TraceRuleNote({days}:{days:number}){
  const items:[MessageKey,MessageKey][]=[['ops.whyWhatLabel','ops.whyWhat'],['ops.whyKeepLabel','ops.whyKeep'],['ops.whyDeleteLabel','ops.whyDelete'],['ops.whyNeverLabel','ops.whyNever'],['ops.whyHoldLabel','ops.whyHold'],['ops.whyActionLabel','ops.whyAction']];
  return <Notice tone="info" title={tr('ops.whyTitle',{days})}>
    <p style={{margin:'4px 0 0'}}>{tr('ops.whyIntro',{days})}</p>
    <details className="afya-trace-rule"><summary>{tr('ops.whyMore')}</summary>
      <dl>{items.map(([label,body])=><div key={label}><dt>{tr(label,{days})}</dt><dd>{tr(body,{days})}</dd></div>)}</dl>
    </details>
  </Notice>;
}

/** The trace retention cell: countdown, held by a grievance, or already deleted. */
export function TraceTimer({stream,now}:{stream:Stream;now:number}){
  if(!stream.trace_expires_at)return <span className="afya-stat-note">—</span>;
  if(stream.trace_on_hold)return <Chip tone="topic">{tr('ops.traceHeld')}</Chip>;
  const expired=new Date(stream.trace_expires_at).getTime()<=now;
  if(expired)return <><Chip tone="neutral">{tr('ops.traceDeleted',{date:nairobiDate(stream.trace_expires_at)})}</Chip></>;
  const urgent=new Date(stream.trace_expires_at).getTime()-now<24*3600_000;
  return <><Chip tone={urgent?'danger':'warn'}>{tr('ops.traceLeft',{time:timeLeft(stream.trace_expires_at,now)})}</Chip>
    {!stream.trace_event_count&&<div className="afya-stat-note">{tr('ops.traceNone')}</div>}</>;
}

export function CommandCenter({telemetry,overview,needsReviewOnly,setNeedsReviewOnly,busyId,onRecordDecision,apiBase}:{telemetry:CommandTelemetry|null;overview:AdminOverview|null;needsReviewOnly:boolean;setNeedsReviewOnly:(v:boolean)=>void;busyId:string|null;onRecordDecision:(sessionId:string)=>void;apiBase:string}){
  const [timeline,setTimeline]=useState<string|null>(null);
  const streams=(telemetry?.active_streams??[]).filter(s=>!needsReviewOnly||s.needs_review);
  const reviewCount=telemetry?.needs_review_count??0;
  // The countdowns tick once a minute.
  const [now,setNow]=useState(()=>Date.now());
  useEffect(()=>{const id=window.setInterval(()=>setNow(Date.now()),60_000);return()=>window.clearInterval(id);},[]);
  const retentionDays=telemetry?.active_streams.find(s=>s.trace_retention_days)?.trace_retention_days??5;
  const anyClosed=streams.some(s=>s.room_closed);

  return <div className="afya-stack">
    <PageHead title={tr('ui.realTimeOperationsOverview')} subtitle={<span style={{display:'inline-flex',alignItems:'center',gap:8}}><span className="afya-live-dot" aria-hidden/>{tr('ui.liveRefreshesAutomatically')}</span>}/>
    <div className="afya-stat-grid">
      <StatTile icon={Video} tone="green" value={telemetry?.completed_today??'—'} label={tr('ui.consultationsCompletedToday')}/>
      <StatTile icon={Users} tone="blue" value={telemetry?.active_specialists??'—'} label={tr('ui.activeSpecialists')} note={overview?tr('ui4.ofRegistered', { count: overview.total_doctors }):undefined}/>
      <StatTile icon={MessageSquare} tone="purple" value={telemetry?.active_sessions_count??'—'} label={tr('ops.ongoing')} note={tr('ui4.videoChat')}/>
    </div>

    <Card title={tr('ops.trackingTitle', { days: retentionDays, count: telemetry?.active_streams.length??0 })} subtitle={tr('ops.trackingSubtitle', { days: retentionDays })}
      actions={<label className="afya-check"><input type="checkbox" checked={needsReviewOnly} onChange={e=>setNeedsReviewOnly(e.target.checked)}/>{tr('mix.needsReviewOnly', { count: reviewCount })}</label>}>
      <div style={{display:'flex',flexDirection:'column',gap:12,marginBottom:16}}>
        <TraceRuleNote days={retentionDays}/>
        {anyClosed&&<Notice tone="warn" title={tr('ops.closedTitle')}>{tr('ops.closedBody')}</Notice>}
      </div>
      {!streams.length?<EmptyState title={needsReviewOnly?tr('ui2.noSessionsNeedReview'):tr('ops.trackingEmpty', { days: retentionDays })}>{tr('ops.trackingEmptyHint', { days: retentionDays })}</EmptyState>:
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>{tr('ui.session')}</th><th>{tr('ui.doctorPatient')}</th><th>{tr('ui.presence')}</th><th>{tr('ui.connection')}</th><th>{tr('ui.status')}</th><th>{tr('ops.traceKept')}</th><th style={{textAlign:'right'}}>{tr('ui.actions')}</th></tr></thead>
        <tbody>{streams.map((s:Stream)=><tr key={s.session_id}>
          <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-stat-icon afya-tone-green" style={{width:34,height:34,borderRadius:9}} aria-hidden><Activity size={16}/></span>
            <span title={s.channel_name} style={{fontFamily:'ui-monospace,monospace',fontSize:12}}>{s.channel_name.length>22?`${s.channel_name.slice(0,22)}…`:s.channel_name}</span></div></td>
          <td><div style={{fontWeight:700}}>{s.doctor_name}</div><div className="afya-stat-note">{tr('mix.patientPrefix')}{s.patient_name}</div></td>
          <td><div className="afya-stat-note" style={{marginTop:0}}>{tr(s.provider_presence_summary?.patient_present?'mix.patientPresent':'mix.patientAbsent')}<br/>{tr(s.provider_presence_summary?.doctor_present?'mix.doctorPresent':'mix.doctorAbsent')}</div></td>
          <td>{s.rtt_ms==null?<Chip tone="neutral">{tr('ui.notMeasured')}</Chip>:<Chip tone={s.rtt_ms<150?'success':'warn'}>{s.rtt_ms} ms</Chip>}<div className="afya-stat-note">{s.connection_state}</div></td>
          <td>{s.room_closed?<><Chip tone="neutral">{tr('ops.roomClosed')}</Chip>{s.join_window_closes_at&&<div className="afya-stat-note">{tr('ops.roomClosedNote',{date:nairobiDate(s.join_window_closes_at)})}</div>}</>:s.needs_review?<Chip tone="warn">{tr('ui.needsReview')}</Chip>:s.ongoing?<Chip tone="success">{tr('ops.statusOngoing')}</Chip>:<Chip tone="neutral">{s.session_status?s.session_status.charAt(0)+s.session_status.slice(1).toLowerCase().replaceAll('_',' '):tr('ui2.active')}</Chip>}
            {s.needs_review&&<div className="afya-stat-note">{tr('mix.paymentStatus', { status: s.payment_status??tr('mix.unknown') })}</div>}</td>
          <td><TraceTimer stream={s} now={now}/></td>
          <td style={{textAlign:'right',whiteSpace:'nowrap'}}>
            {s.timeline_url&&<button type="button" className="afya-btn-secondary" onClick={()=>setTimeline(s.timeline_url!)}>{tr('ui.timeline')}</button>}
            {s.needs_review&&<button type="button" className="afya-btn-primary" style={{marginLeft:8}} disabled={busyId===s.session_id} onClick={()=>onRecordDecision(s.session_id)}>{busyId===s.session_id?tr('ui2.saving'):tr('ui2.recordDecision')}</button>}
          </td>
        </tr>)}</tbody>
      </table></div>}
    </Card>
    {timeline&&<ConsultationTimeline path={timeline} onClose={()=>setTimeline(null)}/>}
  </div>;
}
