'use client';
import {useEffect,useState} from 'react';
import {CheckCircle2,Flag,MessageSquareText,Pill,X} from 'lucide-react';
import type {ComplianceAlert} from '../../lib/api';
import {listQnaAnswers,listQnaReviews,reviewQnaAnswer,type AuditStatus,type QnaAnswer,type QnaAnswerPage,type QnaReview} from '../../lib/reviewApi';
import {Card,Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,Tabs,eatDate,type ChipTone} from '../ui/afya';

/**
 * Operations & Safety → Rx & Q&A Audit (Operations board, panel 5).
 * Prescriptions: the backend's controlled-substance alerts. Q&A: doctors'
 * answers to patient questions; approve or flag with a note. A flagged answer
 * is hidden from patients until approved again. The patient who asked is
 * never shown.
 */
const PAGE=20;
const AUDIT_CHIP:Record<AuditStatus,{tone:ChipTone;label:string}>={UNREVIEWED:{tone:'warn',label:'Awaiting review'},APPROVED:{tone:'success',label:'Approved'},FLAGGED:{tone:'danger',label:'Flagged · hidden'}};
const clip=(s:string,n=110)=>s.length>n?`${s.slice(0,n)}…`:s;

export function AuditPage({alerts}:{alerts:ComplianceAlert[]}){
  const [tab,setTab]=useState<'rx'|'qna'>('qna');
  const [status,setStatus]=useState<AuditStatus|'ALL'>('UNREVIEWED');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);
  const [page,setPage]=useState<QnaAnswerPage|null>(null);const [error,setError]=useState('');const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);
  const [open,setOpen]=useState<QnaAnswer|null>(null);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    listQnaAnswers(new URLSearchParams({status,limit:String(PAGE),offset:String(offset),...(search.trim()?{search:search.trim()}:{})}))
      .then(p=>{if(active)setPage(p);}).catch(()=>{if(active)setError('Q&A answers could not be loaded.');}).finally(()=>{if(active)setLoading(false);});
  },200);return()=>{active=false;clearTimeout(timer);};},[status,search,offset,revision]);
  const counts=page?.counts;

  return <div className="afya-stack">
    <PageHead title="Rx & Q&A Audit" subtitle="Audit flagged prescriptions and review doctors' answers to patient questions."/>
    <div className="afya-stat-grid">
      <StatTile icon={Pill} tone="red" value={alerts.length} label="Flagged prescriptions"/>
      <StatTile icon={MessageSquareText} tone="amber" value={counts?.UNREVIEWED??'—'} label="Answers awaiting review"/>
      <StatTile icon={Flag} tone="red" value={counts?.FLAGGED??'—'} label="Flagged answers (hidden)"/>
      <StatTile icon={CheckCircle2} tone="green" value={counts?.APPROVED??'—'} label="Approved answers"/>
    </div>
    <Card>
      <Tabs label="Audit" value={tab} onChange={setTab} items={[{id:'qna',label:'Q&A answers',count:counts?.ALL},{id:'rx',label:'Prescriptions',count:alerts.length}]}/>
      {tab==='rx'?<>
        <div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>Rx #</th><th>Category</th><th>Doctor</th><th>Patient</th><th>Reason</th><th>Status</th><th>Date</th></tr></thead>
          <tbody>{alerts.map(a=><tr key={a.id}>
            <td><span className="afya-mono">{a.rx_number}</span></td><td>{a.category}</td><td style={{fontWeight:650}}>{a.doctor_name}</td><td>{a.patient_name}</td>
            <td style={{maxWidth:320}}><div>{a.flag_reason}</div>{a.doctor_notes&&<div className="afya-stat-note">Doctor: {clip(a.doctor_notes,90)}</div>}</td>
            <td><Chip tone="warn">{a.status.charAt(0)+a.status.slice(1).toLowerCase().replaceAll('_',' ')}</Chip></td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(a.created_at)}</td>
          </tr>)}</tbody>
        </table>{!alerts.length&&<EmptyState title="No prescriptions flagged">Prescriptions that match controlled-substance rules appear here for counter-signature review.</EmptyState>}</div>
      </>:<>
        <div className="afya-chip-filters" role="group" aria-label="Review status">
          {(['UNREVIEWED','FLAGGED','APPROVED','ALL'] as const).map(s=><button key={s} type="button" aria-pressed={status===s} className={`afya-chip-filter ${status===s?'on':''}`} onClick={()=>{setStatus(s);setOffset(0);}}>
            {s==='ALL'?'All':AUDIT_CHIP[s].label} {counts?.[s]??''}</button>)}
        </div>
        <div className="afya-filter-bar"><SearchField label="Search answers" placeholder="Search by question, answer or doctor…" value={search} onChange={v=>{setSearch(v);setOffset(0);}}/></div>
        {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
        <div className="afya-table-wrap"><table className="admin-table">
          <thead><tr><th>Question</th><th>Doctor</th><th>Answered</th><th>Review</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{(page?.items??[]).map(a=><tr key={a.answer_id} className="admin-clickable-row" onClick={()=>setOpen(a)}>
            <td style={{maxWidth:440}}><div style={{fontWeight:700}}>{clip(a.question_text)}</div><div className="afya-stat-note">{a.category?`${a.category} · `:''}{clip(a.answer_text,90)}</div></td>
            <td>{a.doctor_name??'Doctor'}<div className="afya-stat-note">{a.doctor_specialty}</div></td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(a.answered_at)}</td>
            <td><Chip tone={AUDIT_CHIP[a.audit_status].tone}>{AUDIT_CHIP[a.audit_status].label}</Chip>{a.last_review&&<div className="afya-stat-note">{eatDate(a.last_review.reviewed_at)}</div>}</td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-btn-secondary" onClick={e=>{e.stopPropagation();setOpen(a);}}>Review</button></td>
          </tr>)}</tbody>
        </table>{!loading&&!page?.items.length&&<EmptyState title={status==='UNREVIEWED'?'Nothing awaiting review':'No answers here'}>{status==='UNREVIEWED'?'New doctor answers appear here.':'Try another status or search.'}</EmptyState>}</div>
        <Pagination offset={offset} pageSize={PAGE} total={page?.total??0} busy={loading} onChange={setOffset}/>
      </>}
    </Card>
    {open&&<ReviewPanel answer={open} onClose={()=>setOpen(null)} onReviewed={()=>{setOpen(null);setRevision(v=>v+1);}}/>}
  </div>;
}

function ReviewPanel({answer,onClose,onReviewed}:{answer:QnaAnswer;onClose:()=>void;onReviewed:()=>void}){
  const [history,setHistory]=useState<QnaReview[]|null>(null);const [note,setNote]=useState('');const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  useEffect(()=>{let active=true;listQnaReviews(answer.answer_id).then(h=>{if(active)setHistory(h);}).catch(()=>{if(active)setHistory([]);});return()=>{active=false;};},[answer.answer_id]);
  async function decide(decision:'APPROVED'|'FLAGGED'){
    if(!note.trim()){setError('Add a note explaining the decision.');return;}
    setBusy(true);setError('');
    try{await reviewQnaAnswer(answer.answer_id,decision,note.trim());onReviewed();}catch{setError('The review could not be saved. Try again.');}finally{setBusy(false);}
  }
  const chip=AUDIT_CHIP[answer.audit_status];
  return <div className="admin-modal-backdrop" role="dialog" aria-modal="true" aria-label="Review answer" onClick={e=>{if(e.target===e.currentTarget&&!busy)onClose();}}>
    <div className="admin-modal-card" style={{maxWidth:720}}>
      <div className="afya-modal-head"><div><div className="afya-modal-title">Review answer</div><div className="afya-modal-sub">{answer.doctor_name??'Doctor'} · answered {eatDate(answer.answered_at)}</div></div>
        <button type="button" className="afya-icon-btn" aria-label="Close" onClick={onClose} disabled={busy}><X size={18}/></button></div>
      <div style={{padding:'18px 22px',overflowY:'auto',display:'flex',flexDirection:'column',gap:14}}>
        <div><span className="afya-field-label">Patient question{answer.category?` · ${answer.category}`:''}</span><p className="afya-quote">{answer.question_text}</p></div>
        <div><span className="afya-field-label">Doctor&apos;s answer</span><p className="afya-quote" style={{borderColor:'var(--afya-green-line)'}}>{answer.answer_text}</p></div>
        <div style={{display:'flex',alignItems:'center',gap:8}}><span className="afya-field-label" style={{margin:0}}>Current status</span><Chip tone={chip.tone}>{chip.label}</Chip></div>
        <div><span className="afya-field-label">Review history</span>
          {history===null?<p className="afya-stat-note">Loading…</p>:history.length?<ol className="afya-timeline" style={{marginTop:8}}>{history.map(r=><li key={r.id}><div className="afya-timeline-head"><strong>{eatDate(r.reviewed_at)}</strong><Chip tone={r.decision==='FLAGGED'?'danger':'success'}>{r.decision==='FLAGGED'?'Flagged':'Approved'}</Chip></div><div className="afya-stat-note">{r.note}</div></li>)}</ol>:<p className="afya-stat-note">Not reviewed yet.</p>}
        </div>
        <div className="afya-form"><label htmlFor="qna-note">Review note *</label><textarea id="qna-note" rows={3} maxLength={1000} value={note} onChange={e=>setNote(e.target.value)} placeholder="What did you check, and why approve or flag?"/></div>
        <p className="afya-stat-note" style={{margin:0}}>Flagging hides this answer from patients until an Admin approves it again. The doctor&apos;s text is never edited.</p>
        {error&&<div className="afya-login-error" role="alert">{error}</div>}
      </div>
      <div className="afya-modal-foot">
        <button type="button" className="afya-btn-secondary" onClick={onClose} disabled={busy}>Cancel</button>
        <button type="button" className="afya-btn-danger" disabled={busy} onClick={()=>void decide('FLAGGED')}><Flag size={16} aria-hidden/>Flag and hide</button>
        <button type="button" className="afya-btn-primary" disabled={busy} onClick={()=>void decide('APPROVED')}><CheckCircle2 size={16} aria-hidden/>{busy?'Saving…':'Approve'}</button>
      </div>
    </div>
  </div>;
}
