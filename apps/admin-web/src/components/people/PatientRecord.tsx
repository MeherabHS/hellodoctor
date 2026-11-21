'use client';
import { tr } from '@/lib/i18n';
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
  COMPLETED_WITH_RX:{tone:'success',get label(){return tr('ui.completedPrescriptionIssued')}},
  COMPLETED_NO_RX:{tone:'success',get label(){return tr('ui.completedNoPrescription')}},
  REFERRED:{tone:'info',get label(){return tr('ui.referred')}},
  ESCALATED:{tone:'warn',get label(){return tr('ui.escalated')}},
};
const STATUS_TONE:Record<string,ChipTone>={COMPLETED:'success',CANCELLED:'neutral',NO_SHOW:'danger',CONFIRMED:'info',WAITING:'info',IN_CONSULTATION:'purple'};
const title=(s:string)=>s?s.charAt(0)+s.slice(1).toLowerCase().replaceAll('_',' '):'—';
const modality=(m:string)=>m==='VIDEO'?tr('ui2.videoConsultation'):m==='CHAT'?'24h chat consultation':title(m);

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
    ['Patient ID',patient.id],['Gender',genderLabel(patient.gender)],['Date of birth',patient.dateOfBirth?`${eatDate(patient.dateOfBirth)}${age!==null?tr('tpl.ageYears', { age }):''}`:'—'],
    ['Registered',eatDate(patient.registeredAt)],['Last visit',eatDate(patient.lastConsultAt)],
    ['Consultations',tr('tpl.consultationsBreakdown', { total: patient.consultationsCount??0, video: patient.videoVisits??0, chat: patient.chatSessions??0 })],['Peer sessions',tr('tpl.peerBreakdown', { total: peerSessions.length, done: peerDone })],['Documents',String(docs.length)],
  ];

  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>{tr('ui.backToPatientsDirectory')}</button>
    <section className="admin-card afya-profile-head">
      <span className="afya-avatar lg" aria-hidden>{initialsOf(patient.name)}</span>
      <div style={{flex:1,minWidth:0}}>
        <div style={{display:'flex',alignItems:'center',gap:10,flexWrap:'wrap'}}><h2 style={{margin:0,fontSize:22,fontWeight:800}}>{patient.name}</h2><Chip tone={activity.tone}>{activity.label}</Chip></div>
        <div className="afya-profile-meta"><span>{patient.id}</span><span>{genderLabel(patient.gender)}{age!==null?tr('mix.ageSuffix', { age }):''}</span><span>{tr('mix.registeredOn', { date: eatDate(patient.registeredAt) })}</span></div>
      </div>
      <div style={{display:'flex',gap:8,flexWrap:'wrap'}}>
        <button type="button" className="afya-btn-secondary" disabled={busy} onClick={onExport}><Download size={16} aria-hidden/>{busy?tr('ui2.working'):tr('ui2.exportDataJson2')}</button>
        <button type="button" className="afya-btn-danger" disabled={busy} onClick={onErase}><ShieldAlert size={16} aria-hidden/>{tr('ui.eraseAnonymize')}</button>
      </div>
    </section>

    <div className="afya-two-col" style={{gridTemplateColumns:'minmax(260px,320px) minmax(0,1fr)'}}>
      <Card title={tr('ui.patientSummary')}>
        <dl className="afya-dl">{summary.map(([k,v])=><div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}</dl>
      </Card>
      <section className="admin-card">
        <Tabs label={tr('ui.patientRecord')} value={tab} onChange={setTab} items={[{id:'timeline',label:tr('ui.visitsTimeline')},{id:'consultations',label:tr('ui.consultations'),count:visits.length+peerSessions.length},{id:'attachments',label:tr('ui.attachments'),count:docs.length}]}/>
        {tab==='timeline'&&(timeline.length?<ol className="afya-timeline">{timeline.map(item=>{if(item.kind==='peer'){const p=item.p;const st=SESSION_STATUS[p.status];return <li key={`peer-${p.request_id}`}>
          <div className="afya-timeline-head"><strong style={{display:'inline-flex',alignItems:'center',gap:6}}><HeartHandshake size={15} aria-hidden/>{eatDate(p.start_time)} · Peer support {p.support_type==='CALL'?'call':'chat'}</strong><Chip tone={st.tone}>{st.label}</Chip></div>
          <div className="afya-stat-note">{tr('mix.withPeer', { name: p.peer_name })}{p.messages?tr('mix.messagesCount', { count: p.messages }):''}</div>
        </li>;}const v=item.v;const out=v.outcome?OUTCOME[v.outcome]:null;return <li key={v.appointmentId}>
          <div className="afya-timeline-head"><strong>{eatDate(v.createdAt)} · {modality(v.modality)}</strong>{out?<Chip tone={out.tone}>{out.label}</Chip>:<Chip tone={STATUS_TONE[v.status]??'neutral'}>{title(v.status)}</Chip>}</div>
          <div className="afya-stat-note">{tr('mix.withName', { name: v.doctor })}{v.appointmentNumber?` · ${v.appointmentNumber}`:''}</div>
        </li>;})}</ol>:<EmptyState title={tr('ui.noVisitsYet')}>{tr('ui.consultationsAndPeerSupportSessionsAppea')}</EmptyState>)}
        {tab==='consultations'&&<div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>{tr('ui.date')}</th><th>{tr('ui.reference')}</th><th>{tr('ui.doctor')}</th><th>{tr('ui.type')}</th><th>{tr('ui.status')}</th><th>{tr('ui.outcome')}</th></tr></thead>
          <tbody>{visits.map(v=><tr key={v.appointmentId}><td style={{whiteSpace:'nowrap'}}>{eatDate(v.createdAt)}</td><td>{v.appointmentNumber||'—'}</td><td>{v.doctor}</td><td>{v.modality==='VIDEO'?tr('ui2.video'):v.modality==='CHAT'?tr('ui2.chat'):title(v.modality)}</td><td><Chip tone={STATUS_TONE[v.status]??'neutral'}>{title(v.status)}</Chip></td><td>{v.outcome?OUTCOME[v.outcome]?.label??title(v.outcome):'—'}</td></tr>)}</tbody>
        </table>{!visits.length&&<EmptyState title={tr('ui.noConsultationsYet')}/>}
        <h3 className="afya-subhead" style={{margin:'20px 0 8px',fontSize:15,fontWeight:800}}>{tr('ui.peerSupportSessions')}</h3>
        {peerError&&<div className="afya-login-error" role="alert">{tr('ui.peerSessionsCouldNotBeLoaded')}</div>}
        <table className="admin-table">
          <thead><tr><th>{tr('ui.date')}</th><th>{tr('ui.peerNavigator')}</th><th>{tr('ui.type')}</th><th>{tr('ui.status')}</th><th>{tr('ui.messages')}</th></tr></thead>
          <tbody>{peerSessions.map(p=><tr key={p.request_id}><td style={{whiteSpace:'nowrap'}}>{eatDate(p.start_time)}</td><td>{p.peer_name}</td><td>{p.support_type==='CALL'?tr('ui2.call'):tr('ui2.chat')}</td><td><Chip tone={SESSION_STATUS[p.status].tone}>{SESSION_STATUS[p.status].label}</Chip></td><td>{p.messages}</td></tr>)}</tbody>
        </table>{!peerSessions.length&&!peerError&&<EmptyState title={tr('ui.noPeerSupportSessionsYet')}/>}</div>}
        {tab==='attachments'&&(docs.length?<div className="afya-doc-grid">{docs.map(d=><div key={d.id} className="afya-doc">
          <FileText size={22} aria-hidden color="var(--afya-danger-fg)"/>
          <div style={{flex:1,minWidth:0}}><div style={{fontWeight:700}}>{d.mimeType==='application/pdf'?tr('ui2.pdfDocument'):tr('ui2.image')} · {(d.fileSizeBytes/1024).toFixed(0)} KB</div>
            <div className="afya-stat-note">{tr('mix.uploadedOn', { date: eatDate(d.createdAt) })}</div></div>
          {d.securityStatus==='CLEAN'?<button type="button" className="afya-btn-secondary" onClick={()=>window.open(`/api/documents/download?appointment_id=${encodeURIComponent(d.appointmentId)}&document_id=${encodeURIComponent(d.id)}`,'_blank')}>{tr('ui.view')}</button>:<Chip tone="danger">{tr('ui.notClearedForViewing')}</Chip>}
        </div>)}</div>:<EmptyState title={tr('ui.noDocuments')}>{tr('ui.prescriptionOrLabPhotosThePatient')}</EmptyState>)}
      </section>
    </div>
    <p className="afya-stat-note" style={{margin:0}}>{tr('ui.confidentialRecordAccessIsAudited')}</p>
  </div>;
}
