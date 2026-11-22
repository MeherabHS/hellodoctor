'use client';
import { tr } from '@/lib/i18n';
import {useEffect,useMemo,useState,type ReactNode} from 'react';
import {Activity,ArrowLeft,CheckCircle2,Eye,Inbox,MapPin,MessageSquare,Phone,UserCheck,UserX,Users,XCircle} from 'lucide-react';
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
  PENDING:{tone:'warn',get label(){return tr('ui.pendingReview')}},APPROVED:{tone:'success',get label(){return tr('ui.approved')}},
  REJECTED:{tone:'danger',get label(){return tr('ui.rejected')}},SUSPENDED:{tone:'neutral',get label(){return tr('ui.suspended')}}};
export const SESSION_STATUS:Record<AdminPeerSession['status'],{tone:ChipTone;label:string}>={
  PENDING:{tone:'warn',get label(){return tr('ui.waitingForPeer')}},ACCEPTED:{tone:'info',get label(){return tr('ui.accepted')}},
  COMPLETED:{tone:'success',get label(){return tr('ui.completed')}},DECLINED:{tone:'danger',get label(){return tr('ui.declined')}},CANCELLED:{tone:'neutral',get label(){return tr('ui.cancelledByPatient')}}};
const EVENT_LABEL:Record<string,string>={get CREATED(){return tr('ui2.signedUpInTheCareApp')},get APPROVED(){return tr('ui2.approved')},get REJECTED(){return tr('ui2.rejected')},get SUSPENDED(){return tr('ui2.suspended')}};
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
    <PageHead title={tr('ui.peerDirectory')} subtitle={tr('ui4.everyPeerNavigatorWhoTheyHave')}/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="blue" value={value(all.length)} label={tr('ui.peerNavigators')}/>
      <StatTile icon={UserCheck} tone="green" value={value(all.filter(p=>p.approval_status==='APPROVED').length)} label={tr('ui.approvedAndActive')}/>
      <StatTile icon={Activity} tone="green" value={value(sum('sessions_completed'))} label={tr('ui.sessionsCompleted')}/>
      <StatTile icon={MessageSquare} tone="purple" value={value(sum('patients_supported'))} label={tr('ui.patientsSupported')} note={tr('ui4.countedOncePerPeer')}/>
    </div>
    <section className="admin-card">
      <div className="afya-filter-bar">
        <SearchField label={tr('ui.searchPeerNavigators')} placeholder={tr('ui.searchByNameCountyOrganisationOr')} maxLength={150} value={search} onChange={v=>{setSearch(v);setOffset(0);}}/>
        <select className="afya-select" aria-label={tr('ui.status')} value={status} onChange={e=>{setStatus(e.target.value as ''|PeerApprovalStatus);setOffset(0);}}><option value="">{tr('ui.allStatuses')}</option>{(Object.keys(PEER_STATUS) as PeerApprovalStatus[]).map(s=><option key={s} value={s}>{PEER_STATUS[s].label}</option>)}</select>
      </div>
      {loadError&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{tr('ui.thePeerDirectoryCouldNotBe')} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>{tr('ui.retry')}</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>{tr('ui.peerNavigator')}</th><th>{tr('ui.county2')}</th><th>{tr('ui.patientsSupported')}</th><th>{tr('ui.sessions')}</th><th>{tr('ui.lastSession')}</th><th>{tr('ui.status')}</th><th style={{textAlign:'right'}}>{tr('ui.actions')}</th></tr></thead>
          <tbody>{rows.slice(offset,offset+PAGE).map(p=><tr key={p.id} className="admin-clickable-row" onClick={()=>setOpenId(p.id)}>
            <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden>{initialsOf(p.full_name)}</span><div><div style={{fontWeight:700}}>{p.full_name}</div>{p.organization&&<div className="afya-stat-note">{p.organization}</div>}</div></div></td>
            <td>{p.county}</td>
            <td>{p.patients_supported??0}</td>
            <td>{p.sessions_completed??0} completed<div className="afya-stat-note">{p.chat_sessions??0} chat · {p.call_sessions??0} call{(p.sessions_upcoming??0)>0?` · ${p.sessions_upcoming} upcoming`:''}</div></td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(p.last_session_at)}</td>
            <td><Chip tone={PEER_STATUS[p.approval_status].tone}>{PEER_STATUS[p.approval_status].label}</Chip></td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-icon-btn" aria-label={tr('tpl.viewRecordOf', { name: p.full_name })} onClick={e=>{e.stopPropagation();setOpenId(p.id);}}><Eye size={17}/></button></td>
          </tr>)}</tbody>
        </table>
        {peers&&!rows.length&&<EmptyState title={all.length?tr('ui2.noPeerNavigatorsMatchTheseFilters'):tr('ui2.noPeerNavigatorsYet')}>{all.length?tr('ui2.tryAnotherSearchOrStatus'):tr('ui2.peerNavigatorsAppearHereOnceThey')}</EmptyState>}
        {!peers&&!loadError&&<EmptyState title={tr('ui.loading')}/>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={rows.length} onChange={setOffset}/>
    </section>
  </div>;
}

/** What happened to the requests a peer received, like a doctor's consultation history. */
export function requestOutcomes(sessions:AdminPeerSession[]){
  const count=(f:(s:AdminPeerSession)=>boolean)=>sessions.filter(f).length;
  return {
    received:sessions.length,
    // Accepted at some point: still accepted, completed, or cancelled by the patient after acceptance.
    accepted:count(s=>s.status==='ACCEPTED'||s.status==='COMPLETED'||(s.status==='CANCELLED'&&!!s.decided_at)),
    declined:count(s=>s.status==='DECLINED'),
    cancelled:count(s=>s.status==='CANCELLED'),
    completed:count(s=>s.status==='COMPLETED'),
    waiting:count(s=>s.status==='PENDING'),
    chat:count(s=>s.support_type==='CHAT'&&s.status!=='DECLINED'&&s.status!=='PENDING'),
    call:count(s=>s.support_type==='CALL'&&s.status!=='DECLINED'&&s.status!=='PENDING'),
  };
}

type Tab='sessions'|'patients'|'timeline';

export function PeerRecord({peer,onBack,onOpenPatient,load=getAdminPeerHistory}:{peer:AdminPeerDirectoryRow;onBack:()=>void;onOpenPatient?:(patientId:string)=>void;load?:(id:string)=>Promise<AdminPeerHistory>}){
  const [history,setHistory]=useState<AdminPeerHistory|null>(null);const [error,setError]=useState(false);const [revision,setRevision]=useState(0);
  const [tab,setTab]=useState<Tab>('sessions');
  useEffect(()=>{let active=true;load(peer.id).then(h=>{if(active){setHistory(h);setError(false);}}).catch(()=>{if(active)setError(true);});return()=>{active=false;};},[peer.id,load,revision]);
  const sessions=useMemo(()=>history?.sessions??[],[history]);
  const patients=useMemo(()=>supportedPatients(sessions),[sessions]);
  const outcomes=useMemo(()=>requestOutcomes(sessions),[sessions]);
  const summary:[string,ReactNode][]=[
    ['County',peer.county],['Organisation',peer.organization||'—'],['Email',peer.email||'—'],['Phone',peer.phone_number],
    ['Signed up',eatDate(peer.created_at)],['Last reviewed',peer.reviewed_at?eatDate(peer.reviewed_at):tr('ui2.notYetReviewed')],
    ['Patients supported',String(peer.patients_supported??0)],
    ['Sessions',tr('tpl.sessionsBreakdown', { done: peer.sessions_completed??0, upcoming: peer.sessions_upcoming??0 })],
    ['Messages exchanged',String(peer.messages??0)],
  ];
  const patientCell=(id:string,name:string)=>onOpenPatient
    ?<button type="button" className="afya-link-btn" onClick={()=>onOpenPatient(id)}>{name}</button>
    :<span>{name}</span>;

  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>{tr('ui.backToPeerDirectory')}</button>
    <section className="admin-card afya-profile-head">
      <span className="afya-avatar lg" aria-hidden>{initialsOf(peer.full_name)}</span>
      <div style={{flex:1,minWidth:0}}>
        <div style={{display:'flex',alignItems:'center',gap:10,flexWrap:'wrap'}}><h2 style={{margin:0,fontSize:22,fontWeight:800}}>{peer.full_name}</h2><Chip tone={PEER_STATUS[peer.approval_status].tone}>{PEER_STATUS[peer.approval_status].label}</Chip></div>
        <div className="afya-profile-meta"><span>{tr('ui.peerNavigator')}</span><span><MapPin size={14} aria-hidden/>{peer.county}</span><span>{tr('mix.lastSession', { date: eatDate(peer.last_session_at) })}</span></div>
      </div>
    </section>
    <div className="afya-stat-grid" aria-label={tr('peerStats.label')}>
      <StatTile icon={Inbox} tone="blue" value={history?outcomes.received:'—'} label={tr('peerStats.received')} note={history&&outcomes.waiting?tr('peerStats.waiting',{count:outcomes.waiting}):undefined}/>
      <StatTile icon={UserCheck} tone="green" value={history?outcomes.accepted:'—'} label={tr('peerStats.accepted')} note={history?tr('peerStats.chatCall',{chat:outcomes.chat,call:outcomes.call}):undefined}/>
      <StatTile icon={UserX} tone="red" value={history?outcomes.declined:'—'} label={tr('peerStats.declined')}/>
      <StatTile icon={XCircle} tone="amber" value={history?outcomes.cancelled:'—'} label={tr('peerStats.cancelled')}/>
      <StatTile icon={CheckCircle2} tone="purple" value={history?outcomes.completed:'—'} label={tr('peerStats.completed')}/>
    </div>
    <div className="afya-two-col" style={{gridTemplateColumns:'minmax(260px,320px) minmax(0,1fr)'}}>
      <Card title={tr('ui.peerSummary')}>
        <dl className="afya-dl">{summary.map(([k,v])=><div key={k}><dt>{k}</dt><dd>{v}</dd></div>)}</dl>
        {peer.approval_note&&<p className="afya-stat-note" style={{marginBottom:0}}>{tr('mix.reviewNote')}{peer.approval_note}</p>}
      </Card>
      <section className="admin-card">
        <Tabs label={tr('ui.peerRecord')} value={tab} onChange={setTab} items={[
          {id:'sessions',label:tr('ui.supportSessions'),count:history?sessions.length:undefined},
          {id:'patients',label:tr('ui.patientsSupported'),count:history?patients.length:undefined},
          {id:'timeline',label:tr('ui.accountHistory'),count:history?history.status_history.length:undefined}]}/>
        {error&&<div className="afya-login-error" role="alert">{tr('ui.thisHistoryCouldNotBeLoaded')} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>{tr('ui.retry')}</button></div>}
        {!history&&!error&&<EmptyState title={tr('ui.loading')}/>}
        {history&&tab==='sessions'&&<div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>{tr('ui.sessionTime')}</th><th>{tr('ui.patient')}</th><th>{tr('ui.type')}</th><th>{tr('ui.status')}</th><th>{tr('ui.messages')}</th><th>{tr('ui.call')}</th></tr></thead>
          <tbody>{sessions.map(s=>{const st=SESSION_STATUS[s.status];const mins=callMinutes(s);return <tr key={s.request_id}>
            <td style={{whiteSpace:'nowrap'}}>{eatDateTime(s.start_time)}<div className="afya-stat-note">{tr('mix.requestedOn', { date: eatDate(s.requested_at) })}</div></td>
            <td>{patientCell(s.patient_id,s.patient_name)}<div className="afya-stat-note">{patientCode(s.patient_id)}</div></td>
            <td>{s.support_type==='CALL'?<span style={{display:'inline-flex',alignItems:'center',gap:6}}><Phone size={14} aria-hidden/>{tr('ui.call')}</span>:<span style={{display:'inline-flex',alignItems:'center',gap:6}}><MessageSquare size={14} aria-hidden/>{tr('ui.chat')}</span>}</td>
            <td><Chip tone={st.tone}>{st.label}</Chip></td>
            <td>{s.messages}</td>
            <td>{mins?`${mins} min`:s.call_started_at?tr('ui2.started'):'—'}</td>
          </tr>;})}</tbody>
        </table>{!sessions.length&&<EmptyState title={tr('ui.noSupportSessionsYet')}>{tr('ui.sessionsAppearHereOncePatientsRequest')}</EmptyState>}</div>}
        {history&&tab==='patients'&&<div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>{tr('ui.patient')}</th><th>{tr('ui.patientId')}</th><th>{tr('ui.sessions')}</th><th>{tr('ui.lastSession')}</th></tr></thead>
          <tbody>{patients.map(p=><tr key={p.patientId}>
            <td>{patientCell(p.patientId,p.name)}</td><td><Chip tone="outlined">{patientCode(p.patientId)}</Chip></td>
            <td>{p.sessions}<div className="afya-stat-note">{p.completed} completed</div></td><td style={{whiteSpace:'nowrap'}}>{eatDate(p.last)}</td>
          </tr>)}</tbody>
        </table>{!patients.length&&<EmptyState title={tr('ui.noPatientsSupportedYet')}>{tr('ui.patientsAppearHereOnceThisPeer')}</EmptyState>}</div>}
        {history&&tab==='timeline'&&(history.status_history.length?<ol className="afya-timeline">{history.status_history.map((e,i)=><li key={`${e.event}-${e.at}-${i}`}>
          <div className="afya-timeline-head"><strong>{EVENT_LABEL[e.event]??e.event}</strong><span className="afya-stat-note">{eatDateTime(e.at)}</span></div>
          <div className="afya-stat-note">{e.event==='CREATED'?tr('ui2.byThePeerNavigator'):e.by==='ADMIN'?tr('ui2.byAnAdmin'):'—'}</div>
        </li>)}</ol>:<EmptyState title={tr('ui.noAccountEventsYet')}/>)}
      </section>
    </div>
    <p className="afya-stat-note" style={{margin:0}}>{tr('ui.confidentialRecordTopicsPatientsWriteAnd')}</p>
  </div>;
}
