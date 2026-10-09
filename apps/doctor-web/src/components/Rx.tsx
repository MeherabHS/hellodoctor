'use client';
import { useEffect, useRef, useState } from 'react';
import { ApiError, errorText, runtimeGeneration, uploadPrescription } from '../lib/client';
import { compressRxPhoto, RX_PAGE_MAX_BYTES } from '../lib/rx-compression';
import { uploadPrescriptionPages } from '../lib/rx-upload';
import { Appointment, Profile, RecordDto, RxPage } from '../lib/contracts';
import { State } from './Common';
import { ClinicalAction, date } from './Queue';

export function Rx({profile,appointments,appointment,pages,setPages,select,action,complete}:{profile:Profile;appointments:Appointment[];appointment:Appointment|null;pages:RxPage[];setPages:(pages:RxPage[])=>void;select:(a:Appointment)=>void;action:ClinicalAction;complete:()=>void}) {
  const picker=useRef<HTMLInputElement>(null);const [busy,setBusy]=useState(false);const [preparing,setPreparing]=useState('');const [error,setError]=useState('');const [notes,setNotes]=useState('');const [focused,setFocused]=useState(0);const lock=useRef(false);const preparingLock=useRef(false);const pageRef=useRef(pages);pageRef.current=pages;
  const mounted=useRef(true);const contextRef=useRef(appointment?.id);contextRef.current=appointment?.id;
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useEffect(()=>{setNotes('');setError('');setFocused(0);},[appointment?.id]);
  async function add(files:File[]){
    if(preparingLock.current||lock.current||!files.length)return;
    if(!appointment){setError('Select a consultation first.');return;}
    if(files.length+pageRef.current.length>5){setError('A prescription supports at most five pages.');return;}
    const version=runtimeGeneration();const appointmentId=appointment.id;
    const isCurrent=()=>mounted.current&&version===runtimeGeneration()&&contextRef.current===appointmentId;
    preparingLock.current=true;setError('');
    try {
      for(let index=0;index<files.length;index++){
        if(!isCurrent())break;
        setPreparing(`Compressing photo ${index+1} of ${files.length} to 40 KB or less...`);
        try {
          const file=await compressRxPhoto(files[index],isCurrent);
          if(!isCurrent())break;
          const next:RxPage={key:crypto.randomUUID(),file,preview:URL.createObjectURL(file),state:'LOCAL',requestKey:crypto.randomUUID(),originalBytes:files[index].size};
          pageRef.current=[...pageRef.current,next];setPages(pageRef.current);
        } catch(e){if(isCurrent()&&!(e instanceof DOMException&&e.name==='AbortError'))setError(`Photo ${index+1}: ${e instanceof Error?e.message:'Unable to prepare this photo. Choose another image.'}`);}
      }
    } finally {preparingLock.current=false;if(mounted.current)setPreparing('');}
  }
  async function base64(file:File):Promise<string>{return new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(',')[1]);reader.onerror=()=>reject(new Error('Unable to read image'));reader.readAsDataURL(file);});}
  async function upload(only?:string){
    if(!appointment||lock.current||preparingLock.current)return;lock.current=true;setBusy(true);setError('');
    const version=runtimeGeneration();
    try {
      await uploadPrescriptionPages(pageRef.current,notes,only,async (page,progress)=>{if(page.file.size>RX_PAGE_MAX_BYTES)throw new ApiError(422,'PHOTO_TOO_LARGE','This photo exceeds 40 KB. Remove it and select it again to compress.');const image=await base64(page.file);if(version!==runtimeGeneration())throw new DOMException('Session changed','AbortError');return uploadPrescription<RecordDto>(appointment.id,{base64_photo:image,doctor_notes:page.uploadNotes,mime_type:page.file.type},page.requestKey,progress);},setPages,()=>version===runtimeGeneration(),errorText);
    } catch(e){if(!(e instanceof DOMException&&e.name==='AbortError'))setError(errorText(e));} finally {lock.current=false;setBusy(false);}
  }
  function remove(page:RxPage){if(busy||preparingLock.current||page.state==='UPLOADED')return;URL.revokeObjectURL(page.preview);setPages(pages.filter(p=>p.key!==page.key));setFocused(0);}
  const disabled=busy||!!preparing;
  const allUploaded=pages.length>0&&pages.every(p=>p.state==='UPLOADED');
  const chosen=pages[focused]??pages[0];
  return <><section className="dw-page-view active" id="webRxWorkspace">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>Smart Rx Scanner &amp; Dispatch</h2><p>Handwritten prescription photos &middot; 1-5 pages</p></div><span className="badge badge-green">{pages.length}/5 pages</span></header>
    <div className="web-workspace-grid">
      <div className="web-workspace-stack">
        <section className="dw-section-card"><div className="dw-section-header"><h3 className="dw-section-title">Consultation &amp; Patient Context</h3></div><div className="web-section-body">
          <label>Consultation<select aria-label="Prescription consultation" value={appointment?.id??''} disabled={disabled} onChange={e=>{const a=appointments.find(row=>row.id===e.target.value);if(a)select(a);}}><option value="" disabled>Select a consultation</option>{appointments.map(a=><option key={a.id} value={a.id}>{a.patient_display_name} &middot; {a.appointment_number}</option>)}</select></label>
          <div><strong>{appointment?.patient_display_name??'No consultation selected'}</strong><p className="sheet-desc">{appointment?`${date(appointment.start_time)} · ${appointment.status}`:'Choose an owned appointment to upload a prescription.'}</p></div>
          <div className="web-action-row"><button className="dw-btn-outline-sm" disabled={!appointment} onClick={()=>appointment&&action('history',appointment)}>Patient Case History</button><button className="dw-btn-outline-sm" disabled={!appointment} onClick={()=>appointment&&action('documents',appointment)}>Consultation Documents</button></div>
        </div></section>
        <section className="dw-section-card"><div className="dw-section-header"><h3 className="dw-section-title">Doctor Advice &amp; Dispatch</h3></div><div className="web-section-body">
          <label>Doctor quick advice to patient<textarea id="docAdviceInput" rows={5} value={notes} maxLength={4000} disabled={disabled} onChange={e=>setNotes(e.target.value)}/></label>
          <button id="btnSendCapturedRx" className="dw-btn-primary" disabled={disabled||!appointment||!pages.length||allUploaded} onClick={()=>void upload()}>{busy?'Uploading prescription pages...':preparing?'Preparing prescription photos...':allUploaded?'All pages uploaded':'Send Prescription ->'}</button>
          {pages.map((p,index)=>p.state==='UPLOADING'&&<div key={p.key} className="web-rx-upload-progress"><strong>Uploading page {index+1} of {pages.length}</strong><progress max={100} value={p.uploadProgress==null?undefined:p.uploadProgress*100} aria-label={`Uploading prescription page ${index+1}`}/><span role="status">{p.uploadProgress==null?'Preparing upload...':p.uploadProgress>=1?'100% transferred · Waiting for server confirmation...':`${Math.floor(p.uploadProgress*100)}% transferred`}</span></div>)}
          <State error={error}/>{allUploaded&&<div role="status"><strong>All selected pages uploaded to the server</strong><p className="sheet-desc">Complete the consultation separately to record the clinical outcome.</p><button className="dw-btn-primary" onClick={complete}>Complete consultation</button></div>}
          <button className="dw-btn-outline" disabled={disabled||!appointment} onClick={complete}>Complete without prescription / Other outcome</button>
        </div></section>
      </div>
      <section className="dw-section-card"><div className="dw-section-header"><h3 className="dw-section-title">Upload Handwritten Prescription</h3><span className="badge badge-green">Photo workflow</span></div><div className="web-section-body">
        <div className="web-action-row"><button className="dw-btn-primary" aria-describedby="rxUploadHelp" disabled={disabled||!appointment||pages.length>=5} onClick={()=>picker.current?.click()}>Upload Rx Photo</button></div>
        <p id="rxUploadHelp" className="sheet-desc">{!appointment?(appointments.length?'Select a consultation above to enable photo upload.':'No consultations are available for your account. A prescription must belong to a consultation.'):'Select 1–5 JPEG, PNG or WebP photos, up to 10 MB each. Photos are automatically compressed to at most 40 KB per page. Check the preview, then use Send Prescription.'}</p>
        <input id="rxPhotoFileInput" type="file" ref={picker} hidden multiple accept="image/jpeg,image/png,image/webp" disabled={disabled||!appointment||pages.length>=5} onChange={e=>{void add(Array.from(e.target.files??[]));e.target.value='';}}/>
        {preparing&&<div className="web-rx-preparing" role="status"><progress aria-label="Compressing prescription photos"/>{preparing}</div>}
        <div className="web-action-row"><strong>{profile.full_name}</strong><span className="sheet-desc">{profile.qualifications.join(', ')} &middot; {profile.license_number}</span></div>
        {chosen?<img src={chosen.preview} className="web-rx-page-preview" alt={`Handwritten prescription page ${focused+1}`}/>:<State empty="Choose a handwritten prescription photo to preview"/>}
        <div className="web-rx-preview-grid">{pages.map((p,index)=><figure key={p.key}><button className="web-name-button" aria-label={`Preview prescription page ${index+1}`} onClick={()=>setFocused(index)}><img src={p.preview} alt={`Prescription page ${index+1}`}/></button><figcaption>Page {index+1} &middot; {p.state}<p className="sheet-desc">{Math.ceil(p.file.size/1024)} KB / 40 KB{p.originalBytes!==undefined&&` · Original ${Math.ceil(p.originalBytes/1024)} KB`}</p>{p.error&&<p role="alert">{p.error}</p>}</figcaption>{p.state==='FAILED'&&<button className="dw-btn-outline-sm" disabled={disabled} onClick={()=>void upload(p.key)}>Retry page</button>}{p.state!=='UPLOADED'&&<button className="dw-btn-outline-sm" disabled={disabled} onClick={()=>remove(p)}>Remove</button>}{p.state==='UPLOADED'&&<span className="badge badge-green">Server confirmed</span>}</figure>)}</div>
      </div></section>
    </div>
  </section></>;

}
