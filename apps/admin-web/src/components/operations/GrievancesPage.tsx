'use client';
import {useEffect,useMemo,useState} from 'react';
import {CheckCircle2,Gavel,Hourglass,MessageSquareWarning,RotateCcw,Scale,X} from 'lucide-react';
import type {GrievanceItem} from '../../data/adminStore';
import {decideAppeal,listAppeals,recordAppeal,type GrievanceAppeal} from '../../lib/reviewApi';
import {Card,Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,Tabs,eatDate,type ChipTone} from '../ui/afya';

/**
 * Operations & Safety → Consultation Grievances (Operations board, panel 4):
 * complaints, adjudication queue, resolved and appeals. Adjudication keeps
 * the existing dialog and endpoints. Appeals are recorded by an Admin on the
 * patient's behalf against a warned or dismissed decision (product owner,
 * 2026-11-14). Manual "Log Complaint" and assignment have no backend.
 */
type TabId='all'|'queue'|'resolved'|'appeals';
const PAGE=20;
const STATUS:Record<GrievanceItem['status'],{tone:ChipTone;label:string}>={
  PENDING_REVIEW:{tone:'warn',label:'Pending review'},UNDER_INVESTIGATION:{tone:'purple',label:'Under adjudication'},
  REFUND_APPROVED:{tone:'info',label:'Refund approved'},REFUNDED:{tone:'success',label:'Refunded'},WARNED:{tone:'danger',label:'Doctor warned'},DISMISSED:{tone:'neutral',label:'Dismissed'},
};
const APPEAL:Record<GrievanceAppeal['status'],{tone:ChipTone;label:string}>={OPEN:{tone:'warn',label:'Open'},UPHELD:{tone:'neutral',label:'Upheld'},OVERTURNED:{tone:'info',label:'Overturned'}};
const ACTION_LABEL:Record<string,string>={WARNING:'Doctor warned',DISMISS:'Dismissed',REFUND:'Refund approved'};
const pending=(g:GrievanceItem)=>g.status==='PENDING_REVIEW'||g.status==='UNDER_INVESTIGATION';
const appealable=(g:GrievanceItem)=>g.status==='WARNED'||g.status==='DISMISSED';

export function GrievancesPage({grievances,onOpen,onChanged}:{grievances:GrievanceItem[];onOpen:(id:string)=>void;onChanged:()=>Promise<void>|void}){
  const [tab,setTab]=useState<TabId>('all');const [search,setSearch]=useState('');const [target,setTarget]=useState('ALL');const [offset,setOffset]=useState(0);
  const [appeals,setAppeals]=useState<GrievanceAppeal[]|null>(null);const [appealError,setAppealError]=useState(false);const [revision,setRevision]=useState(0);
  const [recordFor,setRecordFor]=useState<GrievanceItem|null>(null);const [decideFor,setDecideFor]=useState<{appeal:GrievanceAppeal;decision:'UPHOLD'|'OVERTURN'}|null>(null);
  useEffect(()=>{let active=true;listAppeals().then(a=>{if(active){setAppeals(a);setAppealError(false);}}).catch(()=>{if(active)setAppealError(true);});return()=>{active=false;};},[revision]);
  const openAppealFor=useMemo(()=>new Set((appeals??[]).filter(a=>a.status==='OPEN').map(a=>a.grievance_id)),[appeals]);

  const rows=grievances.filter(g=>{
    if(tab==='queue'&&!pending(g))return false;
    if(tab==='resolved'&&pending(g))return false;
    if(target!=='ALL'&&g.target!==target)return false;
    const q=search.trim().toLowerCase();
    return !q||`${g.id} ${g.patient.name} ${g.doctor.name} ${g.doctor.registrationNumber} ${g.category} ${g.claimSummary}`.toLowerCase().includes(q);
  });
  const queue=grievances.filter(pending).length;const resolved=grievances.length-queue;
  const openAppeals=(appeals??[]).filter(a=>a.status==='OPEN').length;
  async function after(){setRecordFor(null);setDecideFor(null);setRevision(v=>v+1);await onChanged();}

  return <div className="afya-stack">
    <PageHead title="Consultation Grievances" subtitle="Manage patient complaints, adjudicate them, and record and decide appeals."/>
    <div className="afya-stat-grid">
      <StatTile icon={MessageSquareWarning} tone="blue" value={grievances.length} label="Total complaints"/>
      <StatTile icon={Hourglass} tone="amber" value={queue} label="Awaiting adjudication"/>
      <StatTile icon={CheckCircle2} tone="green" value={resolved} label="Resolved" note={grievances.length?`${Math.round(resolved/grievances.length*100)}% of all complaints`:undefined}/>
      <StatTile icon={Scale} tone="purple" value={appeals?openAppeals:'—'} label="Open appeals"/>
    </div>
    <Card>
      <Tabs label="Grievances" value={tab} onChange={v=>{setTab(v);setOffset(0);}} items={[{id:'all',label:'Complaints',count:grievances.length},{id:'queue',label:'Adjudication queue',count:queue},{id:'resolved',label:'Resolved',count:resolved},{id:'appeals',label:'Appeals',count:appeals?.length}]}/>
      {tab==='appeals'?<>
        {appealError&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>Appeals could not be loaded. <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
        <div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>Grievance</th><th>Appealed decision</th><th>Reason</th><th>Recorded</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{(appeals??[]).map(a=><tr key={a.id}>
            <td><div style={{fontWeight:700}}>{a.grievance_number}</div><div className="afya-stat-note">{a.category} · {a.target==='DOCTOR'?'Doctor':'System'}</div></td>
            <td>{ACTION_LABEL[a.appealed_action]??a.appealed_action}<div className="afya-stat-note">{eatDate(a.appealed_at)}</div></td>
            <td style={{maxWidth:340}}>{a.reason}{a.decision_note&&<div className="afya-stat-note">Decision: {a.decision_note}</div>}</td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(a.recorded_at)}</td>
            <td><Chip tone={APPEAL[a.status].tone}>{APPEAL[a.status].label}</Chip>{a.decided_at&&<div className="afya-stat-note">{eatDate(a.decided_at)}</div>}</td>
            <td style={{textAlign:'right',whiteSpace:'nowrap'}}>{a.status==='OPEN'&&<>
              <button type="button" className="afya-btn-secondary" onClick={()=>setDecideFor({appeal:a,decision:'UPHOLD'})}>Uphold</button>
              <button type="button" className="afya-btn-primary" style={{marginLeft:8}} onClick={()=>setDecideFor({appeal:a,decision:'OVERTURN'})}><RotateCcw size={15} aria-hidden/>Overturn</button>
            </>}</td>
          </tr>)}</tbody>
        </table>{appeals&&!appeals.length&&<EmptyState title="No appeals recorded">Record an appeal from a warned or dismissed complaint when the patient contests the decision.</EmptyState>}</div>
      </>:<>
        <div className="afya-filter-bar">
          <SearchField label="Search complaints" placeholder="Search by complaint ID, patient, doctor, KMPDC number or category…" maxLength={150} value={search} onChange={v=>{setSearch(v);setOffset(0);}}/>
          <select className="afya-select" aria-label="Target" value={target} onChange={e=>{setTarget(e.target.value);setOffset(0);}}><option value="ALL">All targets</option><option value="DOCTOR">Against a doctor</option><option value="SYSTEM">Against the system</option></select>
        </div>
        <div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>ID</th><th>Patient</th><th>Complaint</th><th>Doctor</th><th>Target</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{rows.slice(offset,offset+PAGE).map(g=>{const s=STATUS[g.status]??{tone:'neutral' as ChipTone,label:g.status};return <tr key={g.id} className="admin-clickable-row" onClick={()=>onOpen(g.id)}>
            <td><div style={{fontWeight:700}}>{g.id}</div><div className="afya-stat-note">{g.relativeTime}</div></td>
            <td style={{fontWeight:650}}>{g.patient.name}</td>
            <td style={{maxWidth:320}}><div style={{fontWeight:650}}>{g.category}</div><div className="afya-stat-note" style={{whiteSpace:'nowrap',overflow:'hidden',textOverflow:'ellipsis'}}>{g.claimSummary}</div></td>
            <td>{g.doctor.name}<div className="afya-stat-note">{g.doctor.registrationNumber}</div></td>
            <td><Chip tone={g.target==='DOCTOR'?'danger':'info'}>{g.target==='DOCTOR'?'Doctor':'System'}</Chip></td>
            <td><Chip tone={s.tone}>{s.label}</Chip>{openAppealFor.has(g.backendId??'')&&<div style={{marginTop:4}}><Chip tone="warn">Appeal open</Chip></div>}</td>
            <td style={{textAlign:'right',whiteSpace:'nowrap'}}>
              {appealable(g)&&g.backendId&&!openAppealFor.has(g.backendId)&&<button type="button" className="afya-btn-secondary" onClick={e=>{e.stopPropagation();setRecordFor(g);}}><Scale size={15} aria-hidden/>Record appeal</button>}
              <button type="button" className={pending(g)?'afya-btn-primary':'afya-btn-secondary'} style={{marginLeft:8}} onClick={e=>{e.stopPropagation();onOpen(g.id);}}>{pending(g)?<><Gavel size={15} aria-hidden/>Adjudicate</>:'View'}</button>
            </td>
          </tr>;})}</tbody>
        </table>{!rows.length&&<EmptyState title={grievances.length?'No complaints match these filters':'No complaints filed'}>{grievances.length?'Try another tab, target or search.':'Complaints patients file about a consultation appear here.'}</EmptyState>}</div>
        <Pagination offset={offset} pageSize={PAGE} total={rows.length} onChange={setOffset}/>
      </>}
    </Card>
    {recordFor&&<TextDialog title={`Record appeal · ${recordFor.id}`} sub={`Against the decision "${STATUS[recordFor.status]?.label}". Recorded on the patient's behalf.`} label="Patient's reason for appealing" confirm="Record appeal"
      onClose={()=>setRecordFor(null)} onSubmit={async text=>{await recordAppeal(recordFor.backendId!,text);await after();}}/>}
    {decideFor&&<TextDialog title={decideFor.decision==='OVERTURN'?`Overturn decision · ${decideFor.appeal.grievance_number}`:`Uphold decision · ${decideFor.appeal.grievance_number}`}
      sub={decideFor.decision==='OVERTURN'?'The decision is kept as history and the complaint returns to the adjudication queue to be decided again.':'The original decision stands.'}
      label="Decision note" confirm={decideFor.decision==='OVERTURN'?'Overturn':'Uphold'} danger={decideFor.decision==='OVERTURN'}
      onClose={()=>setDecideFor(null)} onSubmit={async text=>{await decideAppeal(decideFor.appeal.id,decideFor.decision,text);await after();}}/>}
  </div>;
}

function TextDialog({title,sub,label,confirm,danger,onClose,onSubmit}:{title:string;sub:string;label:string;confirm:string;danger?:boolean;onClose:()=>void;onSubmit:(text:string)=>Promise<void>}){
  const [text,setText]=useState('');const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  async function submit(){
    const value=text.trim();if(value.length<8){setError('Write at least 8 characters.');return;}
    setBusy(true);setError('');
    try{await onSubmit(value);}catch(e){const m=e instanceof Error?e.message:'';setError(m.includes('[409]')?'This changed in the meantime. Close and reload the list.':'This could not be saved. Try again.');setBusy(false);}
  }
  return <div className="admin-modal-backdrop" role="dialog" aria-modal="true" aria-label={title} onClick={e=>{if(e.target===e.currentTarget&&!busy)onClose();}}>
    <div className="admin-modal-card" style={{maxWidth:560}}>
      <div className="afya-modal-head"><div><div className="afya-modal-title">{title}</div><div className="afya-modal-sub">{sub}</div></div>
        <button type="button" className="afya-icon-btn" aria-label="Close" disabled={busy} onClick={onClose}><X size={18}/></button></div>
      <div className="afya-form" style={{padding:'16px 22px'}}>
        <label htmlFor="dialog-text">{label} *</label>
        <textarea id="dialog-text" rows={4} maxLength={2000} value={text} onChange={e=>setText(e.target.value)}/>
        <span className="afya-stat-note">8–2000 characters. Saved to the audit trail.</span>
        {error&&<div className="afya-login-error" role="alert">{error}</div>}
      </div>
      <div className="afya-modal-foot">
        <button type="button" className="afya-btn-secondary" disabled={busy} onClick={onClose}>Cancel</button>
        <button type="button" className={danger?'afya-btn-danger':'afya-btn-primary'} disabled={busy} onClick={()=>void submit()}>{busy?'Saving…':confirm}</button>
      </div>
    </div>
  </div>;
}
