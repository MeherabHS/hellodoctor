'use client';
import {useEffect,useMemo,useState,type ReactNode} from 'react';
import {ArrowLeft,BadgeCheck,Building2,Clock,Eye,FileText,MapPin,Stethoscope,UserCheck,Users,XCircle} from 'lucide-react';
import {getAdminDoctors,getDoctorDossier,getVerificationSubmissions,type AdminDoctor,type PendingDoctorVerification,type VerificationSubmission} from '../../lib/api';
import {buildSearchIndex,searchIndex} from '../../lib/searchIndex';
import {Card,Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,Tabs,eatDate,type ChipTone} from '../ui/afya';
import {initialsOf} from './PatientsDirectory';

/**
 * People & Care → Doctor Verification (People & Care board, panels 3–5):
 * queue, application review and clinician detail. Approval and rejection use
 * the existing handlers and endpoints. Doctors may upload supporting documents
 * and resubmit after a rejection (Doctor Web redesign, 2026-11-14); those are
 * shown read-only here. No Request Changes, messaging or photos exist.
 */
type Status='PENDING'|'VERIFIED'|'REJECTED';
type TabId=Status|'ALL';
const STATUS:Record<Status,{tone:ChipTone;label:string}>={PENDING:{tone:'warn',label:'Pending review'},VERIFIED:{tone:'success',label:'Verified'},REJECTED:{tone:'danger',label:'Rejected'}};
const PAGE=20;
interface Row{id:string;name:string;license:string;authority:string;country:string;specialty:string;hospital:string;years:number;phone:string;qualifications:string[];status:Status;submitted:string|null;doctor?:AdminDoctor}
interface Encounter{appointment_id:string;appointment_number:string;patient_name:string;modality:string;status:string;created_at:string;clinical_outcome:string|null}

const fromPending=(d:PendingDoctorVerification):Row=>({id:d.id,name:d.full_name,license:d.license_number,authority:d.license_authority,country:d.license_country,specialty:d.primary_specialty,hospital:d.current_hospital,years:d.experience_years,phone:d.verified_phone,qualifications:d.qualifications,status:'PENDING',submitted:d.created_at});
const fromDoctor=(d:AdminDoctor):Row=>({id:d.id,name:d.full_name,license:d.license_number,authority:d.license_authority,country:d.license_country,specialty:d.primary_specialty,hospital:d.current_hospital,years:d.experience_years,phone:d.verified_phone,qualifications:d.qualifications,status:d.professional_verification_status,submitted:null,doctor:d});
const title=(s:string)=>s?s.charAt(0)+s.slice(1).toLowerCase().replaceAll('_',' '):'—';

function Field({label,children}:{label:string;children:ReactNode}){return <div><dt>{label}</dt><dd>{children||'—'}</dd></div>;}

export function DoctorVerification({queue,busy,onSelect,onDecide}:{queue:PendingDoctorVerification[];busy:boolean;onSelect:(id:string)=>void;onDecide:(action:'approve'|'reject')=>Promise<void>}){
  const [doctors,setDoctors]=useState<AdminDoctor[]|null>(null);const [loadError,setLoadError]=useState(false);const [revision,setRevision]=useState(0);
  const [tab,setTab]=useState<TabId>('PENDING');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);
  const [openId,setOpenId]=useState<string|null>(null);
  useEffect(()=>{let active=true;getAdminDoctors().then(d=>{if(active){setDoctors(d);setLoadError(false);}}).catch(()=>{if(active)setLoadError(true);});return()=>{active=false;};},[revision,queue.length]);

  // Pending applications come from the live queue; decided ones from the doctors list.
  const rows=useMemo(()=>{const pendingIds=new Set(queue.map(q=>q.id));
    return [...queue.map(fromPending),...(doctors??[]).filter(d=>!pendingIds.has(d.id)&&d.professional_verification_status!=='PENDING').map(fromDoctor)];},[queue,doctors]);
  const count=(s:Status)=>rows.filter(r=>r.status===s).length;
  // Indexed once per load: name, KMPDC number, hospital and specialty.
  const index=useMemo(()=>buildSearchIndex(rows,r=>[r.name,r.license,r.hospital,r.specialty,r.phone]),[rows]);
  const filtered=searchIndex(index,search).filter(r=>tab==='ALL'||r.status===tab);
  const open=rows.find(r=>r.id===openId)??null;

  function openRow(r:Row){if(r.status==='PENDING')onSelect(r.id);setOpenId(r.id)}
  async function decide(action:'approve'|'reject'){
    if(action==='reject'&&!window.confirm('Reject this application? The doctor will not be able to consult until a new review.'))return;
    await onDecide(action);setOpenId(null);setRevision(v=>v+1);
  }

  if(open)return open.status==='PENDING'
    ?<ApplicationReview row={open} busy={busy} onBack={()=>setOpenId(null)} onDecide={decide}/>
    :<ClinicianDetail row={open} onBack={()=>setOpenId(null)}/>;

  return <div className="afya-stack">
    <PageHead title={`Doctor Verification Queue (${queue.length})`} subtitle="Review KMPDC professional registrations before doctors can consult."/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="blue" value={doctors?rows.length:'—'} label="Total applications"/>
      <StatTile icon={Clock} tone="amber" value={queue.length} label="Pending review"/>
      <StatTile icon={BadgeCheck} tone="green" value={doctors?count('VERIFIED'):'—'} label="Verified"/>
      <StatTile icon={XCircle} tone="red" value={doctors?count('REJECTED'):'—'} label="Rejected"/>
    </div>
    <section className="admin-card">
      <Tabs label="Verification status" value={tab} onChange={v=>{setTab(v);setOffset(0);}} items={[{id:'PENDING',label:'Pending review',count:queue.length},{id:'VERIFIED',label:'Verified',count:doctors?count('VERIFIED'):undefined},{id:'REJECTED',label:'Rejected',count:doctors?count('REJECTED'):undefined},{id:'ALL',label:'All',count:doctors?rows.length:undefined}]}/>
      <div className="afya-filter-bar"><SearchField label="Search doctors" placeholder="Search by name, KMPDC number, specialty or hospital…" value={search} onChange={v=>{setSearch(v);setOffset(0);}}/></div>
      {loadError&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>Verified and rejected doctors could not be loaded. <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Applicant</th><th>KMPDC Reg. #</th><th>Specialty</th><th>Hospital</th><th>{tab==='PENDING'?'Submitted':'Experience'}</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{filtered.slice(offset,offset+PAGE).map(r=><tr key={r.id} className="admin-clickable-row" onClick={()=>openRow(r)}>
            <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden>{initialsOf(r.name.replace(/^Dr\.?\s*/i,''))}</span><span style={{fontWeight:700}}>{r.name}</span></div></td>
            <td>{r.license}</td><td>{r.specialty}</td><td>{r.hospital}</td>
            <td style={{whiteSpace:'nowrap'}}>{tab==='PENDING'?eatDate(r.submitted):`${r.years} years`}</td>
            <td><Chip tone={STATUS[r.status].tone}>{STATUS[r.status].label}</Chip></td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-btn-secondary" onClick={e=>{e.stopPropagation();openRow(r);}}><Eye size={15} aria-hidden/>{r.status==='PENDING'?'Review':'View'}</button></td>
          </tr>)}</tbody>
        </table>
        {!filtered.length&&<EmptyState title={tab==='PENDING'?'No applications awaiting review':'No doctors here'}>{tab==='PENDING'?'New doctor registrations appear here for KMPDC review.':'Try another tab or search.'}</EmptyState>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={filtered.length} onChange={setOffset}/>
    </section>
  </div>;
}

function ProfileHead({row,actions}:{row:Row;actions?:ReactNode}){
  return <section className="admin-card afya-profile-head">
    <span className="afya-avatar lg" aria-hidden>{initialsOf(row.name.replace(/^Dr\.?\s*/i,''))}</span>
    <div style={{flex:1,minWidth:0}}>
      <div style={{display:'flex',alignItems:'center',gap:10,flexWrap:'wrap'}}><h2 style={{margin:0,fontSize:22,fontWeight:800}}>{row.name}</h2><Chip tone={STATUS[row.status].tone}>{STATUS[row.status].label}</Chip></div>
      <div className="afya-profile-meta"><span>{row.license}</span><span><Stethoscope size={14} aria-hidden/>{row.specialty}</span><span><Building2 size={14} aria-hidden/>{row.hospital}</span><span><MapPin size={14} aria-hidden/>{row.country}</span></div>
    </div>
    {actions}
  </section>;
}

function RegistrationCards({row}:{row:Row}){
  return <div className="afya-two-col" style={{gridTemplateColumns:'1fr 1fr'}}>
    <Card title="Professional registration"><dl className="afya-dl">
      <Field label="KMPDC registration #">{row.license}</Field>
      <Field label="Regulatory authority">{row.authority}</Field>
      <Field label="Country">{row.country}</Field>
      <Field label="Qualifications">{row.qualifications.join(', ')||'Not provided'}</Field>
    </dl></Card>
    <Card title="Specialty & practice"><dl className="afya-dl">
      <Field label="Primary specialty">{row.specialty}</Field>
      <Field label="Practice facility">{row.hospital}</Field>
      <Field label="Years of practice">{`${row.years} years`}</Field>
      <Field label="Verified phone">{row.phone}</Field>
    </dl></Card>
  </div>;
}

function ApplicationReview({row,busy,onBack,onDecide}:{row:Row;busy:boolean;onBack:()=>void;onDecide:(a:'approve'|'reject')=>void}){
  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>Back to Verification Queue</button>
    <ProfileHead row={row}/>
    <RegistrationCards row={row}/>
    <SubmittedDocuments doctorId={row.id}/>
    <p className="afya-stat-note" style={{margin:0}}>Submitted {eatDate(row.submitted)}. Check the registration number against the KMPDC register before approving.</p>
    <div style={{display:'flex',justifyContent:'flex-end',gap:10}}>
      <button type="button" className="afya-btn-danger" disabled={busy} onClick={()=>onDecide('reject')}><XCircle size={16} aria-hidden/>Reject</button>
      <button type="button" className="afya-btn-primary" disabled={busy} onClick={()=>onDecide('approve')}><UserCheck size={16} aria-hidden/>{busy?'Working…':'Approve'}</button>
    </div>
  </div>;
}

function ClinicianDetail({row,onBack}:{row:Row;onBack:()=>void}){
  const [tab,setTab]=useState<'overview'|'history'>('overview');
  const [history,setHistory]=useState<Encounter[]|null>(null);const [failed,setFailed]=useState(false);
  useEffect(()=>{let active=true;getDoctorDossier(row.id).then(d=>{if(active)setHistory(((d as unknown as {encounters?:Encounter[]}).encounters??[]).slice().sort((a,b)=>b.created_at.localeCompare(a.created_at)));}).catch(()=>{if(active)setFailed(true);});return()=>{active=false;};},[row.id]);
  const d=row.doctor;
  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>Back to Doctor Verification</button>
    <ProfileHead row={row}/>
    <section className="admin-card">
      <Tabs label="Clinician" value={tab} onChange={setTab} items={[{id:'overview',label:'Overview'},{id:'history',label:'Consultation History',count:history?.length}]}/>
      {tab==='overview'&&<><div className="afya-stat-grid" style={{marginBottom:16}}>
          <StatTile icon={Users} tone="green" value={d?.consultations_count??'—'} label="Completed consultations"/>
          <StatTile icon={UserCheck} tone={d?.is_on_duty?'green':'neutral'} value={d?.is_on_duty?'On duty':'Off duty'} label="Availability now"/>
        </div><RegistrationCards row={row}/><div style={{marginTop:16}}><SubmittedDocuments doctorId={row.id}/></div></>}
      {tab==='history'&&(failed?<EmptyState title="Consultation history could not be loaded"/>:!history?<EmptyState title="Loading…"/>:history.length?<ol className="afya-timeline">{history.map(e=><li key={e.appointment_id}>
        <div className="afya-timeline-head"><strong>{eatDate(e.created_at)} · {e.modality==='VIDEO'?'Video consultation':'24h chat consultation'}</strong><Chip tone={e.status==='COMPLETED'?'success':'neutral'}>{title(e.status)}</Chip></div>
        <div className="afya-stat-note">{e.patient_name}{e.appointment_number?` · ${e.appointment_number}`:''}{e.clinical_outcome?` · ${title(e.clinical_outcome)}`:''}</div>
      </li>)}</ol>:<EmptyState title="No consultations yet"/>)}
    </section>
  </div>;
}

/** Supporting documents and resubmission notes; files open through the audited binary route. */
function SubmittedDocuments({doctorId}:{doctorId:string}){
  const [rows,setRows]=useState<VerificationSubmission[]|null>(null);const [failed,setFailed]=useState(false);
  useEffect(()=>{let active=true;getVerificationSubmissions(doctorId).then(r=>{if(active)setRows(r);}).catch(()=>{if(active)setFailed(true);});return()=>{active=false;};},[doctorId]);
  return <Card title="Submitted documents">
    {failed?<EmptyState title="Documents could not be loaded"/>:!rows?<EmptyState title="Loading…"/>:!rows.length?<EmptyState title="No documents uploaded">The doctor has not uploaded supporting documents.</EmptyState>:
    <ol className="afya-timeline">{rows.map(s=><li key={s.id}>
      <div className="afya-timeline-head"><strong>{eatDate(s.created_at)}</strong><Chip tone={s.previous_status==='REJECTED'?'warn':'neutral'}>{s.previous_status==='REJECTED'?'Resubmitted after rejection':'Added while pending'}</Chip></div>
      {s.previous_note&&<div className="afya-stat-note">Earlier rejection reason: {s.previous_note}</div>}
      {s.message&&<div className="afya-stat-note">Doctor&apos;s note: {s.message}</div>}
      <div style={{display:'flex',gap:8,flexWrap:'wrap',marginTop:8}}>{s.documents.map((d,i)=><a key={d.id} className="afya-btn-secondary" href={`/api/verification-documents?doctor_id=${encodeURIComponent(doctorId)}&document_id=${encodeURIComponent(d.id)}`} target="_blank" rel="noopener noreferrer"><FileText size={15} aria-hidden/>{d.mime_type==='application/pdf'?'PDF':'Image'} {i+1} · {Math.ceil(d.size_bytes/1024)} KB</a>)}</div>
    </li>)}</ol>}
  </Card>;
}
