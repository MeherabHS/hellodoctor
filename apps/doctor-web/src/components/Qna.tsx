'use client';
import { useRef, useState } from 'react';
import { api, errorText, post, useResource } from '../lib/client';
import { Profile, Question } from '../lib/contracts';
import { Prototype, hasClass, id } from './Prototype';
import { State } from './Common';
import { date, initials } from './Queue';

const presets=[['💧 Hydration & ORS','Oral hydration with ORS + Paracetamol 500mg (max 4/day). Avoid antibiotics without blood test.'],['🩸 Fasting Lab','Fasting blood glucose & HbA1c test recommended at a verified lab. Cut refined sugars.'],['👶 Pediatric Care','Keep room humidified and provide warm fluids. If symptoms worsen, please visit hospital.']];
function QuestionCard({ question:q, profile, onAnswered, mobile }: {question:Question;profile:Profile;onAnswered:()=>void;mobile:boolean}) {
  const [answer,setAnswer]=useState('');const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [done,setDone]=useState(false);const lock=useRef(false);
  async function publish(){if(lock.current||!answer.trim())return;lock.current=true;setBusy(true);setError('');try{await post(`doctor/qna/questions/${q.id}/answers`,{answer_text:answer});setDone(true);onAnswered();}catch(e){setError(errorText(e));if(e instanceof Error&&'status'in e&&e.status===409)onAnswered();}finally{lock.current=false;setBusy(false);}}
  return <article className={`dw-section-card web-qna-card ${q.status==='ANSWERED'?'answered':''}`} style={mobile?{padding:'14px 15px',borderRadius:16}:undefined} data-question-id={q.id}>
    <div style={{display:'flex',justifyContent:'space-between',gap:8,flexWrap:'wrap'}}><span className="badge badge-blue">{q.category??'General Health'}</span><span className={`badge ${q.status==='ANSWERED'?'badge-green':'badge-amber'}`}>{q.status==='ANSWERED'?'✅ Resolved & published':'⏳ Awaiting clinical guidance'}</span><span style={{fontSize:11,color:'#64748B'}}>Anonymous · {date(q.created_at)}</span></div>
    <h3>{q.question_text}</h3>
    {q.status==='ANSWERED'?<div className="web-qna-answer" style={{background:'#ECFDF5',borderColor:'#A7F3D0'}}><strong>{q.answering_doctor}</strong><p className="web-clinical-text">{q.answer_text}</p></div>:<div className="web-qna-answer"><div style={{display:'flex',gap:8,alignItems:'center',fontSize:12,fontWeight:800}}><div className="dw-meet-pip-avatar">{initials(profile.full_name)}</div>{profile.full_name}<span style={{color:'#059669',fontSize:10.5}}>KMPDC {profile.license_number}</span></div><div className="web-qna-presets">{presets.map(([label,value])=><button key={label} className="dw-btn-mini" disabled={busy||done} onClick={()=>setAnswer(value)}>{label}</button>)}</div><label htmlFor={`answer-${q.id}`} style={{fontSize:11}}>Your clinical guidance</label><textarea id={`answer-${q.id}`} rows={3} maxLength={4000} value={answer} disabled={busy||done} onChange={e=>setAnswer(e.target.value)} placeholder="Write your clinical guidance…"/><div style={{display:'flex',justifyContent:'space-between',alignItems:'center',marginTop:10,gap:10,flexWrap:'wrap'}}><span style={{fontSize:11,color:'#64748B'}}>Published to the public community feed.</span><button className="dw-btn-primary" disabled={busy||done||!answer.trim()} onClick={()=>void publish()}>{busy?'Publishing…':done?'Answer published':'Publish Verified Doctor Answer →'}</button></div><State error={error}/></div>}
  </article>;
}
export function Qna({profile,mobile,onBack}:{profile:Profile;mobile:boolean;onBack:()=>void}) {
  const questions=useResource<Question[]>('doctor/qna/questions?status=ALL&limit=100&offset=0');
  const [filter,setFilter]=useState('all');const [search,setSearch]=useState('');const [paging,setPaging]=useState(false);const [pageError,setPageError]=useState('');const [hasMore,setHasMore]=useState(true);
  const all=questions.data??[],open=all.filter(q=>q.status==='OPEN'),answered=all.filter(q=>q.status==='ANSWERED');
  const filtered=all.filter(q=>(filter==='all'||q.status.toLowerCase()===(filter==='open'?'open':'answered'))&&`${q.question_text} ${q.category??''} ${q.answer_text??''}`.toLowerCase().includes(search.toLowerCase()));
  async function more(){if(paging)return;setPaging(true);try{const page=await api<Question[]>(`doctor/qna/questions?status=ALL&limit=100&offset=${all.length}`);questions.setData([...all,...page]);setHasMore(page.length===100);}catch(e){setPageError(errorText(e));}finally{setPaging(false);}}
  return <Prototype name={mobile?'doc-view-5':'dwPage_qna'} action={handler=>{if(handler.includes('switchDoctorTab'))onBack();else if(handler.includes('Search'))setSearch('');else if(handler.includes('filter'))setFilter(handler.match(/'([^']+)'/)?.[1]??'all');else void questions.refresh();}} override={n=>{
    if(id(n)==='doc-view-5'||id(n)==='dwPage_qna')return {props:{className:mobile?'screen-panel active':'dw-page-view active',style:{display:'flex',flex:1,flexDirection:'column'}}};
    if(hasClass(n,'flow-top-title-wrap'))return {children:<><h1>Open Health Q&amp;A Triage</h1><p>Answer anonymous community questions as {profile.full_name}</p></>};
    if(hasClass(n,'dw-page-title-group'))return {children:<><h2>Free Health Q&amp;A Clinical Hub</h2><p>Physician triage · Review anonymous inquiries & publish clinical guidance</p></>};
    if(hasClass(n,'badge-green')&&n.line<9900)return {children:profile.license_number.startsWith('KMPDC')?profile.license_number:`KMPDC ${profile.license_number}`};
    if(id(n)==='dwQnaSearchInput')return {props:{value:search,onChange:(e:React.ChangeEvent<HTMLInputElement>)=>setSearch(e.target.value)}};
    if(id(n)==='dwKpiOpenQnaCount')return {children:`${open.length} Pending`};
    if(id(n)==='dwKpiAnsweredQnaCount')return {children:`${answered.length} Resolved`};
    if(hasClass(n,'dw-kpi-val')&&n.line>9900)return {children:'Unavailable'};
    if(hasClass(n,'dw-kpi-sub')&&n.line>9920)return {children:'Public helpful votes are unavailable'};
    if(hasClass(n,'dw-kpi-lbl')&&n.line===9912)return {children:'Answered inquiries'};
    const counts:Record<string,number>={docMobileQnaTotalCount:all.length,docMobileQnaOpenCount:open.length,docMobileQnaAnsweredCount:answered.length,dwQnaPillAllCount:all.length,dwQnaPillOpenCount:open.length,dwQnaPillAnsweredCount:answered.length};
    if(id(n) in counts)return {children:counts[id(n)]};
    if(['docMobileQnaFilterAll','docMobileQnaFilterOpen','docMobileQnaFilterAnswered','dwQnaTabAll','dwQnaTabOpen','dwQnaTabAnswered'].includes(id(n)))return {props:{className:`${mobile?'doc-filter-chip':'dw-filter-pill'} ${id(n).toLowerCase().endsWith(filter==='all'?'all':filter==='open'?'open':'answered')?'active':''}`}};
    if(id(n)==='docMobileQnaContainer'||id(n)==='dwQnaListContainer')return {children:<><State loading={questions.loading} error={questions.error||pageError} empty={!questions.loading&&!questions.error&&!filtered.length?'No matching questions found':undefined} retry={()=>void questions.refresh()}/>{filtered.map(q=><QuestionCard key={q.id} question={q} profile={profile} mobile={mobile} onAnswered={()=>void questions.refresh()}/>)}{hasMore&&all.length>=100&&<button className="slot-btn sec" disabled={paging} onClick={()=>void more()}>{paging?'Loading…':'Load more questions'}</button>}</>};
    return undefined;
  }}/>;
}
