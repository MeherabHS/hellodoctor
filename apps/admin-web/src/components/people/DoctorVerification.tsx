'use client';
import { tr } from '@/lib/i18n';
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
const STATUS:Record<Status,{tone:ChipTone;label:string}>={PENDING:{tone:'warn',get label(){return tr('ui.pendingReview')}},VERIFIED:{tone:'success',get label(){return tr('ui.verified')}},REJECTED:{tone:'danger',get label(){return tr('ui.rejected')}}};
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
    if(action==='reject'&&!window.confirm(tr('ui.rejectThisApplicationTheDoctorWill')))return;
    await onDecide(action);setOpenId(null);setRevision(v=>v+1);
  }

  if(open)return open.status==='PENDING'
    ?<ApplicationReview row={open} busy={busy} onBack={()=>setOpenId(null)} onDecide={decide}/>
    :<ClinicianDetail row={open} onBack={()=>setOpenId(null)}/>;

  return <div className="afya-stack">
    <PageHead title={tr('tpl.verificationQueue', { count: queue.length })} subtitle={tr('ui4.reviewKmpdcProfessionalRegistrationsBeforeDoctors')}/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="blue" value={doctors?rows.length:'—'} label={tr('ui.totalApplications')}/>
      <StatTile icon={Clock} tone="amber" value={queue.length} label={tr('ui.pendingReview')}/>
      <StatTile icon={BadgeCheck} tone="green" value={doctors?count('VERIFIED'):'—'} label={tr('ui.verified')}/>
      <StatTile icon={XCircle} tone="red" value={doctors?count('REJECTED'):'—'} label={tr('ui.rejected')}/>
    </div>
    <section className="admin-card">
      <Tabs label={tr('ui.verificationStatus')} value={tab} onChange={v=>{setTab(v);setOffset(0);}} items={[{id:'PENDING',label:tr('ui.pendingReview'),count:queue.length},{id:'VERIFIED',label:tr('ui.verified'),count:doctors?count('VERIFIED'):undefined},{id:'REJECTED',label:tr('ui.rejected'),count:doctors?count('REJECTED'):undefined},{id:'ALL',label:tr('ui.all'),count:doctors?rows.length:undefined}]}/>
      <div className="afya-filter-bar"><SearchField label={tr('ui.searchDoctors')} placeholder={tr('ui.searchByNameKmpdcNumberSpecialty')} value={search} onChange={v=>{setSearch(v);setOffset(0);}}/></div>
      {loadError&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{tr('ui.verifiedAndRejectedDoctorsCouldNot')} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>{tr('ui.retry')}</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>{tr('ui.applicant')}</th><th>{tr('ui.kmpdcReg')}</th><th>{tr('ui.specialty')}</th><th>{tr('ui.hospital')}</th><th>{tab==='PENDING'?tr('ui2.submitted'):tr('ui2.experience')}</th><th>{tr('ui.status')}</th><th style={{textAlign:'right'}}>{tr('ui.actions')}</th></tr></thead>
          <tbody>{filtered.slice(offset,offset+PAGE).map(r=><tr key={r.id} className="admin-clickable-row" onClick={()=>openRow(r)}>
            <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden>{initialsOf(r.name.replace(/^Dr\.?\s*/i,''))}</span><span style={{fontWeight:700}}>{r.name}</span></div></td>
            <td>{r.license}</td><td>{r.specialty}</td><td>{r.hospital}</td>
            <td style={{whiteSpace:'nowrap'}}>{tab==='PENDING'?eatDate(r.submitted):`${r.years} years`}</td>
            <td><Chip tone={STATUS[r.status].tone}>{STATUS[r.status].label}</Chip></td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-btn-secondary" onClick={e=>{e.stopPropagation();openRow(r);}}><Eye size={15} aria-hidden/>{r.status==='PENDING'?tr('ui2.review'):tr('ui2.view')}</button></td>
          </tr>)}</tbody>
        </table>
        {!filtered.length&&<EmptyState title={tab==='PENDING'?tr('ui2.noApplicationsAwaitingReview'):tr('ui2.noDoctorsHere')}>{tab==='PENDING'?tr('ui2.newDoctorRegistrationsAppearHereFor'):tr('ui2.tryAnotherTabOrSearch')}</EmptyState>}
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
    <Card title={tr('ui.professionalRegistration')}><dl className="afya-dl">
      <Field label={tr('ui.kmpdcRegistration')}>{row.license}</Field>
      <Field label={tr('ui.regulatoryAuthority')}>{row.authority}</Field>
      <Field label={tr('ui.country')}>{row.country}</Field>
      <Field label={tr('ui.qualifications')}>{row.qualifications.join(', ')||tr('ui2.notProvided')}</Field>
    </dl></Card>
    <Card title={tr('ui.specialtyPractice')}><dl className="afya-dl">
      <Field label={tr('ui.primarySpecialty')}>{row.specialty}</Field>
      <Field label={tr('ui.practiceFacility')}>{row.hospital}</Field>
      <Field label={tr('ui.yearsOfPractice')}>{`${row.years} years`}</Field>
      <Field label={tr('ui.verifiedPhone')}>{row.phone}</Field>
    </dl></Card>
  </div>;
}

function ApplicationReview({row,busy,onBack,onDecide}:{row:Row;busy:boolean;onBack:()=>void;onDecide:(a:'approve'|'reject')=>void}){
  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>{tr('ui.backToVerificationQueue')}</button>
    <ProfileHead row={row}/>
    <RegistrationCards row={row}/>
    <SubmittedDocuments doctorId={row.id}/>
    <p className="afya-stat-note" style={{margin:0}}>{tr('mix.submittedCheck', { date: eatDate(row.submitted) })}</p>
    <div style={{display:'flex',justifyContent:'flex-end',gap:10}}>
      <button type="button" className="afya-btn-danger" disabled={busy} onClick={()=>onDecide('reject')}><XCircle size={16} aria-hidden/>{tr('ui.reject')}</button>
      <button type="button" className="afya-btn-primary" disabled={busy} onClick={()=>onDecide('approve')}><UserCheck size={16} aria-hidden/>{busy?tr('ui2.working'):tr('ui2.approve')}</button>
    </div>
  </div>;
}

function ClinicianDetail({row,onBack}:{row:Row;onBack:()=>void}){
  const [tab,setTab]=useState<'overview'|'history'>('overview');
  const [history,setHistory]=useState<Encounter[]|null>(null);const [failed,setFailed]=useState(false);
  useEffect(()=>{let active=true;getDoctorDossier(row.id).then(d=>{if(active)setHistory(((d as unknown as {encounters?:Encounter[]}).encounters??[]).slice().sort((a,b)=>b.created_at.localeCompare(a.created_at)));}).catch(()=>{if(active)setFailed(true);});return()=>{active=false;};},[row.id]);
  const d=row.doctor;
  return <div className="afya-stack">
    <button type="button" className="afya-back-link" onClick={onBack}><ArrowLeft size={16} aria-hidden/>{tr('ui.backToDoctorVerification')}</button>
    <ProfileHead row={row}/>
    <section className="admin-card">
      <Tabs label={tr('ui.clinician')} value={tab} onChange={setTab} items={[{id:'overview',label:tr('ui.overview')},{id:'history',label:tr('ui.consultationHistory'),count:history?.length}]}/>
      {tab==='overview'&&<><div className="afya-stat-grid" style={{marginBottom:16}}>
          <StatTile icon={Users} tone="green" value={d?.consultations_count??'—'} label={tr('ui.completedConsultations')}/>
          <StatTile icon={UserCheck} tone={d?.is_on_duty?'green':'neutral'} value={d?.is_on_duty?tr('ui2.onDuty'):tr('ui2.offDuty')} label={tr('ui.availabilityNow')}/>
        </div><RegistrationCards row={row}/><div style={{marginTop:16}}><SubmittedDocuments doctorId={row.id}/></div></>}
      {tab==='history'&&(failed?<EmptyState title={tr('ui.consultationHistoryCouldNotBeLoaded')}/>:!history?<EmptyState title={tr('ui.loading')}/>:history.length?<ol className="afya-timeline">{history.map(e=><li key={e.appointment_id}>
        <div className="afya-timeline-head"><strong>{eatDate(e.created_at)} · {e.modality==='VIDEO'?tr('ui2.videoConsultation'):'24h chat consultation'}</strong><Chip tone={e.status==='COMPLETED'?'success':'neutral'}>{title(e.status)}</Chip></div>
        <div className="afya-stat-note">{e.patient_name}{e.appointment_number?` · ${e.appointment_number}`:''}{e.clinical_outcome?` · ${title(e.clinical_outcome)}`:''}</div>
      </li>)}</ol>:<EmptyState title={tr('ui.noConsultationsYet')}/>)}
    </section>
  </div>;
}

/** Supporting documents and resubmission notes; files open through the audited binary route. */
function SubmittedDocuments({doctorId}:{doctorId:string}){
  const [rows,setRows]=useState<VerificationSubmission[]|null>(null);const [failed,setFailed]=useState(false);
  useEffect(()=>{let active=true;getVerificationSubmissions(doctorId).then(r=>{if(active)setRows(r);}).catch(()=>{if(active)setFailed(true);});return()=>{active=false;};},[doctorId]);
  return <Card title={tr('ui.submittedDocuments')}>
    {failed?<EmptyState title={tr('ui.documentsCouldNotBeLoaded')}/>:!rows?<EmptyState title={tr('ui.loading')}/>:!rows.length?<EmptyState title={tr('ui.noDocumentsUploaded')}>{tr('ui.theDoctorHasNotUploadedSupporting')}</EmptyState>:
    <ol className="afya-timeline">{rows.map(s=><li key={s.id}>
      <div className="afya-timeline-head"><strong>{eatDate(s.created_at)}</strong><Chip tone={s.previous_status==='REJECTED'?'warn':'neutral'}>{s.previous_status==='REJECTED'?tr('ui2.resubmittedAfterRejection'):tr('ui2.addedWhilePending')}</Chip></div>
      {s.previous_note&&<div className="afya-stat-note">{tr('mix.earlierRejection')}{s.previous_note}</div>}
      {s.message&&<div className="afya-stat-note">Doctor&apos;s note: {s.message}</div>}
      <div style={{display:'flex',gap:8,flexWrap:'wrap',marginTop:8}}>{s.documents.map((d,i)=><a key={d.id} className="afya-btn-secondary" href={`/api/verification-documents?doctor_id=${encodeURIComponent(doctorId)}&document_id=${encodeURIComponent(d.id)}`} target="_blank" rel="noopener noreferrer"><FileText size={15} aria-hidden/>{d.mime_type==='application/pdf'?'PDF':tr('ui2.image')} {i+1} · {Math.ceil(d.size_bytes/1024)} KB</a>)}</div>
    </li>)}</ol>}
  </Card>;
}
