'use client';
import {useEffect,useState} from 'react';
import {Activity,MessageSquare,PhoneCall,Users,Video} from 'lucide-react';
import type {AdminOverview,CommandTelemetry} from '../../lib/api';
import {Card,Chip,EmptyState,PageHead,StatTile} from '../ui/afya';

/**
 * Dashboard → Operations & Live Command Center (Operations board, panel 1).
 * Platform uptime, per-service latency and day-on-day trends are not shown:
 * the backend does not measure them (product owner, 2026-11-14).
 */
type Stream=CommandTelemetry['active_streams'][number];

function elapsed(from:string|null,now:number){
  if(!from)return '—';const s=Math.max(0,Math.floor((now-new Date(from).getTime())/1000));
  const h=Math.floor(s/3600),m=Math.floor(s%3600/60),sec=s%60;
  return `${h?`${h}:`:''}${String(m).padStart(h?2:1,'0')}:${String(sec).padStart(2,'0')}`;
}

export function CommandCenter({telemetry,overview,needsReviewOnly,setNeedsReviewOnly,busyId,onRecordDecision,apiBase}:{telemetry:CommandTelemetry|null;overview:AdminOverview|null;needsReviewOnly:boolean;setNeedsReviewOnly:(v:boolean)=>void;busyId:string|null;onRecordDecision:(sessionId:string)=>void;apiBase:string}){
  const [now,setNow]=useState(()=>Date.now());
  useEffect(()=>{const t=window.setInterval(()=>setNow(Date.now()),1000);return()=>window.clearInterval(t);},[]);
  const streams=(telemetry?.active_streams??[]).filter(s=>!needsReviewOnly||s.needs_review);
  const reviewCount=telemetry?.needs_review_count??0;

  return <div className="afya-stack">
    <PageHead title="Real-Time Operations Overview" subtitle={<span style={{display:'inline-flex',alignItems:'center',gap:8}}><span className="afya-live-dot" aria-hidden/>Live · refreshes automatically</span>}/>
    <div className="afya-stat-grid">
      <StatTile icon={Video} tone="green" value={telemetry?.completed_today??'—'} label="Consultations completed today"/>
      <StatTile icon={Users} tone="blue" value={telemetry?.active_specialists??'—'} label="Active specialists" note={overview?`of ${overview.total_doctors} registered`:undefined}/>
      <StatTile icon={MessageSquare} tone="purple" value={telemetry?.active_sessions_count??'—'} label="Live sessions" note="Video + chat"/>
      <div className="afya-stat afya-stat-alert">
        <div className="afya-stat-icon afya-tone-red"><PhoneCall size={22} aria-hidden/></div>
        <div><div className="afya-stat-label" style={{marginTop:0,color:'var(--afya-danger-fg)',fontWeight:700}}>Emergency helpline</div>
          <div className="afya-stat-value" style={{fontSize:18,color:'var(--afya-success-fg)'}}>● Configured</div>
          <div className="afya-stat-note">Kenya contact set per deployment</div></div>
      </div>
    </div>

    <Card title={`Live Consultation Sessions (${telemetry?.active_streams.length??0})`} subtitle="Agora sessions in progress and sessions that need a recovery decision."
      actions={<label className="afya-check"><input type="checkbox" checked={needsReviewOnly} onChange={e=>setNeedsReviewOnly(e.target.checked)}/>Needs review only ({reviewCount})</label>}>
      {!streams.length?<EmptyState title={needsReviewOnly?'No sessions need review':'No consultations in progress'}>Live sessions appear here as soon as a consultation starts.</EmptyState>:
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>Session</th><th>Doctor / patient</th><th>Presence</th><th>Connection</th><th>Elapsed</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
        <tbody>{streams.map((s:Stream)=><tr key={s.session_id}>
          <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-stat-icon afya-tone-green" style={{width:34,height:34,borderRadius:9}} aria-hidden><Activity size={16}/></span>
            <span title={s.channel_name} style={{fontFamily:'ui-monospace,monospace',fontSize:12}}>{s.channel_name.length>22?`${s.channel_name.slice(0,22)}…`:s.channel_name}</span></div></td>
          <td><div style={{fontWeight:700}}>{s.doctor_name}</div><div className="afya-stat-note">Patient: {s.patient_name}</div></td>
          <td><div className="afya-stat-note" style={{marginTop:0}}>Patient {s.provider_presence_summary?.patient_present?'present':'absent'}<br/>Doctor {s.provider_presence_summary?.doctor_present?'present':'absent'}</div></td>
          <td>{s.rtt_ms==null?<Chip tone="neutral">Not measured</Chip>:<Chip tone={s.rtt_ms<150?'success':'warn'}>{s.rtt_ms} ms</Chip>}<div className="afya-stat-note">{s.connection_state}</div></td>
          <td style={{fontVariantNumeric:'tabular-nums'}}>{elapsed(s.started_at,now)}</td>
          <td>{s.needs_review?<Chip tone="warn">Needs review</Chip>:<Chip tone="success">{s.session_status?s.session_status.charAt(0)+s.session_status.slice(1).toLowerCase():'Active'}</Chip>}
            {s.needs_review&&<div className="afya-stat-note">Payment: {s.payment_status??'unknown'}</div>}</td>
          <td style={{textAlign:'right',whiteSpace:'nowrap'}}>
            {s.timeline_url&&<a className="afya-btn-secondary" href={`${apiBase}${s.timeline_url}`} target="_blank" rel="noreferrer" style={{textDecoration:'none'}}>Timeline</a>}
            {s.needs_review&&<button type="button" className="afya-btn-primary" style={{marginLeft:8}} disabled={busyId===s.session_id} onClick={()=>onRecordDecision(s.session_id)}>{busyId===s.session_id?'Saving…':'Record decision'}</button>}
          </td>
        </tr>)}</tbody>
      </table></div>}
    </Card>
  </div>;
}
