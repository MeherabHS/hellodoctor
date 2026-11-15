'use client';
import {useMemo,useState} from 'react';
import {Activity,Eye,MessageSquare,Users,Video} from 'lucide-react';
import type {PatientRecordItem} from '../../data/adminStore';
import {buildSearchIndex,searchIndex} from '../../lib/searchIndex';
import {Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,eatDate,type ChipTone} from '../ui/afya';

/**
 * People & Care → Patients Directory (docs/design/admin, People & Care board,
 * panel 1). Shows only identity, demographics and visit activity: patient
 * medical conditions are never shown anywhere in Admin (product owner,
 * 2026-11-14).
 */
const PAGE=20;
const DAY=86_400_000;
export const ACTIVE_WINDOW_DAYS=30;

export function ageFrom(dob:string|null|undefined,now=Date.now()):number|null{
  if(!dob)return null;const d=new Date(dob);if(!Number.isFinite(d.getTime()))return null;
  const n=new Date(now);let age=n.getUTCFullYear()-d.getUTCFullYear();
  if(n.getUTCMonth()<d.getUTCMonth()||(n.getUTCMonth()===d.getUTCMonth()&&n.getUTCDate()<d.getUTCDate()))age--;
  return age>=0&&age<130?age:null;
}
export const genderLabel=(g:string|null|undefined)=>g?g.charAt(0)+g.slice(1).toLowerCase():'—';
type Activity='active'|'inactive'|'none';
export function activityOf(p:PatientRecordItem,now=Date.now()):Activity{
  if(!p.lastConsultAt)return 'none';
  return now-new Date(p.lastConsultAt).getTime()<=ACTIVE_WINDOW_DAYS*DAY?'active':'inactive';
}
export const ACTIVITY_CHIP:Record<Activity,{tone:ChipTone;label:string}>={
  active:{tone:'success',label:'Active'},inactive:{tone:'neutral',label:'No recent visit'},none:{tone:'info',label:'No visits yet'},
};
const AGE_GROUPS=[{id:'u18',label:'Under 18',min:0,max:17},{id:'18-24',label:'18–24',min:18,max:24},{id:'25-34',label:'25–34',min:25,max:34},{id:'35-49',label:'35–49',min:35,max:49},{id:'50+',label:'50 and over',min:50,max:200}];

export function initialsOf(name:string){return name.split(/\s+/).filter(Boolean).map(w=>w[0]).join('').slice(0,2).toUpperCase()||'P';}

export function PatientsDirectory({patients,onOpen}:{patients:PatientRecordItem[];onOpen:(key:string)=>void}){
  const [search,setSearch]=useState('');const [gender,setGender]=useState('');const [ageGroup,setAgeGroup]=useState('');const [activity,setActivity]=useState<''|Activity>('');const [offset,setOffset]=useState(0);
  // Activity is judged against the time the directory was opened.
  const [now]=useState(()=>Date.now());
  const genders=useMemo(()=>[...new Set(patients.map(p=>p.gender).filter((g):g is string=>Boolean(g)))].sort(),[patients]);
  // Indexed once per list: name, patient ID (with or without "#PT-").
  const index=useMemo(()=>buildSearchIndex(patients,p=>[p.name,p.id,p.backendId]),[patients]);
  const rows=useMemo(()=>searchIndex(index,search).filter(p=>{
    if(gender&&p.gender!==gender)return false;
    if(ageGroup){const g=AGE_GROUPS.find(a=>a.id===ageGroup)!;const age=ageFrom(p.dateOfBirth,now);if(age===null||age<g.min||age>g.max)return false;}
    if(activity&&activityOf(p,now)!==activity)return false;
    return true;
  }).sort((a,b)=>(b.lastConsultAt??'').localeCompare(a.lastConsultAt??'')||a.name.localeCompare(b.name)),[index,search,gender,ageGroup,activity,now]);
  const page=rows.slice(offset,offset+PAGE);
  const active=patients.filter(p=>activityOf(p,now)==='active').length;
  const video=patients.reduce((s,p)=>s+(p.videoVisits??0),0);const chat=patients.reduce((s,p)=>s+(p.chatSessions??0),0);
  const reset=(f:()=>void)=>{f();setOffset(0);};

  return <div className="afya-stack">
    <PageHead title="Patients Directory" subtitle="Find patients and review their visit history and documents."/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="green" value={patients.length} label="Total patients"/>
      <StatTile icon={Activity} tone="green" value={active} label={`Active (last ${ACTIVE_WINDOW_DAYS} days)`}/>
      <StatTile icon={Video} tone="blue" value={video} label="Video consultations"/>
      <StatTile icon={MessageSquare} tone="purple" value={chat} label="Chat consultations"/>
    </div>
    <section className="admin-card">
      <div className="afya-filter-bar">
        <SearchField label="Search patients" placeholder="Search patients by name or patient ID…" maxLength={150} value={search} onChange={v=>reset(()=>setSearch(v))}/>
        <select className="afya-select" aria-label="Gender" value={gender} onChange={e=>reset(()=>setGender(e.target.value))}><option value="">All genders</option>{genders.map(g=><option key={g} value={g}>{genderLabel(g)}</option>)}</select>
        <select className="afya-select" aria-label="Age group" value={ageGroup} onChange={e=>reset(()=>setAgeGroup(e.target.value))}><option value="">All age groups</option>{AGE_GROUPS.map(a=><option key={a.id} value={a.id}>{a.label}</option>)}</select>
        <select className="afya-select" aria-label="Activity" value={activity} onChange={e=>reset(()=>setActivity(e.target.value as ''|Activity))}><option value="">All statuses</option>{(Object.keys(ACTIVITY_CHIP) as Activity[]).map(a=><option key={a} value={a}>{ACTIVITY_CHIP[a].label}</option>)}</select>
      </div>
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Patient</th><th>Patient ID</th><th>Age / Gender</th><th>Consultations</th><th>Last visit</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{page.map(p=>{const age=ageFrom(p.dateOfBirth,now);const chip=ACTIVITY_CHIP[activityOf(p,now)];return <tr key={p.key} className="admin-clickable-row" onClick={()=>onOpen(p.key)}>
            <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden>{initialsOf(p.name)}</span><span style={{fontWeight:700}}>{p.name}</span></div></td>
            <td><Chip tone="outlined">{p.id}</Chip></td>
            <td>{age??'—'} · {genderLabel(p.gender)}</td>
            <td>{p.consultationsCount??0}<div className="afya-stat-note">{p.videoVisits??0} video · {p.chatSessions??0} chat</div></td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(p.lastConsultAt)}</td>
            <td><Chip tone={chip.tone}>{chip.label}</Chip></td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-icon-btn" aria-label={`View record of ${p.name}`} onClick={e=>{e.stopPropagation();onOpen(p.key);}}><Eye size={17}/></button></td>
          </tr>;})}</tbody>
        </table>
        {!page.length&&<EmptyState title={patients.length?'No patients match these filters':'No patients yet'}>{patients.length?'Try another search or filter.':'Patients appear here once they register in the app.'}</EmptyState>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={rows.length} onChange={setOffset}/>
    </section>
  </div>;
}
