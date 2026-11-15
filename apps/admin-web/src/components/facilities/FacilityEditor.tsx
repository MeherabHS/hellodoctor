/* eslint-disable @next/next/no-img-element -- Photos come from the authenticated media route or a local data URL. */
'use client';
import {useEffect,useRef,useState} from 'react';
import {ArrowLeft,Building2,Check,Link2,MapPin,Phone,Trash2} from 'lucide-react';
import type {AdminFacility,FacilityContent,ServiceCategory} from '../../lib/facilityContent';
import {blankFacility,editableFacility,facilityErrors,publishProblems,statusLabel} from '../../lib/facilityContent';
import {createFacility,facilityImageUrl,getFacility,removeFacilityImage,transitionFacility,updateFacility,uploadFacilityImage} from '../../lib/facilityApi';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import {MAX_ADMIN_IMAGE_UPLOAD} from '../../lib/imageLimits';
import {blogErrorMessage} from '../../lib/blogErrors';
import {HoursEditor} from './HoursEditor';
import {Card,Chip} from '../ui/afya';
import {FACILITY_CHIP} from './facilityStatus';

export function FacilityEditor({initial,services,counties,onSaved,onClose}:{initial:AdminFacility|null;services:ServiceCategory[];counties:string[];onSaved:()=>void;onClose:()=>void}){
  const [facility,setFacility]=useState(initial);
  const [content,setContent]=useState<FacilityContent>(()=>initial?editableFacility(initial):blankFacility());
  const baseline=useRef(JSON.stringify(initial?editableFacility(initial):blankFacility()));
  const [dirty,setDirty]=useState(false);const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');
  const [photo,setPhoto]=useState<File|null>(null);const [localPhoto,setLocalPhoto]=useState('');
  const fileRef=useRef<HTMLInputElement>(null);
  const {requestNavigation}=useAdminNavigation();useUnsavedChanges(dirty||Boolean(photo),busy);
  useEffect(()=>{if(!photo)return;let active=true;const r=new FileReader();r.onload=()=>{if(active)setLocalPhoto(String(r.result));};r.readAsDataURL(photo);return()=>{active=false;r.abort();};},[photo]);

  function change<K extends keyof FacilityContent>(key:K,value:FacilityContent[K]){setContent(c=>{const next={...c,[key]:value};setDirty(JSON.stringify(next)!==baseline.current);return next;});setNotice('');}
  function adopt(saved:AdminFacility){setFacility(saved);setContent(editableFacility(saved));baseline.current=JSON.stringify(editableFacility(saved));setDirty(false);}
  const archived=facility?.publication_status==='ARCHIVED';const published=facility?.publication_status==='PUBLISHED';
  const errors=facilityErrors(content);
  const hasPhoto=Boolean(photo)||Boolean(facility?.has_image);
  const blockers=publishProblems(content,hasPhoto);

  async function save(){
    setBusy(true);setError('');setNotice('');
    try{
      if(errors.length)throw new Error(errors.join(' '));
      if(published&&blockers.length)throw new Error(`A published facility must stay complete: ${blockers.join(' ')}`);
      let saved=facility&&dirty?await updateFacility(facility.id,facility.version,content):facility??await createFacility(content);
      adopt(saved);
      if(photo){
        try{saved=await uploadFacilityImage(saved.id,saved.version,photo);adopt(saved);setPhoto(null);}
        catch(e){setError(`Details saved. ${blogErrorMessage(e)}`);onSaved();return;}
      }
      setNotice(published?'Published facility updated. Patients see the change on their next refresh.':'Draft saved. Drafts are not visible to patients.');onSaved();
    }catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  async function act(action:'publish'|'unpublish'|'archive'){
    if(!facility||dirty||photo)return;
    if(action==='archive'&&!window.confirm('Archive this facility? Archived facilities cannot be edited or published again.'))return;
    setBusy(true);setError('');setNotice('');
    try{const saved=await transitionFacility(facility.id,facility.version,action);adopt(saved);setNotice({publish:'Published. Patients can now see this facility.',unpublish:'Unpublished. Patients no longer see this facility.',archive:'Archived.'}[action]);onSaved();}
    catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  async function removePhoto(){
    if(!facility)return;setBusy(true);setError('');
    try{adopt(await removeFacilityImage(facility.id,facility.version));onSaved();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  async function reload(){if(!facility)return;setBusy(true);try{adopt(await getFacility(facility.id));setPhoto(null);setError('');}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  const preview=localPhoto&&photo?localPhoto:facility?.has_image?facilityImageUrl(facility.id,facility.image_version):'';
  const locked=busy||archived;
  const status=facility?.publication_status??'DRAFT';

  return <div className="afya-stack fac-editor">
    <div className="afya-page-head">
      <div style={{display:'flex',alignItems:'center',gap:12}}>
        <button type="button" className="afya-icon-btn" aria-label="Back to facilities" onClick={()=>requestNavigation(onClose)}><ArrowLeft size={20}/></button>
        <div><h2>{facility?facility.name:'Create Facility'}</h2>
          <p style={{display:'flex',gap:8,alignItems:'center'}}><Chip tone={FACILITY_CHIP[status]}>{statusLabel[status]}</Chip>{facility?`Version ${facility.version}`:'Not saved yet'}{dirty||photo?' · Unsaved changes':''}</p></div>
      </div>
    </div>
    {error&&<div role="alert" className="afya-login-error">{error} {facility&&<button type="button" className="afya-btn-secondary" onClick={reload}>Reload saved facility</button>}</div>}
    {notice&&<p role="status" className="afya-notice info" style={{margin:0}}>{notice}</p>}

    <div className="afya-two-col">
      <div className="afya-stack">
        <Card title="Facility details">
          <fieldset className="afya-form" disabled={locked} style={{border:0,padding:0,margin:0}}>
            <label htmlFor="fac-name">Facility name *</label>
            <input id="fac-name" value={content.name} maxLength={120} placeholder="e.g. Siaya County Hospital" onChange={e=>change('name',e.target.value)}/>
            <div className="fac-grid-2">
              <div className="afya-form"><label htmlFor="fac-county">County *</label>
                <select id="fac-county" value={content.county} onChange={e=>change('county',e.target.value)}><option value="">Choose county</option>{counties.map(c=><option key={c}>{c}</option>)}</select></div>
              <div className="afya-form"><label htmlFor="fac-phone">Contact number *</label>
                <div className="fac-icon-input"><Phone size={16} aria-hidden/><input id="fac-phone" type="tel" value={content.contact_phone} maxLength={24} placeholder="0712 345 678" onChange={e=>change('contact_phone',e.target.value)}/></div></div>
            </div>
            <label htmlFor="fac-location">Exact location (address) *</label>
            <div className="fac-icon-input"><MapPin size={16} aria-hidden/><input id="fac-location" value={content.location_text} maxLength={200} placeholder="Town, locality or street address" onChange={e=>change('location_text',e.target.value)}/></div>
            <label htmlFor="fac-maps">Google Maps direction link *</label>
            <div className="fac-icon-input"><Link2 size={16} aria-hidden/><input id="fac-maps" type="url" value={content.google_maps_url} maxLength={2048} placeholder="https://maps.app.goo.gl/…" onChange={e=>change('google_maps_url',e.target.value)}/></div>
            <span className="afya-stat-note">Open the facility in Google Maps, choose Share and paste the link. Patients see it under Get Directions, which opens it in Google Maps.</span>
            <label htmlFor="fac-desc">About this facility</label>
            <textarea id="fac-desc" value={content.description} maxLength={2000} rows={4} onChange={e=>change('description',e.target.value)}/>
            <label className="afya-switch-row"><span>Youth friendly</span><input type="checkbox" role="switch" checked={content.youth_friendly} onChange={e=>change('youth_friendly',e.target.checked)}/></label>
          </fieldset>
        </Card>
        <Card title="Opening hours" subtitle="Local Kenya time (Africa/Nairobi). Patients see open or closed now from these hours.">
          <fieldset disabled={locked} style={{border:0,padding:0,margin:0,minWidth:0}}><HoursEditor value={content.opening_hours} onChange={v=>change('opening_hours',v)}/></fieldset>
        </Card>
      </div>

      <div className="afya-stack">
        <Card title="Cover image *">
          <fieldset disabled={locked} style={{border:0,padding:0,margin:0}}>
            <div className="fac-photo">
              {preview?<img src={preview} alt={`Photo of ${content.name||'the facility'}`}/>:<div className="afya-tone-green" style={{display:'grid',placeItems:'center',height:'100%'}}><Building2 size={36} aria-hidden/></div>}
            </div>
            <div style={{display:'flex',gap:8,marginTop:10,flexWrap:'wrap'}}>
              <button type="button" className="afya-btn-secondary" onClick={()=>fileRef.current?.click()}>{hasPhoto?'Replace photo':'Upload photo'}</button>
              {photo&&<button type="button" className="afya-btn-secondary" onClick={()=>{setPhoto(null);setLocalPhoto('');}}>Discard selected photo</button>}
              {facility?.has_image&&!photo&&facility.publication_status==='DRAFT'&&<button type="button" className="afya-icon-btn" aria-label="Remove photo" onClick={()=>void removePhoto()}><Trash2 size={16} color="var(--afya-danger-fg)"/></button>}
            </div>
            <input ref={fileRef} type="file" accept="image/jpeg,image/png" hidden aria-label="Facility photo file" onChange={e=>{const file=e.target.files?.[0]||null;e.target.value='';if(file&&(!['image/jpeg','image/png'].includes(file.type)||file.size>MAX_ADMIN_IMAGE_UPLOAD)){setError('Choose a JPEG or PNG up to 25 MB.');return;}setPhoto(file);}}/>
            <p className="afya-stat-note" style={{margin:'8px 0 0'}}>JPEG or PNG, any size up to 25 MB. Required before publishing. Compressed automatically to 1 MB and stored privately.</p>
          </fieldset>
        </Card>
        <Card title="Service tags *" subtitle="Select every service offered here.">
          <fieldset disabled={locked} className="fac-service-tags" style={{border:0,padding:0,margin:0}}>
            {services.map(s=>{const on=content.services.includes(s.code);return <button key={s.code} type="button" aria-pressed={on} className={`fac-service-tag ${on?'on':''}`} onClick={()=>change('services',on?content.services.filter(c=>c!==s.code):[...content.services,s.code])}>{on&&<Check size={14} aria-hidden/>}{s.name_en}</button>;})}
          </fieldset>
        </Card>
        {!archived&&<Card title="Publication">
          {errors.length>0&&<ul className="afya-stat-note" style={{margin:'0 0 10px',paddingLeft:18}}>{errors.map(e=><li key={e}>{e}</li>)}</ul>}
          {facility?.publication_status==='DRAFT'&&blockers.length>0&&<p className="afya-stat-note" style={{margin:'0 0 10px'}}>Before publishing: {blockers.join(' ')}</p>}
          <div style={{display:'flex',gap:10}}>
            <button type="button" className="afya-btn-outline-green" style={{flex:1}} disabled={busy||errors.length>0||(!dirty&&!photo&&Boolean(facility))} onClick={()=>void save()}>{busy?'Saving…':published?'Update published facility':'Save draft'}</button>
            {facility?.publication_status==='DRAFT'&&<button type="button" className="afya-btn-primary" style={{flex:1}} disabled={busy||dirty||Boolean(photo)||blockers.length>0} title={blockers.join(' ')} onClick={()=>void act('publish')}>Publish</button>}
          </div>
          <div style={{display:'flex',gap:10,marginTop:10,flexWrap:'wrap'}}>
            {published&&<button type="button" className="afya-btn-secondary" disabled={busy||dirty||Boolean(photo)} onClick={()=>void act('unpublish')}>Unpublish</button>}
            {facility&&<button type="button" className="afya-btn-danger" disabled={busy||dirty||Boolean(photo)} onClick={()=>void act('archive')}>Archive</button>}
          </div>
          {(dirty||photo)&&facility&&<p className="afya-stat-note" style={{margin:'10px 0 0'}}>Save changes before publishing, unpublishing or archiving.</p>}
        </Card>}
      </div>
    </div>
  </div>;
}
