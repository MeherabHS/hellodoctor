'use client';
import { tr } from '@/lib/i18n';
import {useEffect,useRef,useState} from 'react';
import {Archive,Building2,CheckCircle2,FileEdit,MapPin,Pencil,Plus} from 'lucide-react';
import type {AdminFacility,PublicationStatus,ServiceCategory} from '../../lib/facilityContent';
import {statusLabel} from '../../lib/facilityContent';
import {facilityImageUrl,facilityOptions,getFacility,listFacilities} from '../../lib/facilityApi';
import {useAdminNavigation} from '../navigation/NavigationGuard';
import {blogErrorMessage} from '../../lib/blogErrors';
import {FacilityEditor} from './FacilityEditor';
import {Chip,EmptyState,PageHead,Pagination,QueuedImage,SearchField,StatTile,Tabs,eatDate} from '../ui/afya';
import {FACILITY_CHIP} from './facilityStatus';
import './facilities.css';

const PAGE=20;
type StatusTab=''|PublicationStatus;
const TABS:{id:StatusTab;label:string}[]=[{id:'',get label(){return tr('ui.all')}},{id:'PUBLISHED',get label(){return tr('ui.published')}},{id:'DRAFT',get label(){return tr('ui.draft')}},{id:'ARCHIVED',get label(){return tr('ui.archived')}}];

function FacilityThumb({f}:{f:AdminFacility}){
  const box={width:64,height:44,borderRadius:8,flexShrink:0} as const;
  const tile=<div aria-hidden style={{...box,display:'grid',placeItems:'center'}} className="afya-tone-green"><Building2 size={20}/></div>;
  if(!f.has_image)return tile;
  return <QueuedImage src={facilityImageUrl(f.id,f.image_version)} alt="" style={{...box,objectFit:'cover'}} fallback={tile}/>;
}

/** Content → Facilities. Admins create, edit, publish/unpublish and archive
 * facilities; patients see published ones in the Patient app. */
export function FacilityManager(){
  const {requestNavigation}=useAdminNavigation();
  const [rows,setRows]=useState<AdminFacility[]>([]);const [total,setTotal]=useState(0);const [offset,setOffset]=useState(0);
  const [search,setSearch]=useState('');const [county,setCounty]=useState('');const [status,setStatus]=useState<StatusTab>('');
  const [counts,setCounts]=useState<Partial<Record<StatusTab,number>>>({});
  const [services,setServices]=useState<ServiceCategory[]>([]);const [counties,setCounties]=useState<string[]>([]);
  const [editor,setEditor]=useState<AdminFacility|null|undefined>(undefined);
  const [error,setError]=useState('');const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);const generation=useRef(0);
  useEffect(()=>{let active=true;facilityOptions().then(o=>{if(active){setServices(o.services);setCounties(o.counties);}}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[]);
  // Counts one status at a time to stay inside the per-user concurrency limit.
  useEffect(()=>{let active=true;
    (async()=>{for(const t of TABS){
      try{const r=await listFacilities(new URLSearchParams({limit:'1',offset:'0',...(t.id?{status:t.id}:{})}));if(!active)return;setCounts(prev=>({...prev,[t.id]:r.total}));}
      catch{if(!active)return;}
    }})();
    return()=>{active=false;};},[revision]);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    const q=new URLSearchParams({limit:String(PAGE),offset:String(offset),...(search.trim()?{search:search.trim()}:{}),...(county?{county}:{}),...(status?{status}:{})});
    listFacilities(q).then(r=>{if(active){setRows(r.items);setTotal(r.total);}}).catch(e=>{if(active)setError(blogErrorMessage(e));}).finally(()=>{if(active)setLoading(false);});
  },200);return()=>{active=false;clearTimeout(timer);};},[offset,search,county,status,revision]);
  async function open(id:string){const g=++generation.current;setLoading(true);try{const f=await getFacility(id);if(g===generation.current)setEditor(f);}catch(e){setError(blogErrorMessage(e));}finally{setLoading(false);}}
  const serviceName=(code:string)=>services.find(s=>s.code===code)?.name_en??code;

  if(editor!==undefined)return <div><FacilityEditor key={editor?.id||'new'} initial={editor} services={services} counties={counties} onSaved={()=>setRevision(v=>v+1)} onClose={()=>setEditor(undefined)}/></div>;

  return <div className="afya-stack">
    <PageHead title={tr('ui.facilities')} subtitle={tr('ui4.manageHealthFacilitiesTheirServicesOpening')}
      actions={<button type="button" className="afya-btn-primary" disabled={!services.length} onClick={()=>requestNavigation(()=>setEditor(null))}><Plus size={16} aria-hidden/>{tr('ui.newFacility')}</button>}/>
    <div className="afya-stat-grid">
      <StatTile icon={MapPin} tone="green" value={counts['']??'—'} label={tr('ui.totalFacilities')}/>
      <StatTile icon={CheckCircle2} tone="green" value={counts.PUBLISHED??'—'} label={tr('ui.published')}/>
      <StatTile icon={FileEdit} tone="amber" value={counts.DRAFT??'—'} label={tr('ui.drafts')}/>
      <StatTile icon={Archive} tone="neutral" value={counts.ARCHIVED??'—'} label={tr('ui.archived')}/>
    </div>
    <section className="admin-card">
      <Tabs label={tr('ui.facilityStatus')} value={status} onChange={v=>{setStatus(v);setOffset(0);}} items={TABS.map(t=>({...t,count:counts[t.id]}))}/>
      <div className="afya-filter-bar">
        <SearchField label={tr('ui.searchFacilities')} placeholder={tr('ui.searchFacilitiesByName')} maxLength={120} value={search} onChange={v=>{setSearch(v);setOffset(0);}}/>
        <select className="afya-select" aria-label={tr('ui.county2')} value={county} onChange={e=>{setCounty(e.target.value);setOffset(0);}}><option value="">{tr('ui.allCounties')}</option>{counties.map(c=><option key={c}>{c}</option>)}</select>
      </div>
      {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>{tr('ui.retry')}</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>{tr('ui.facility')}</th><th>{tr('ui.location')}</th><th>{tr('ui.services')}</th><th>{tr('ui.status')}</th><th>{tr('ui.updated')}</th><th style={{textAlign:'right'}}>{tr('ui.actions')}</th></tr></thead>
          <tbody>{loading&&!rows.length?<tr><td colSpan={6}><EmptyState title={tr('ui.loadingFacilities')}/></td></tr>:rows.map(f=><tr key={f.id} className="admin-clickable-row" onClick={()=>requestNavigation(()=>open(f.id))}>
            <td><div style={{display:'flex',alignItems:'center',gap:12}}><FacilityThumb f={f}/><div>
              <div style={{fontWeight:700}}>{f.name}</div>
              <div style={{display:'flex',gap:6,marginTop:4,flexWrap:'wrap'}}>{f.youth_friendly&&<Chip tone="success">{tr('ui.youthFriendly2')}</Chip>}{!f.has_image&&<Chip tone="neutral">{tr('ui.noPhoto')}</Chip>}</div></div></div></td>
            <td><div>{f.location_text||'—'}</div><div className="afya-stat-note">{f.county||tr('ui2.noCounty')}</div></td>
            <td><div style={{display:'flex',gap:6,flexWrap:'wrap',maxWidth:280}}>{f.services.slice(0,3).map(code=><Chip key={code} tone="topic">{serviceName(code)}</Chip>)}{f.services.length>3&&<Chip tone="outlined">+{f.services.length-3}</Chip>}{!f.services.length&&<span className="afya-stat-note">—</span>}</div></td>
            <td><Chip tone={FACILITY_CHIP[f.publication_status]}>{statusLabel[f.publication_status]}</Chip>{f.publication_status==='DRAFT'&&f.publish_problems.length>0&&<div className="afya-stat-note">{tr('ui.incomplete')}</div>}</td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(f.updated_at)}</td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-icon-btn" aria-label={tr(f.publication_status==='ARCHIVED'?'tpl.viewNamed':'tpl.editNamed', { name: f.name })} onClick={e=>{e.stopPropagation();requestNavigation(()=>open(f.id));}}><Pencil size={16}/></button></td>
          </tr>)}</tbody>
        </table>
        {!loading&&!rows.length&&<EmptyState title={tr('ui.noFacilitiesMatchTheseFilters')}>{tr('ui.tryAnotherStatusCountyOrSearch')}</EmptyState>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={total} busy={loading} onChange={setOffset}/>
    </section>
  </div>;
}
