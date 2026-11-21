'use client';
import { tr, type MessageKey } from '@/lib/i18n';
import {useEffect,useMemo,useState,type ReactNode} from 'react';
import {ArrowLeft,BadgeCheck,Ban,Clock,Eye,MapPin,RotateCcw,UserCheck,Users,XCircle} from 'lucide-react';
import {decideAdminPeer,getAdminPeers,recordAdminView,type AdminPeer,type PeerApprovalStatus,type PeerDecision} from '../../lib/api';
import {blogErrorMessage} from '../../lib/blogErrors';
import {buildSearchIndex,searchIndex} from '../../lib/searchIndex';
import {Card,Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,Tabs,eatDate,type ChipTone} from '../ui/afya';
import {initialsOf} from './PatientsDirectory';

/**
 * People & Care → Peer Navigators (docs/peer-navigator/PEER_NAVIGATOR_PLAN.md,
 * Phase 6). Same layout as Doctor Verification, without KMPDC. Peers sign up in
 * the Care app; an Admin approves before patients can see or book them.
 * Rejecting or suspending needs a note (the backend enforces it too). Admins
 * never see peer chat content or the patients a peer supports.
 */
type TabId=PeerApprovalStatus|'ALL';
const STATUS:Record<PeerApprovalStatus,{tone:ChipTone;label:string}>={
  PENDING:{tone:'warn',get label(){return tr('ui.pendingReview')}},APPROVED:{tone:'success',get label(){return tr('ui.approved')}},
  REJECTED:{tone:'danger',get label(){return tr('ui.rejected')}},SUSPENDED:{tone:'neutral',get label(){return tr('ui.suspended')}}};
const PAGE=20;
const NOTE_MAX=500;
/** Which decisions each status allows, mirroring `admin_decide` in the backend. */
export const PEER_ACTIONS:Record<PeerApprovalStatus,PeerDecision[]>={PENDING:['reject','approve'],APPROVED:['suspend'],REJECTED:['approve'],SUSPENDED:['reinstate']};
export const peerDecisionNeedsNote=(a:PeerDecision)=>a==='reject'||a==='suspend';
const ACTION_LABEL:Record<PeerDecision,string>={get approve(){return tr('ui2.approve')},get reject(){return tr('ui2.reject')},get suspend(){return tr('ui2.suspend')},get reinstate(){return tr('ui2.reinstate')}};

function Field({label,children}:{label:string;children:ReactNode}){return <div><dt>{label}</dt><dd>{children||'—'}</dd></div>;}

export function PeerNavigators({onChanged}:{onChanged?:(pending:number)=>void}){
  const [peers,setPeers]=useState<AdminPeer[]|null>(null);const [loadError,setLoadError]=useState(false);const [revision,setRevision]=useState(0);
  const [tab,setTab]=useState<TabId>('PENDING');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);
  const [openId,setOpenId]=useState<string|null>(null);
  useEffect(()=>{let active=true;getAdminPeers().then(p=>{if(active){setPeers(p);setLoadError(false);onChanged?.(p.filter(x=>x.approval_status==='PENDING').length);}}).catch(()=>{if(active)setLoadError(true);});return()=>{active=false;};},[revision,onChanged]);

  const rows=useMemo(()=>peers??[],[peers]);
  const count=(s:PeerApprovalStatus)=>rows.filter(r=>r.approval_status===s).length;
  const index=useMemo(()=>buildSearchIndex(rows,r=>[r.full_name,r.county,r.organization,r.email,r.phone_number]),[rows]);
  const filtered=searchIndex(index,search).filter(r=>tab==='ALL'||r.approval_status===tab);
  const open=rows.find(r=>r.id===openId)??null;

  if(open)return <PeerDetail peer={open} onBack={()=>setOpenId(null)} onDecided={()=>{setOpenId(null);setRevision(v=>v+1);}}/>;

  const value=(n:number)=>peers?n:'—';
  return <div className="afya-stack">
    <PageHead title={tr('tpl.peersPending', { count: peers?count('PENDING'):'—' })} subtitle={tr('ui4.approvePeerNavigatorsBeforePatientsCan')}/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="blue" value={value(rows.length)} label={tr('ui.totalApplications')}/>
      <StatTile icon={Clock} tone="amber" value={value(count('PENDING'))} label={tr('ui.pendingReview')}/>
      <StatTile icon={BadgeCheck} tone="green" value={value(count('APPROVED'))} label={tr('ui.approved')}/>
      <StatTile icon={XCircle} tone="red" value={value(count('REJECTED')+count('SUSPENDED'))} label={tr('ui.rejectedOrSuspended')}/>
    </div>
    <section className="admin-card">
      <Tabs label={tr('ui.peerStatus')} value={tab} onChange={v=>{setTab(v);setOffset(0);}} items={[
        {id:'PENDING',label:tr('ui.pendingReview'),count:peers?count('PENDING'):undefined},{id:'APPROVED',label:tr('ui.approved'),count:peers?count('APPROVED'):undefined},
        {id:'REJECTED',label:tr('ui.rejected'),count:peers?count('REJECTED'):undefined},{id:'SUSPENDED',label:tr('ui.suspended'),count:peers?count('SUSPENDED'):undefined},
        {id:'ALL',label:tr('ui.all'),count:peers?rows.length:undefined}]}/>
      <div className="afya-filter-bar"><SearchField label={tr('ui.searchPeerNavigators')} placeholder={tr('ui.searchByNameCountyOrganisationOr')} value={search} onChange={v=>{setSearch(v);setOffset(0);}}/></div>
      {loadError&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{tr('ui.peerNavigatorsCouldNotBeLoaded')} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>{tr('ui.retry')}</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>{tr('ui.applicant')}</th><th>{tr('ui.county2')}</th><th>{tr('ui.organisation')}</th><th>{tr('ui.applied')}</th><th>{tr('ui.status')}</th><th style={{textAlign:'right'}}>{tr('ui.actions')}</th></tr></thead>
          <tbody>{filtered.slice(offset,offset+PAGE).map(r=><tr key={r.id} className="admin-clickable-row" onClick={()=>{setOpenId(r.id);void recordAdminView('PEER_NAVIGATOR',r.id);}}>
            <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden>{initialsOf(r.full_name)}</span><span style={{fontWeight:700}}>{r.full_name}</span></div></td>
            <td>{r.county}</td><td>{r.organization||'—'}</td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(r.created_at)}</td>
            <td><Chip tone={STATUS[r.approval_status].tone}>{STATUS[r.approval_status].label}</Chip></td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-btn-secondary" onClick={e=>{e.stopPropagation();setOpenId(r.id);void recordAdminView('PEER_NAVIGATOR',r.id);}}><Eye size={15} aria-hidden/>{r.approval_status==='PENDING'?tr('ui2.review'):tr('ui2.view')}</button></td>
          </tr>)}</tbody>
        </table>
        {peers&&!filtered.length&&<EmptyState title={tab==='PENDING'?tr('ui2.noApplicationsAwaitingReview'):tr('ui2.noPeerNavigatorsHere')}>{tab==='PENDING'?tr('ui2.newPeerNavigatorSignUpsFrom'):tr('ui2.tryAnotherTabOrSearch')}</EmptyState>}
        {!peers&&!loadError&&<EmptyState title={tr('ui.loading')}/>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={filtered.length} onChange={setOffset}/>
    </section>
  </div>;
}

export function PeerDetail({peer,onBack,onDecided}:{peer:AdminPeer;onBack:()=>void;onDecided:()=>void}){
  const [note,setNote]=useState('');const [busy,setBusy]=useState(false);const [error,setError]=useState<string|null>(null);
  const actions=PEER_ACTIONS[peer.approval_status];
  async function decide(action:PeerDecision){
    if(peerDecisionNeedsNote(action)&&!note.trim()){setError(tr(`tpl.peerNote.${action}` as MessageKey));return;}
    setBusy(true);setError(null);
    try{await decideAdminPeer(peer.id,action,note);onDecided();}
    catch(e){setError(blogErrorMessage(e));}
    finally{setBusy(false);}
  }
  const icon:Record<PeerDecision,ReactNode>={approve:<UserCheck size={16} aria-hidden/>,reject:<XCircle size={16} aria-hidden/>,suspend:<Ban size={16} aria-hidden/>,reinstate:<RotateCcw size={16} aria-hidden/>};
  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>{tr('ui.backToPeerNavigators')}</button>
    <section className="admin-card afya-profile-head">
      <span className="afya-avatar lg" aria-hidden>{initialsOf(peer.full_name)}</span>
      <div style={{flex:1,minWidth:0}}>
        <div style={{display:'flex',alignItems:'center',gap:10,flexWrap:'wrap'}}><h2 style={{margin:0,fontSize:22,fontWeight:800}}>{peer.full_name}</h2><Chip tone={STATUS[peer.approval_status].tone}>{STATUS[peer.approval_status].label}</Chip></div>
        <div className="afya-profile-meta"><span>{tr('ui.peerNavigator')}</span><span><MapPin size={14} aria-hidden/>{peer.county}</span></div>
      </div>
    </section>
    <div className="afya-two-col" style={{gridTemplateColumns:'1fr 1fr'}}>
      <Card title={tr('ui.application')}><dl className="afya-dl">
        <Field label={tr('ui.fullName')}>{peer.full_name}</Field>
        <Field label={tr('ui.county2')}>{peer.county}</Field>
        <Field label={tr('ui.organisation')}>{peer.organization}</Field>
        <Field label={tr('ui.applied')}>{eatDate(peer.created_at)}</Field>
      </dl></Card>
      <Card title={tr('ui.contactReview')}><dl className="afya-dl">
        <Field label={tr('ui.email2')}>{peer.email}</Field>
        <Field label={tr('ui.phone2')}>{peer.phone_number}</Field>
        <Field label={tr('ui.lastReviewed')}>{peer.reviewed_at?eatDate(peer.reviewed_at):tr('ui2.notYetReviewed')}</Field>
        <Field label={tr('ui.reviewNote2')}>{peer.approval_note}</Field>
      </dl></Card>
    </div>
    <p className="afya-stat-note" style={{margin:0}}>{tr('ui.theApplicantAcceptedThePeerNavigator')}</p>
    {actions.length>0&&<Card title={tr('ui.decision')}>
      <div className="afya-form">
        <label htmlFor="peer-note" className="afya-field-label">{tr(actions.some(peerDecisionNeedsNote)?'mix.noteRequired':'mix.noteOptional')}</label>
        <textarea id="peer-note" rows={3} maxLength={NOTE_MAX} value={note} onChange={e=>setNote(e.target.value)} placeholder={tr('ui.shownToThePeerNavigatorIn')}/>
      </div>
      {error&&<div className="afya-login-error" role="alert" style={{marginTop:10}}>{error}</div>}
      <div style={{display:'flex',justifyContent:'flex-end',gap:10,marginTop:12}}>
        {actions.map(a=><button key={a} type="button" className={peerDecisionNeedsNote(a)?'afya-btn-danger':'afya-btn-primary'} disabled={busy} onClick={()=>decide(a)}>{icon[a]}{busy?tr('ui2.working'):ACTION_LABEL[a]}</button>)}
      </div>
    </Card>}
  </div>;
}
