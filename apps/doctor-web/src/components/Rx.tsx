'use client';
import { useEffect, useRef, useState } from 'react';
import { ApiError, errorText, runtimeGeneration, uploadPrescription } from '../lib/client';
import { compressRxPhoto, RX_PAGE_MAX_BYTES } from '../lib/rx-compression';
import { uploadPrescriptionPages } from '../lib/rx-upload';
import { Appointment, Profile, RecordDto, RxPage } from '../lib/contracts';
import { Avatar, Badge, PageHeader, State, demographics } from './Common';
import { Icon } from './Icons';
import { ClinicalAction, date, time } from './Queue';

export function Rx({profile,appointments,appointment,pages,setPages,select,action,complete,back}:{profile:Profile;appointments:Appointment[];appointment:Appointment|null;pages:RxPage[];setPages:(pages:RxPage[])=>void;select:(a:Appointment)=>void;action:ClinicalAction;complete:()=>void;back:()=>void}) {
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
  const stateTone:Record<RxPage['state'],'slate'|'blue'|'green'|'red'>={LOCAL:'slate',UPLOADING:'blue',UPLOADED:'green',FAILED:'red'};
  const stateLabel:Record<RxPage['state'],string>={LOCAL:'Ready to send',UPLOADING:'Uploading',UPLOADED:'Sent',FAILED:'Failed'};
  return <section id="webRxWorkspace" className="am-stack" style={{gap:22}}>
    <PageHeader title="Prescription (Rx)" subtitle="Photograph or scan your handwritten prescription. 1 to 5 pages, each compressed to 40 KB or less." back={{label:'Consultation room',onClick:back}}>
      <Badge tone="green">{pages.length}/5 pages</Badge>
    </PageHeader>
    <div className="am-grid main-side" style={{alignItems:'start'}}>
      <section className="am-card">
        <div className="am-card-head"><h2><Icon name="upload"/>Prescription pages</h2><button className="am-btn primary sm" aria-describedby="rxUploadHelp" disabled={disabled||!appointment||pages.length>=5} onClick={()=>picker.current?.click()}><Icon name="plus" size={15}/>Add photo</button></div>
        <div className="am-card-body am-stack">
          <input id="rxPhotoFileInput" type="file" ref={picker} hidden multiple accept="image/jpeg,image/png,image/webp" disabled={disabled||!appointment||pages.length>=5} onChange={e=>{void add(Array.from(e.target.files??[]));e.target.value='';}}/>
          {!pages.length&&<button type="button" className="am-dropzone" style={{cursor:appointment?'pointer':'not-allowed'}} disabled={disabled||!appointment} onClick={()=>picker.current?.click()}><Icon name="camera" size={34}/><strong>Choose prescription photos</strong><span id="rxUploadHelp">{!appointment?(appointments.length?'Select a consultation first.':'A prescription must belong to a consultation, and none are available.'):'JPEG, PNG or WebP, up to 10 MB each. They are compressed automatically before sending.'}</span></button>}
          {preparing&&<div role="status" className="am-stack" style={{gap:6}}><progress className="am-meter" aria-label="Compressing prescription photos"/><span className="am-faint">{preparing}</span></div>}
          {chosen&&<img src={chosen.preview} className="am-rx-preview" alt={`Handwritten prescription page ${focused+1}`}/>}
          {pages.length>0&&<div className="am-rx-grid">{pages.map((p,index)=><figure key={p.key} className="am-rx-page" style={{margin:0,borderColor:index===focused?'var(--am-green)':undefined}}>
            <button className="am-rx-thumb" aria-label={`Preview prescription page ${index+1}`} onClick={()=>setFocused(index)}><img src={p.preview} alt={`Prescription page ${index+1}`}/></button>
            <figcaption>
              <div className="am-row between"><strong className="am-strong">Page {index+1}</strong><Badge tone={stateTone[p.state]}>{p.state==='UPLOADED'?'Server confirmed':stateLabel[p.state]}</Badge></div>
              <span className="am-faint">{Math.ceil(p.file.size/1024)} KB of 40 KB{p.originalBytes!==undefined&&` · was ${Math.ceil(p.originalBytes/1024)} KB`}</span>
              {p.state==='UPLOADING'&&<><progress className="am-meter" max={100} value={p.uploadProgress==null?undefined:p.uploadProgress*100} aria-label={`Uploading prescription page ${index+1}`}/><span className="am-faint" role="status">{p.uploadProgress==null?'Preparing upload…':p.uploadProgress>=1?'Waiting for server confirmation…':`${Math.floor(p.uploadProgress*100)}% sent`}</span></>}
              {p.error&&<p role="alert" className="am-error-text">{p.error}</p>}
              <div className="am-row" style={{gap:6}}>{p.state==='FAILED'&&<button className="am-btn soft sm" disabled={disabled} onClick={()=>void upload(p.key)}>Retry</button>}{p.state!=='UPLOADED'&&<button className="am-btn danger sm" disabled={disabled} onClick={()=>remove(p)}><Icon name="trash" size={14}/>Remove</button>}</div>
            </figcaption>
          </figure>)}</div>}
          <State error={error}/>
        </div>
      </section>
      <div className="am-stack" style={{gap:18}}>
        <section className="am-card">
          <div className="am-card-head"><h2><Icon name="user"/>Consultation</h2></div>
          <div className="am-card-body am-stack">
            <label className="am-field"><span>Prescription for</span><select className="am-select" aria-label="Prescription consultation" value={appointment?.id??''} disabled={disabled} onChange={e=>{const a=appointments.find(row=>row.id===e.target.value);if(a)select(a);}}><option value="" disabled>Select a consultation</option>{appointments.map(a=><option key={a.id} value={a.id}>{a.patient_display_name} · {a.appointment_number}</option>)}</select></label>
            {appointment&&<div className="am-person"><Avatar name={appointment.patient_display_name}/><div><strong>{appointment.patient_display_name}</strong><span>{[demographics(appointment),`${date(appointment.start_time)}, ${time(appointment.start_time)}`].filter(Boolean).join(' · ')}</span></div></div>}
            <div className="am-row"><button className="am-btn ghost sm" disabled={!appointment} onClick={()=>appointment&&action('history',appointment)}><Icon name="history" size={15}/>History</button><button className="am-btn ghost sm" disabled={!appointment} onClick={()=>appointment&&action('documents',appointment)}><Icon name="file" size={15}/>Documents</button></div>
            <div className="am-callout" style={{padding:'10px 12px'}}><Icon name="stethoscope"/><span><strong>{profile.full_name}</strong><br/><span className="am-faint">{[profile.qualifications.join(', '),`KMPDC ${profile.license_number}`].filter(Boolean).join(' · ')}</span></span></div>
          </div>
        </section>
        <section className="am-card">
          <div className="am-card-head"><h2><Icon name="send"/>Send to patient</h2></div>
          <div className="am-card-body am-stack">
            <label className="am-field"><span>Advice for the patient (optional)</span><textarea className="am-textarea" id="docAdviceInput" rows={4} value={notes} maxLength={4000} disabled={disabled} placeholder="For example: take after meals, return if symptoms persist." onChange={e=>setNotes(e.target.value)}/></label>
            <button id="btnSendCapturedRx" className="am-btn primary block lg" disabled={disabled||!appointment||!pages.length||allUploaded} onClick={()=>void upload()}><Icon name="send" size={17}/>{busy?'Sending prescription…':preparing?'Preparing photos…':allUploaded?'All pages sent':'Send prescription'}</button>
            {allUploaded&&<div className="am-callout" role="status"><Icon name="checkCircle"/><span><strong>Prescription sent.</strong> Complete the consultation to record its outcome.</span></div>}
            <button className="am-btn ghost block" disabled={disabled||!appointment} onClick={complete}><Icon name="checkCircle" size={16}/>{allUploaded?'Complete consultation':'Complete without prescription / other outcome'}</button>
          </div>
        </section>
      </div>
    </div>
  </section>;
}
