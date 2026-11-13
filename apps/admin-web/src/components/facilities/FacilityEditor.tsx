/* eslint-disable @next/next/no-img-element -- Photos come from the authenticated media route or a local data URL. */
'use client';
import {useEffect,useRef,useState} from 'react';
import type {AdminFacility,FacilityContent,ServiceCategory} from '../../lib/facilityContent';
import {blankFacility,editableFacility,facilityErrors,publishProblems,statusLabel} from '../../lib/facilityContent';
import {createFacility,facilityImageUrl,getFacility,removeFacilityImage,transitionFacility,updateFacility,uploadFacilityImage} from '../../lib/facilityApi';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import {blogErrorMessage} from '../../lib/blogErrors';
import {HoursEditor} from './HoursEditor';

export function FacilityEditor({initial,services,counties,onSaved,onClose}:{initial:AdminFacility|null;services:ServiceCategory[];counties:string[];onSaved:()=>void;onClose:()=>void}){
  const [facility,setFacility]=useState(initial);
  const [content,setContent]=useState<FacilityContent>(()=>initial?editableFacility(initial):blankFacility());
  const baseline=useRef(JSON.stringify(initial?editableFacility(initial):blankFacility()));
  const [dirty,setDirty]=useState(false);const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');
  const [photo,setPhoto]=useState<File|null>(null);const [localPhoto,setLocalPhoto]=useState('');
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

  return <div className="cms-editor fac-editor">
    <div className="cms-editor-heading"><div><h2>{facility?facility.name:'New facility'}</h2><p>{facility?statusLabel[facility.publication_status]:'Draft'}{facility?` · version ${facility.version}`:''}</p></div><button type="button" onClick={()=>requestNavigation(onClose)}>Back to facilities</button></div>
    {error&&<div role="alert" className="cms-error">{error}{facility&&<button type="button" onClick={reload}>Reload saved facility</button>}</div>}
    {notice&&<p role="status" className="cms-notice">{notice}</p>}
    <fieldset className="cms-form" disabled={busy||archived}>
      <legend>Facility information</legend>
      <label>Facility name *<input value={content.name} maxLength={120} onChange={e=>change('name',e.target.value)}/></label>
      <label>County *<select value={content.county} onChange={e=>change('county',e.target.value)}><option value="">Choose county</option>{counties.map(c=><option key={c}>{c}</option>)}</select></label>
      <label>Location / address *<input value={content.location_text} maxLength={200} placeholder="Town, locality or street address" onChange={e=>change('location_text',e.target.value)}/></label>
      <label>Short description<textarea value={content.description} maxLength={2000} rows={4} onChange={e=>change('description',e.target.value)}/></label>
      <label className="cms-check"><input type="checkbox" checked={content.youth_friendly} onChange={e=>change('youth_friendly',e.target.checked)}/>Youth friendly</label>
    </fieldset>
    <fieldset className="cms-form" disabled={busy||archived}>
      <legend>Services available *</legend>
      <div className="fac-services">{services.map(s=><label key={s.code} className="cms-check"><input type="checkbox" checked={content.services.includes(s.code)} onChange={e=>change('services',e.target.checked?[...content.services,s.code]:content.services.filter(c=>c!==s.code))}/>{s.name_en}</label>)}</div>
    </fieldset>
    <fieldset className="cms-form" disabled={busy||archived}>
      <legend>Contact and directions</legend>
      <label>Contact phone *<input type="tel" value={content.contact_phone} maxLength={24} placeholder="0712 345 678" onChange={e=>change('contact_phone',e.target.value)}/></label>
      <label>Location URL (Google Maps) *<input type="url" value={content.google_maps_url} maxLength={2048} placeholder="https://maps.app.goo.gl/…" onChange={e=>change('google_maps_url',e.target.value)}/></label>
      <p className="cms-hint">Open the facility in Google Maps, choose Share and paste the link. Only google.com/maps, maps.google.com and maps.app.goo.gl links are accepted. In the Patient app this link is shown under the Get Directions button, which opens it in Google Maps.</p>
    </fieldset>
    <fieldset className="cms-form" disabled={busy||archived}>
      <legend>Opening hours</legend>
      <HoursEditor value={content.opening_hours} onChange={v=>change('opening_hours',v)}/>
    </fieldset>
    <fieldset className="cms-form" disabled={busy||archived}>
      <legend>Facility photo *</legend>
      {preview?<img className="fac-photo-preview" src={preview} alt={`Photo of ${content.name||'the facility'}`}/>:<p className="cms-hint">No photo yet. A photo is required before publishing.</p>}
      <label>{hasPhoto?'Replace photo':'Upload photo'} (JPEG/PNG, up to 5 MB)<input type="file" accept="image/jpeg,image/png" onChange={e=>{const file=e.target.files?.[0]||null;if(file&&(!['image/jpeg','image/png'].includes(file.type)||file.size>5*1024*1024)){setError('Choose a JPEG or PNG up to 5 MB.');return;}setPhoto(file);}}/></label>
      {photo&&<button type="button" onClick={()=>{setPhoto(null);setLocalPhoto('');}}>Discard selected photo</button>}
      {facility?.has_image&&!photo&&facility.publication_status==='DRAFT'&&<button type="button" onClick={()=>void removePhoto()}>Remove photo</button>}
      <p className="cms-hint">Photos are resized and stored privately; patients load them through the app.</p>
    </fieldset>
    {!archived&&<div className="cms-actions">
      <button type="button" className="cms-primary" disabled={busy||errors.length>0||(!dirty&&!photo&&Boolean(facility))} onClick={()=>void save()}>{busy?'Saving…':published?'Update published facility':'Save draft'}</button>
      {facility?.publication_status==='DRAFT'&&<button type="button" disabled={busy||dirty||Boolean(photo)||blockers.length>0} title={blockers.join(' ')} onClick={()=>void act('publish')}>Publish</button>}
      {published&&<button type="button" disabled={busy||dirty||Boolean(photo)} onClick={()=>void act('unpublish')}>Unpublish</button>}
      {facility&&<button type="button" disabled={busy||dirty||Boolean(photo)} onClick={()=>void act('archive')}>Archive</button>}
    </div>}
    {errors.length>0&&<ul className="cms-hint">{errors.map(e=><li key={e}>{e}</li>)}</ul>}
    {facility?.publication_status==='DRAFT'&&blockers.length>0&&<p className="cms-hint">Before publishing: {blockers.join(' ')}</p>}
    {(dirty||photo)&&facility&&<p className="cms-hint">Save changes before publishing, unpublishing or archiving.</p>}
  </div>;
}
