'use client';
import {useEffect,useRef,useState} from 'react';
import type {AdminFacility,ServiceCategory} from '../../lib/facilityContent';
import {statusLabel} from '../../lib/facilityContent';
import {facilityOptions,getFacility,listFacilities} from '../../lib/facilityApi';
import {useAdminNavigation} from '../navigation/NavigationGuard';
import {blogErrorMessage} from '../../lib/blogErrors';
import {FacilityEditor} from './FacilityEditor';
import '../blogs/blogs.css';
import './facilities.css';

/** Content → Facilities & Services. Admins create, edit, publish/unpublish and
 * archive facilities; patients see published ones in the Patient app. */
export function FacilityManager(){
  const {requestNavigation}=useAdminNavigation();
  const [rows,setRows]=useState<AdminFacility[]>([]);const [total,setTotal]=useState(0);const [offset,setOffset]=useState(0);
  const [search,setSearch]=useState('');const [county,setCounty]=useState('');const [status,setStatus]=useState('');
  const [services,setServices]=useState<ServiceCategory[]>([]);const [counties,setCounties]=useState<string[]>([]);
  const [editor,setEditor]=useState<AdminFacility|null|undefined>(undefined);
  const [error,setError]=useState('');const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);const generation=useRef(0);
  useEffect(()=>{let active=true;facilityOptions().then(o=>{if(active){setServices(o.services);setCounties(o.counties);}}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[]);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    const q=new URLSearchParams({limit:'20',offset:String(offset),...(search.trim()?{search:search.trim()}:{}),...(county?{county}:{}),...(status?{status}:{})});
    listFacilities(q).then(r=>{if(active){setRows(r.items);setTotal(r.total);}}).catch(e=>{if(active)setError(blogErrorMessage(e));}).finally(()=>{if(active)setLoading(false);});
  },200);return()=>{active=false;clearTimeout(timer);};},[offset,search,county,status,revision]);
  async function open(id:string){const g=++generation.current;setLoading(true);try{const f=await getFacility(id);if(g===generation.current)setEditor(f);}catch(e){setError(blogErrorMessage(e));}finally{setLoading(false);}}
  const serviceNames=(codes:string[])=>codes.map(c=>services.find(s=>s.code===c)?.name_en??c).join(' · ')||'—';
  return <div className="cms-manager">
    {editor!==undefined?<FacilityEditor key={editor?.id||'new'} initial={editor} services={services} counties={counties} onSaved={()=>setRevision(v=>v+1)} onClose={()=>setEditor(undefined)}/>:<>
      <div className="cms-editor-heading"><div><h2>Facilities &amp; Services</h2><p>Health facilities patients can find, call and get directions to. Only published facilities appear in the Patient app.</p></div>
        <button className="cms-primary" type="button" disabled={!services.length} onClick={()=>requestNavigation(()=>setEditor(null))}>Create facility</button></div>
      <div className="cms-filters">
        <label>Search by name<input type="search" value={search} maxLength={120} onChange={e=>{setSearch(e.target.value);setOffset(0);}}/></label>
        <label>County<select value={county} onChange={e=>{setCounty(e.target.value);setOffset(0);}}><option value="">All counties</option>{counties.map(c=><option key={c}>{c}</option>)}</select></label>
        <label>Status<select value={status} onChange={e=>{setStatus(e.target.value);setOffset(0);}}><option value="">All statuses</option>{(['DRAFT','PUBLISHED','ARCHIVED'] as const).map(s=><option key={s} value={s}>{statusLabel[s]}</option>)}</select></label>
      </div>
      {error&&<p role="alert" className="cms-error">{error}<button type="button" onClick={()=>setRevision(v=>v+1)}>Retry</button></p>}
      {loading?<p role="status">Loading facilities…</p>:<div className="cms-table-wrap"><table><thead><tr>{['Facility','Location','Services','Status','Updated','Actions'].map(h=><th key={h}>{h}</th>)}</tr></thead>
        <tbody>{rows.map(f=><tr key={f.id}><td><strong>{f.name}</strong>{f.youth_friendly&&<small>Youth friendly</small>}{!f.has_image&&<small>No photo</small>}</td><td>{f.location_text||'—'}<small>{f.county||'No county'}</small></td><td>{serviceNames(f.services)}</td>
          <td><span className={`cms-status cms-status-${f.publication_status.toLowerCase()}`}>{statusLabel[f.publication_status]}</span>{f.publication_status==='DRAFT'&&f.publish_problems.length>0&&<small>Incomplete</small>}</td>
          <td>{new Date(f.updated_at).toLocaleDateString('en-KE',{timeZone:'Africa/Nairobi'})}</td>
          <td><button type="button" onClick={()=>requestNavigation(()=>open(f.id))}>{f.publication_status==='ARCHIVED'?'View':'Edit'}</button></td></tr>)}</tbody></table>
        {!rows.length&&<p>No facilities match these filters.</p>}</div>}
      <div className="cms-pagination"><button type="button" disabled={offset===0||loading} onClick={()=>setOffset(v=>Math.max(0,v-20))}>Previous</button><span>{total?offset+1:0}–{Math.min(offset+20,total)} of {total}</span><button type="button" disabled={offset+20>=total||loading} onClick={()=>setOffset(v=>v+20)}>Next</button></div>
    </>}
  </div>;
}
