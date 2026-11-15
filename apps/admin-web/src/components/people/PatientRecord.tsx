'use client';
import {useEffect,useMemo,useState,type ReactNode} from 'react';
import {ArrowLeft,Download,FileText,HeartHandshake,ShieldAlert} from 'lucide-react';
import type {PatientRecordItem} from '../../data/adminStore';
import {getAdminPatientPeerSessions,type AdminPatientPeerSession} from '../../lib/api';
import {SESSION_STATUS} from './PeerDirectory';
import {Card,Chip,EmptyState,Tabs,eatDate,type ChipTone} from '../ui/afya';
import {ACTIVITY_CHIP,activityOf,ageFrom,genderLabel,initialsOf} from './PatientsDirectory';

/**
 * People & Care → patient record (People & Care board, panel 2): profile,
 * visit timeline, consultations and uploaded documents. No conditions, care
 * plan, risk level, prescriptions or chat (product owner, 2026-11-14).
 * Peer Navigator sessions are listed with the doctor consultations
 * (2026-11-15): date, peer, chat or call, status. Never the topic the patient
 * wrote or any message. Opening the record is audited by the server when it
 * loads the patient's dossier.
 */
type Tab='timeline'|'consultations'|'attachments';
const OUTCOME:Record<string,{tone:ChipTone;label:string}>={
  COMPLETED_WITH_RX:{tone:'success',label:'Completed · prescription issued'},
  COMPLETED_NO_RX:{tone:'success',label:'Completed · no prescription'},
  REFERRED:{tone:'info',label:'Referred'},
  ESCALATED:{tone:'warn',label:'Escalated'},
};
const STATUS_TONE:Record<string,ChipTone>={COMPLETED:'success',CANCELLED:'neutral',NO_SHOW:'danger',CONFIRMED:'info',WAITING:'info',IN_CONSULTATION:'purple'};
const title=(s:string)=>s?s.charAt(0)+s.slice(1).toLowerCase().replaceAll('_',' '):'—';
const modality=(m:string)=>m==='VIDEO'?'Video consultation':m==='CHAT'?'24h chat consultation':title(m);

export function PatientRecord({patient,onBack,onExport,onErase,busy,loadPeerSessions=getAdminPatientPeerSessions}:{patient:PatientRecordItem;onBack:()=>void;onExport:()=>void;onErase:()=>void;busy:boolean;loadPeerSessions?:(patientId:string)=>Promise<AdminPatientPeerSession[]>}){
  const [tab,setTab]=useState<Tab>('timeline');
  const patientId=patient.backendId??patient.key;
  const [peerSessions,setPeerSessions]=useState<AdminPatientPeerSession[]>([]);
  const [peerError,setPeerError]=useState(false);
  useEffect(()=>{let active=true;loadPeerSessions(patientId).then(r=>{if(active){setPeerSessions(r);setPeerError(false);}}).catch(()=>{if(active)setPeerError(true);});return()=>{active=false;};},[patientId,loadPeerSessions]);
  const visits=[...(patient.visits??[])].sort((a,b)=>(b.createdAt??'').localeCompare(a.createdAt??''));
  // Doctor consultations and peer sessions on one timeline, newest first.
  const timeline=useMemo(()=>[
    ...visits.map(v=>({kind:'doctor' as const,at:v.createdAt??'',v})),
    ...peerSessions.map(p=>({kind:'peer' as const,at:p.start_time,p})),
  ].sort((a,b)=>b.at.localeCompare(a.at)),[visits,peerSessions]);
  const peerDone=peerSessions.filter(p=>p.status==='COMPLETED').length;
  const docs=patient.vaultRecords??[];
  const [now]=useState(()=>Date.now());
  const age=ageFrom(patient.dateOfBirth,now);const activity=ACTIVITY_CHIP[activityOf(patient,now)];
  const summary:[string,ReactNode][]=[
    ['Patient ID',patient.id],['Gender',genderLabel(patient.gender)],['Date of birth',patient.dateOfBirth?`${eatDate(patient.dateOfBirth)}${age!==null?` (${age} years)`:''}`:'—'],
    ['Registered',eatDate(patient.registeredAt)],['Last visit',eatDate(patient.lastConsultAt)],
    ['Consultations',`${patient.consultationsCount??0} (${patient.videoVisits??0} video, ${patient.chatSessions??0} chat)`],['Peer sessions',`${peerSessions.length} (${peerDone} completed)`],['Documents',String(docs.length)],
  ];

  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>Back to Patients Directory</button>
    <section className="admin-card afya-profile-head">
      <span className="afya-avatar lg" aria-hidden>{initialsOf(patient.name)}</span>
      <div style={{flex:1,minWidth:0}}>
        <div style={{display:'flex',alignItems:'center',gap:10,flexWrap:'wrap'}}><h2 style={{margin:0,fontSize:22,fontWeight:800}}>{patient.name}</h2><Chip tone={activity.tone}>{activity.label}</Chip></div>
        <div className="afya-profile-meta"><span>{patient.id}</span><span>{genderLabel(patient.gender)}{age!==null?`, ${age} years`:''}</span><span>Registered {eatDate(patient.registeredAt)}</span></div>
      </div>
      <div style={{display:'flex',gap:8,flexWrap:'wrap'}}>
        <button type="button" className="afya-btn-secondary" disabled={busy} onClick={onExport}><Download size={16} aria-hidden/>{busy?'Working…':'Export data (JSON)'}</button>
        <button type="button" className="afya-btn-danger" disabled={busy} onClick={onErase}><ShieldAlert size={16} aria-hidden/>Erase / anonymize</button>
      </div>
    </section>

    <div className="afya-two-col" style={{gridTemplateColumns:'minmax(260px,320px) minmax(0,1fr)'}}>
      <Card title="Patient summary">
        <dl className="afya-dl">{summary.map(([k,v])=><div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}</dl>
      </Card>
      <section className="admin-card">
        <Tabs label="Patient record" value={tab} onChange={setTab} items={[{id:'timeline',label:'Visits & Timeline'},{id:'consultations',label:'Consultations',count:visits.length+peerSessions.length},{id:'attachments',label:'Attachments',count:docs.length}]}/>
        {tab==='timeline'&&(timeline.length?<ol className="afya-timeline">{timeline.map(item=>{if(item.kind==='peer'){const p=item.p;const st=SESSION_STATUS[p.status];return <li key={`peer-${p.request_id}`}>
          <div className="afya-timeline-head"><strong style={{display:'inline-flex',alignItems:'center',gap:6}}><HeartHandshake size={15} aria-hidden/>{eatDate(p.start_time)} · Peer support {p.support_type==='CALL'?'call':'chat'}</strong><Chip tone={st.tone}>{st.label}</Chip></div>
          <div className="afya-stat-note">With {p.peer_name} (Peer Navigator){p.messages?` · ${p.messages} messages`:''}</div>
        </li>;}const v=item.v;const out=v.outcome?OUTCOME[v.outcome]:null;return <li key={v.appointmentId}>
          <div className="afya-timeline-head"><strong>{eatDate(v.createdAt)} · {modality(v.modality)}</strong>{out?<Chip tone={out.tone}>{out.label}</Chip>:<Chip tone={STATUS_TONE[v.status]??'neutral'}>{title(v.status)}</Chip>}</div>
          <div className="afya-stat-note">With {v.doctor}{v.appointmentNumber?` · ${v.appointmentNumber}`:''}</div>
        </li>;})}</ol>:<EmptyState title="No visits yet">Consultations and peer support sessions appear here once booked.</EmptyState>)}
        {tab==='consultations'&&<div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>Date</th><th>Reference</th><th>Doctor</th><th>Type</th><th>Status</th><th>Outcome</th></tr></thead>
          <tbody>{visits.map(v=><tr key={v.appointmentId}><td style={{whiteSpace:'nowrap'}}>{eatDate(v.createdAt)}</td><td>{v.appointmentNumber||'—'}</td><td>{v.doctor}</td><td>{v.modality==='VIDEO'?'Video':v.modality==='CHAT'?'Chat':title(v.modality)}</td><td><Chip tone={STATUS_TONE[v.status]??'neutral'}>{title(v.status)}</Chip></td><td>{v.outcome?OUTCOME[v.outcome]?.label??title(v.outcome):'—'}</td></tr>)}</tbody>
        </table>{!visits.length&&<EmptyState title="No consultations yet"/>}
        <h3 className="afya-subhead" style={{margin:'20px 0 8px',fontSize:15,fontWeight:800}}>Peer support sessions</h3>
        {peerError&&<div className="afya-login-error" role="alert">Peer sessions could not be loaded.</div>}
        <table className="admin-table">
          <thead><tr><th>Date</th><th>Peer Navigator</th><th>Type</th><th>Status</th><th>Messages</th></tr></thead>
          <tbody>{peerSessions.map(p=><tr key={p.request_id}><td style={{whiteSpace:'nowrap'}}>{eatDate(p.start_time)}</td><td>{p.peer_name}</td><td>{p.support_type==='CALL'?'Call':'Chat'}</td><td><Chip tone={SESSION_STATUS[p.status].tone}>{SESSION_STATUS[p.status].label}</Chip></td><td>{p.messages}</td></tr>)}</tbody>
        </table>{!peerSessions.length&&!peerError&&<EmptyState title="No peer support sessions yet"/>}</div>}
        {tab==='attachments'&&(docs.length?<div className="afya-doc-grid">{docs.map(d=><div key={d.id} className="afya-doc">
          <FileText size={22} aria-hidden color="var(--afya-danger-fg)"/>
          <div style={{flex:1,minWidth:0}}><div style={{fontWeight:700}}>{d.mimeType==='application/pdf'?'PDF document':'Image'} · {(d.fileSizeBytes/1024).toFixed(0)} KB</div>
            <div className="afya-stat-note">Uploaded {eatDate(d.createdAt)}</div></div>
          {d.securityStatus==='CLEAN'?<button type="button" className="afya-btn-secondary" onClick={()=>window.open(`/api/documents/download?appointment_id=${encodeURIComponent(d.appointmentId)}&document_id=${encodeURIComponent(d.id)}`,'_blank')}>View</button>:<Chip tone="danger">Not cleared for viewing</Chip>}
        </div>)}</div>:<EmptyState title="No documents">Prescription or lab photos the patient uploads for a consultation appear here.</EmptyState>)}
      </section>
    </div>
    <p className="afya-stat-note" style={{margin:0}}>Confidential record. Access is audited.</p>
  </div>;
}
