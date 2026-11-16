'use client';
import {useState} from 'react';
import {Activity,MessageSquare,Users,Video} from 'lucide-react';
import {ConsultationTimeline} from './ConsultationTimeline';
import type {AdminOverview,CommandTelemetry} from '../../lib/api';
import {Card,Chip,EmptyState,PageHead,StatTile} from '../ui/afya';

/**
 * Dashboard → Operations & Live Command Center (Operations board, panel 1).
 * Platform uptime, per-service latency and day-on-day trends are not shown:
 * the backend does not measure them (product owner, 2026-11-14).
 * No elapsed timer (it read as "the room is live" when nobody was in it) and
 * no emergency-helpline tile (product owner, 2026-11-16).
 */
type Stream=CommandTelemetry['active_streams'][number];

export function CommandCenter({telemetry,overview,needsReviewOnly,setNeedsReviewOnly,busyId,onRecordDecision,apiBase}:{telemetry:CommandTelemetry|null;overview:AdminOverview|null;needsReviewOnly:boolean;setNeedsReviewOnly:(v:boolean)=>void;busyId:string|null;onRecordDecision:(sessionId:string)=>void;apiBase:string}){
  const [timeline,setTimeline]=useState<string|null>(null);
  const streams=(telemetry?.active_streams??[]).filter(s=>!needsReviewOnly||s.needs_review);
  const reviewCount=telemetry?.needs_review_count??0;

  return <div className="afya-stack">
    <PageHead title="Real-Time Operations Overview" subtitle={<span style={{display:'inline-flex',alignItems:'center',gap:8}}><span className="afya-live-dot" aria-hidden/>Live · refreshes automatically</span>}/>
    <div className="afya-stat-grid">
      <StatTile icon={Video} tone="green" value={telemetry?.completed_today??'—'} label="Consultations completed today"/>
      <StatTile icon={Users} tone="blue" value={telemetry?.active_specialists??'—'} label="Active specialists" note={overview?`of ${overview.total_doctors} registered`:undefined}/>
      <StatTile icon={MessageSquare} tone="purple" value={telemetry?.active_sessions_count??'—'} label="Live sessions" note="Video + chat"/>
    </div>

    <Card title={`Live Consultation Sessions (${telemetry?.active_streams.length??0})`} subtitle="Agora sessions in progress and sessions that need a recovery decision."
      actions={<label className="afya-check"><input type="checkbox" checked={needsReviewOnly} onChange={e=>setNeedsReviewOnly(e.target.checked)}/>Needs review only ({reviewCount})</label>}>
      {!streams.length?<EmptyState title={needsReviewOnly?'No sessions need review':'No consultations in progress'}>Live sessions appear here as soon as a consultation starts.</EmptyState>:
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>Session</th><th>Doctor / patient</th><th>Presence</th><th>Connection</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
        <tbody>{streams.map((s:Stream)=><tr key={s.session_id}>
          <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-stat-icon afya-tone-green" style={{width:34,height:34,borderRadius:9}} aria-hidden><Activity size={16}/></span>
            <span title={s.channel_name} style={{fontFamily:'ui-monospace,monospace',fontSize:12}}>{s.channel_name.length>22?`${s.channel_name.slice(0,22)}…`:s.channel_name}</span></div></td>
          <td><div style={{fontWeight:700}}>{s.doctor_name}</div><div className="afya-stat-note">Patient: {s.patient_name}</div></td>
          <td><div className="afya-stat-note" style={{marginTop:0}}>Patient {s.provider_presence_summary?.patient_present?'present':'absent'}<br/>Doctor {s.provider_presence_summary?.doctor_present?'present':'absent'}</div></td>
          <td>{s.rtt_ms==null?<Chip tone="neutral">Not measured</Chip>:<Chip tone={s.rtt_ms<150?'success':'warn'}>{s.rtt_ms} ms</Chip>}<div className="afya-stat-note">{s.connection_state}</div></td>
          <td>{s.needs_review?<Chip tone="warn">Needs review</Chip>:<Chip tone="success">{s.session_status?s.session_status.charAt(0)+s.session_status.slice(1).toLowerCase():'Active'}</Chip>}
            {s.needs_review&&<div className="afya-stat-note">Payment: {s.payment_status??'unknown'}</div>}</td>
          <td style={{textAlign:'right',whiteSpace:'nowrap'}}>
            {s.timeline_url&&<button type="button" className="afya-btn-secondary" onClick={()=>setTimeline(s.timeline_url!)}>Timeline</button>}
            {s.needs_review&&<button type="button" className="afya-btn-primary" style={{marginLeft:8}} disabled={busyId===s.session_id} onClick={()=>onRecordDecision(s.session_id)}>{busyId===s.session_id?'Saving…':'Record decision'}</button>}
          </td>
        </tr>)}</tbody>
      </table></div>}
    </Card>
    {timeline&&<ConsultationTimeline path={timeline} onClose={()=>setTimeline(null)}/>}
  </div>;
}
