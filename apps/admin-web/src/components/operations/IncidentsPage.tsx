'use client';
import {useState} from 'react';
import {AlertTriangle,CheckCircle2,Check,Download,Eye,Flame,Layers,RefreshCw} from 'lucide-react';
import type {AppErrorLogItem} from '../../data/adminStore';
import {Card,Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,Tabs,type ChipTone} from '../ui/afya';

/**
 * Operations & Safety → App Errors & Incidents (Operations board, panel 3).
 * Real `system_error_logs` rows only. Outages, maintenance windows, manual
 * "Log Incident" and assignment have no backend and are not shown (product
 * owner, 2026-11-14). Filtering state lives in the page, as before.
 */
const PAGE=20;
export const SUBSYSTEMS:[string,string][]=[
  ['PAYMENT_GATEWAY','Payment gateway'],['AGORA_RTC','Agora video'],['DATABASE','Database'],['PATIENT_FLUTTER','Patient app'],['DOCTOR_FLUTTER','Doctor app'],['DOCTOR_WEB','Doctor Web'],['ADMIN_WEB','Admin Web'],
  ['INTERNAL','Internal'],['DEPENDENCY','Dependency'],['CLINICAL_RX_ENGINE','Prescriptions'],['SYNC_OFFLINE_CACHE','Sync & offline'],['PROFESSIONAL_KYC_HUB','Professional registration'],['AUTH_OTP_GATEWAY','Auth / OTP'],
];
const SEVERITY:Record<AppErrorLogItem['severity'],ChipTone>={CRITICAL:'danger',ERROR:'warn',WARNING:'info',INFO:'neutral'};
const STATUS:Record<AppErrorLogItem['status'],{tone:ChipTone;label:string}>={UNRESOLVED:{tone:'danger',label:'Unresolved'},INVESTIGATING:{tone:'info',label:'Investigating'},RESOLVED:{tone:'success',label:'Resolved'}};
const sentence=(s:string)=>s.charAt(0)+s.slice(1).toLowerCase();

export function IncidentsPage(props:{
  logs:AppErrorLogItem[];filtered:AppErrorLogItem[];
  search:string;setSearch:(v:string)=>void;severity:string;setSeverity:(v:string)=>void;subsystem:string;setSubsystem:(v:string)=>void;status:string;setStatus:(v:string)=>void;
  busyId:string|null;onInspect:(id:string)=>void;onResolve:(id:string)=>void;onExport:()=>void;onRefresh:()=>void;
}){
  const {logs,filtered}=props;const [offset,setOffset]=useState(0);
  const open=logs.filter(l=>l.status!=='RESOLVED').length;const resolved=logs.length-open;
  const severe=logs.filter(l=>l.status!=='RESOLVED'&&(l.severity==='CRITICAL'||l.severity==='ERROR')).length;
  const resetTo=(f:()=>void)=>{f();setOffset(0);};
  const statusCount=(s:string)=>s==='ALL'?logs.length:logs.filter(l=>l.status===s).length;

  return <div className="afya-stack">
    <PageHead title="App Errors & Incidents" subtitle="Track, triage and resolve app errors reported by the backend and the apps."
      actions={<>

        <button type="button" className="afya-btn-secondary" onClick={props.onExport}><Download size={16} aria-hidden/>Export (JSON)</button>
        <button type="button" className="afya-btn-primary" onClick={props.onRefresh}><RefreshCw size={16} aria-hidden/>Refresh</button>
      </>}/>
    <div className="afya-stat-grid">
      <StatTile icon={AlertTriangle} tone="red" value={open} label="Open incidents"/>
      <StatTile icon={Flame} tone="amber" value={severe} label="Critical or error, open"/>
      <StatTile icon={Layers} tone="blue" value={logs.length} label="Total logged"/>
      <StatTile icon={CheckCircle2} tone="green" value={logs.length?`${Math.round(resolved/logs.length*100)}%`:'—'} label="Resolved" note={logs.length?`${resolved} of ${logs.length}`:undefined}/>
    </div>
    <Card>
      <Tabs label="Incident status" value={props.status} onChange={v=>resetTo(()=>props.setStatus(v))} items={['ALL','UNRESOLVED','INVESTIGATING','RESOLVED'].map(s=>({id:s,label:s==='ALL'?'All incidents':STATUS[s as AppErrorLogItem['status']].label,count:statusCount(s)}))}/>
      <div className="afya-filter-bar">
        <SearchField label="Search incidents" placeholder="Search by user, trace ID, message or error code…" maxLength={150} value={props.search} onChange={v=>resetTo(()=>props.setSearch(v))}/>
        <select className="afya-select" aria-label="Module" value={props.subsystem} onChange={e=>resetTo(()=>props.setSubsystem(e.target.value))}><option value="ALL">All modules</option>{SUBSYSTEMS.map(([v,l])=><option key={v} value={v}>{l}</option>)}</select>
      </div>
      <div className="afya-chip-filters" role="group" aria-label="Severity">
        {['ALL','CRITICAL','ERROR','WARNING'].map(s=>{const n=s==='ALL'?logs.length:logs.filter(l=>l.severity===s).length;return <button key={s} type="button" aria-pressed={props.severity===s} className={`afya-chip-filter ${props.severity===s?'on':''}`} onClick={()=>resetTo(()=>props.setSeverity(s))}>
          {s!=='ALL'&&<span className={`afya-dot ${SEVERITY[s as AppErrorLogItem['severity']]}`} aria-hidden/>}{s==='ALL'?'All':sentence(s)} {n}</button>;})}
      </div>
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Trace ID</th><th>Incident / error</th><th>Module</th><th>User</th><th>Severity</th><th>Status</th><th>Updated</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{filtered.slice(offset,offset+PAGE).map(log=><tr key={log.id} className="admin-clickable-row" onClick={()=>props.onInspect(log.id)}>
            <td><span className="afya-mono">{log.traceId}</span></td>
            <td style={{maxWidth:360}}><div style={{fontWeight:700}}>{log.errorMessage}</div><div className="afya-stat-note"><span className="afya-mono">{log.errorCode}</span>{log.occurrenceCount>1?` · ${log.occurrenceCount} occurrences`:''}</div></td>
            <td><Chip tone="topic">{SUBSYSTEMS.find(([v])=>v===log.subsystem)?.[1]??log.subsystem}</Chip></td>
            <td><div style={{fontWeight:650}}>{log.user.name}</div><div className="afya-stat-note">{log.user.role}</div></td>
            <td><Chip tone={SEVERITY[log.severity]}>{sentence(log.severity)}</Chip></td>
            <td><Chip tone={STATUS[log.status].tone}>{STATUS[log.status].label}</Chip></td>
            <td style={{whiteSpace:'nowrap'}}>{log.timeFormatted}<div className="afya-stat-note">{log.relativeTime}</div></td>
            <td style={{textAlign:'right',whiteSpace:'nowrap'}}>
              <button type="button" className="afya-icon-btn" aria-label={`Inspect ${log.traceId}`} onClick={e=>{e.stopPropagation();props.onInspect(log.id);}}><Eye size={17}/></button>
              {log.status!=='RESOLVED'&&<button type="button" className="afya-icon-btn" aria-label={`Mark ${log.traceId} resolved`} title="Mark as resolved" disabled={props.busyId===log.id} onClick={e=>{e.stopPropagation();props.onResolve(log.id);}}><Check size={17} color="var(--afya-success-fg)"/></button>}
            </td>
          </tr>)}</tbody>
        </table>
        {!filtered.length&&<EmptyState title={logs.length?'No incidents match these filters':'No incidents logged'}>{logs.length?'Try another status, module or severity.':'Errors reported by the backend and the apps appear here.'}</EmptyState>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={filtered.length} onChange={setOffset}/>
    </Card>
  </div>;
}
