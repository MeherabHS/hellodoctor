'use client';
import {useEffect,useMemo,useState,type ReactNode} from 'react';
import {Activity,ArrowLeft,Eye,MapPin,MessageSquare,Phone,UserCheck,Users} from 'lucide-react';
import {getAdminPeerDirectory,getAdminPeerHistory,type AdminPeerDirectoryRow,type AdminPeerHistory,type AdminPeerSession,type PeerApprovalStatus} from '../../lib/api';
import {buildSearchIndex,searchIndex} from '../../lib/searchIndex';
import {Card,Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,Tabs,eatDate,type ChipTone} from '../ui/afya';
import {initialsOf} from './PatientsDirectory';

/**
 * People & Care → Peer Directory (2026-11-15): every Peer Navigator with
 * their activity, and a record page with their account history and the
 * patients they have supported. Approval decisions stay on the Peer
 * Navigators page. Never shows the topic a patient wrote (it can describe a
 * health condition) or any chat message; the backend does not send them.
 */
const PAGE=20;
export const PEER_STATUS:Record<PeerApprovalStatus,{tone:ChipTone;label:string}>={
  PENDING:{tone:'warn',label:'Pending review'},APPROVED:{tone:'success',label:'Approved'},
  REJECTED:{tone:'danger',label:'Rejected'},SUSPENDED:{tone:'neutral',label:'Suspended'}};
export const SESSION_STATUS:Record<AdminPeerSession['status'],{tone:ChipTone;label:string}>={
  PENDING:{tone:'warn',label:'Waiting for peer'},ACCEPTED:{tone:'info',label:'Accepted'},
  COMPLETED:{tone:'success',label:'Completed'},DECLINED:{tone:'danger',label:'Declined'},CANCELLED:{tone:'neutral',label:'Cancelled by patient'}};
const EVENT_LABEL:Record<string,string>={CREATED:'Signed up in the Care app',APPROVED:'Approved',REJECTED:'Rejected',SUSPENDED:'Suspended'};
export const patientCode=(id:string)=>`#PT-${id.slice(0,5).toUpperCase()}`;
export function eatDateTime(value:string|null|undefined):string{
  if(!value)return '—';const d=new Date(value);if(!Number.isFinite(d.getTime()))return '—';
  return `${eatDate(value)}, ${d.toLocaleTimeString('en-US',{timeZone:'Africa/Nairobi',hour:'numeric',minute:'2-digit'})}`;
}
const callMinutes=(s:AdminPeerSession)=>{
  if(!s.call_started_at||!s.call_ended_at)return null;
  const ms=new Date(s.call_ended_at).getTime()-new Date(s.call_started_at).getTime();
  return Number.isFinite(ms)&&ms>=0?Math.max(1,Math.round(ms/60000)):null;
};
type Supported={patientId:string;name:string;sessions:number;completed:number;last:string};
/** The distinct patients a peer has supported (accepted or completed sessions). */
export function supportedPatients(sessions:AdminPeerSession[]):Supported[]{
  const by=new Map<string,Supported>();
  for(const s of sessions){
    if(s.status!=='ACCEPTED'&&s.status!=='COMPLETED')continue;
    const p=by.get(s.patient_id)??{patientId:s.patient_id,name:s.patient_name,sessions:0,completed:0,last:s.start_time};
    p.sessions++;if(s.status==='COMPLETED')p.completed++;if(s.start_time>p.last)p.last=s.start_time;
    by.set(s.patient_id,p);
  }
  return [...by.values()].sort((a,b)=>b.last.localeCompare(a.last));
}

export function PeerDirectory({onOpenPatient}:{onOpenPatient?:(patientId:string)=>void}){
  const [peers,setPeers]=useState<AdminPeerDirectoryRow[]|null>(null);const [loadError,setLoadError]=useState(false);const [revision,setRevision]=useState(0);
  const [search,setSearch]=useState('');const [status,setStatus]=useState<''|PeerApprovalStatus>('');const [offset,setOffset]=useState(0);
  const [openId,setOpenId]=useState<string|null>(null);
  useEffect(()=>{let active=true;getAdminPeerDirectory().then(p=>{if(active){setPeers(p);setLoadError(false);}}).catch(()=>{if(active)setLoadError(true);});return()=>{active=false;};},[revision]);
  // Indexed once per load: name, county, organisation, email and phone.
  const index=useMemo(()=>buildSearchIndex(peers??[],p=>[p.full_name,p.county,p.organization,p.email,p.phone_number]),[peers]);
  const rows=useMemo(()=>searchIndex(index,search).filter(p=>!status||p.approval_status===status).sort((a,b)=>(b.last_session_at??'').localeCompare(a.last_session_at??'')||a.full_name.localeCompare(b.full_name)),[index,search,status]);
  const open=(peers??[]).find(p=>p.id===openId)??null;
  if(open)return <PeerRecord peer={open} onBack={()=>setOpenId(null)} onOpenPatient={onOpenPatient}/>;

  const all=peers??[];const sum=(k:keyof AdminPeerDirectoryRow)=>all.reduce((n,p)=>n+Number(p[k]??0),0);
  const value=(n:number)=>peers?n:'—';
  return <div className="afya-stack">
    <PageHead title="Peer Directory" subtitle="Every Peer Navigator, who they have supported and how often. Approve new sign-ups on the Peer Navigators page."/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="blue" value={value(all.length)} label="Peer Navigators"/>
      <StatTile icon={UserCheck} tone="green" value={value(all.filter(p=>p.approval_status==='APPROVED').length)} label="Approved and active"/>
      <StatTile icon={Activity} tone="green" value={value(sum('sessions_completed'))} label="Sessions completed"/>
      <StatTile icon={MessageSquare} tone="purple" value={value(sum('patients_supported'))} label="Patients supported" note="Counted once per peer"/>
    </div>
    <section className="admin-card">
      <div className="afya-filter-bar">
        <SearchField label="Search Peer Navigators" placeholder="Search by name, county, organisation or email…" maxLength={150} value={search} onChange={v=>{setSearch(v);setOffset(0);}}/>
        <select className="afya-select" aria-label="Status" value={status} onChange={e=>{setStatus(e.target.value as ''|PeerApprovalStatus);setOffset(0);}}><option value="">All statuses</option>{(Object.keys(PEER_STATUS) as PeerApprovalStatus[]).map(s=><option key={s} value={s}>{PEER_STATUS[s].label}</option>)}</select>
      </div>
      {loadError&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>The Peer Directory could not be loaded. <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Peer Navigator</th><th>County</th><th>Patients supported</th><th>Sessions</th><th>Last session</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{rows.slice(offset,offset+PAGE).map(p=><tr key={p.id} className="admin-clickable-row" onClick={()=>setOpenId(p.id)}>
            <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden>{initialsOf(p.full_name)}</span><div><div style={{fontWeight:700}}>{p.full_name}</div>{p.organization&&<div className="afya-stat-note">{p.organization}</div>}</div></div></td>
            <td>{p.county}</td>
            <td>{p.patients_supported??0}</td>
            <td>{p.sessions_completed??0} completed<div className="afya-stat-note">{p.chat_sessions??0} chat · {p.call_sessions??0} call{(p.sessions_upcoming??0)>0?` · ${p.sessions_upcoming} upcoming`:''}</div></td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(p.last_session_at)}</td>
            <td><Chip tone={PEER_STATUS[p.approval_status].tone}>{PEER_STATUS[p.approval_status].label}</Chip></td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-icon-btn" aria-label={`View record of ${p.full_name}`} onClick={e=>{e.stopPropagation();setOpenId(p.id);}}><Eye size={17}/></button></td>
          </tr>)}</tbody>
        </table>
        {peers&&!rows.length&&<EmptyState title={all.length?'No Peer Navigators match these filters':'No Peer Navigators yet'}>{all.length?'Try another search or status.':'Peer Navigators appear here once they sign up in the Care app.'}</EmptyState>}
        {!peers&&!loadError&&<EmptyState title="Loading…"/>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={rows.length} onChange={setOffset}/>
    </section>
  </div>;
}

type Tab='sessions'|'patients'|'timeline';

export function PeerRecord({peer,onBack,onOpenPatient,load=getAdminPeerHistory}:{peer:AdminPeerDirectoryRow;onBack:()=>void;onOpenPatient?:(patientId:string)=>void;load?:(id:string)=>Promise<AdminPeerHistory>}){
  const [history,setHistory]=useState<AdminPeerHistory|null>(null);const [error,setError]=useState(false);const [revision,setRevision]=useState(0);
  const [tab,setTab]=useState<Tab>('sessions');
  useEffect(()=>{let active=true;load(peer.id).then(h=>{if(active){setHistory(h);setError(false);}}).catch(()=>{if(active)setError(true);});return()=>{active=false;};},[peer.id,load,revision]);
  const sessions=useMemo(()=>history?.sessions??[],[history]);
  const patients=useMemo(()=>supportedPatients(sessions),[sessions]);
  const summary:[string,ReactNode][]=[
    ['County',peer.county],['Organisation',peer.organization||'—'],['Email',peer.email||'—'],['Phone',peer.phone_number],
    ['Signed up',eatDate(peer.created_at)],['Last reviewed',peer.reviewed_at?eatDate(peer.reviewed_at):'Not yet reviewed'],
    ['Patients supported',String(peer.patients_supported??0)],
    ['Sessions',`${peer.sessions_completed??0} completed, ${peer.sessions_upcoming??0} upcoming`],
    ['Messages exchanged',String(peer.messages??0)],
  ];
  const patientCell=(id:string,name:string)=>onOpenPatient
    ?<button type="button" className="afya-link-btn" onClick={()=>onOpenPatient(id)}>{name}</button>
    :<span>{name}</span>;

  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>Back to Peer Directory</button>
    <section className="admin-card afya-profile-head">
      <span className="afya-avatar lg" aria-hidden>{initialsOf(peer.full_name)}</span>
      <div style={{flex:1,minWidth:0}}>
        <div style={{display:'flex',alignItems:'center',gap:10,flexWrap:'wrap'}}><h2 style={{margin:0,fontSize:22,fontWeight:800}}>{peer.full_name}</h2><Chip tone={PEER_STATUS[peer.approval_status].tone}>{PEER_STATUS[peer.approval_status].label}</Chip></div>
        <div className="afya-profile-meta"><span>Peer Navigator</span><span><MapPin size={14} aria-hidden/>{peer.county}</span><span>Last session {eatDate(peer.last_session_at)}</span></div>
      </div>
    </section>
    <div className="afya-two-col" style={{gridTemplateColumns:'minmax(260px,320px) minmax(0,1fr)'}}>
      <Card title="Peer summary">
        <dl className="afya-dl">{summary.map(([k,v])=><div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}</dl>
        {peer.approval_note&&<p className="afya-stat-note" style={{marginBottom:0}}>Review note: {peer.approval_note}</p>}
      </Card>
      <section className="admin-card">
        <Tabs label="Peer record" value={tab} onChange={setTab} items={[
          {id:'sessions',label:'Support sessions',count:history?sessions.length:undefined},
          {id:'patients',label:'Patients supported',count:history?patients.length:undefined},
          {id:'timeline',label:'Account history',count:history?history.status_history.length:undefined}]}/>
        {error&&<div className="afya-login-error" role="alert">This history could not be loaded. <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
        {!history&&!error&&<EmptyState title="Loading…"/>}
        {history&&tab==='sessions'&&<div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>Session time</th><th>Patient</th><th>Type</th><th>Status</th><th>Messages</th><th>Call</th></tr></thead>
          <tbody>{sessions.map(s=>{const st=SESSION_STATUS[s.status];const mins=callMinutes(s);return <tr key={s.request_id}>
            <td style={{whiteSpace:'nowrap'}}>{eatDateTime(s.start_time)}<div className="afya-stat-note">Requested {eatDate(s.requested_at)}</div></td>
            <td>{patientCell(s.patient_id,s.patient_name)}<div className="afya-stat-note">{patientCode(s.patient_id)}</div></td>
            <td>{s.support_type==='CALL'?<span style={{display:'inline-flex',alignItems:'center',gap:6}}><Phone size={14} aria-hidden/>Call</span>:<span style={{display:'inline-flex',alignItems:'center',gap:6}}><MessageSquare size={14} aria-hidden/>Chat</span>}</td>
            <td><Chip tone={st.tone}>{st.label}</Chip></td>
            <td>{s.messages}</td>
            <td>{mins?`${mins} min`:s.call_started_at?'Started':'—'}</td>
          </tr>;})}</tbody>
        </table>{!sessions.length&&<EmptyState title="No support sessions yet">Sessions appear here once patients request this Peer Navigator.</EmptyState>}</div>}
        {history&&tab==='patients'&&<div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>Patient</th><th>Patient ID</th><th>Sessions</th><th>Last session</th></tr></thead>
          <tbody>{patients.map(p=><tr key={p.patientId}>
            <td>{patientCell(p.patientId,p.name)}</td><td><Chip tone="outlined">{patientCode(p.patientId)}</Chip></td>
            <td>{p.sessions}<div className="afya-stat-note">{p.completed} completed</div></td><td style={{whiteSpace:'nowrap'}}>{eatDate(p.last)}</td>
          </tr>)}</tbody>
        </table>{!patients.length&&<EmptyState title="No patients supported yet">Patients appear here once this Peer Navigator accepts their request.</EmptyState>}</div>}
        {history&&tab==='timeline'&&(history.status_history.length?<ol className="afya-timeline">{history.status_history.map((e,i)=><li key={`${e.event}-${e.at}-${i}`}>
          <div className="afya-timeline-head"><strong>{EVENT_LABEL[e.event]??e.event}</strong><span className="afya-stat-note">{eatDateTime(e.at)}</span></div>
          <div className="afya-stat-note">{e.event==='CREATED'?'By the Peer Navigator':e.by==='ADMIN'?'By an Admin':'—'}</div>
        </li>)}</ol>:<EmptyState title="No account events yet"/>)}
      </section>
    </div>
    <p className="afya-stat-note" style={{margin:0}}>Confidential record. Topics patients write and chat messages are private to the patient and the Peer Navigator and are never shown here. Viewing this record is audited.</p>
  </div>;
}
