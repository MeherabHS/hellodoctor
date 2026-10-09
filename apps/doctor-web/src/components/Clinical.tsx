'use client';
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, ApiError, authorizedDocument, errorText, post, runtimeGeneration, useResource } from '../lib/client';
import { Appointment, RecordDto, money } from '../lib/contracts';
import { Prototype, hasClass, id } from './Prototype';
import { Modal, State } from './Common';
import { date, initials, time } from './Queue';

export function Summary({appointment:a,history,documents,complete}:{appointment:Appointment;history:()=>void;documents:()=>void;complete:()=>void}) {
  const summary=useResource<RecordDto>(`doctor/consultations/${a.id}/summary`);
  return <><div className="doc-slot-card" style={{margin:0}}><div className="doc-slot-header"><strong>{a.patient_display_name}</strong><span className="badge badge-blue">{a.doctor_lifecycle}</span></div><p>{date(a.start_time)} · {time(a.start_time)} – {time(a.end_time)}</p><p className="web-gap">{a.appointment_number}</p></div><State loading={summary.loading} error={summary.error} retry={()=>void summary.refresh()}/>{summary.data&&<><div className="queue-complaint"><strong>Chief Complaint</strong><p className="web-clinical-text">{String(summary.data.chief_complaint??'No complaint provided')}</p><strong>Clinical notes</strong><p className="web-clinical-text">{String(summary.data.clinical_notes??'No notes provided')}</p></div><div className="slot-primary-actions"><button className="slot-btn sec" onClick={history}>Patient History</button><button className="slot-btn sec" onClick={documents}>Documents</button><button className="slot-btn pri" onClick={complete}>Complete consultation</button></div></>}</>;
}
export function Completion({appointment:a,onCompleted}:{appointment:Appointment;onCompleted:()=>void}) {
  const [outcome,setOutcome]=useState('COMPLETED_WITH_RX');const [reason,setReason]=useState('');const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [confirmed,setConfirmed]=useState(false);const key=useRef(crypto.randomUUID());const submitted=useRef(false);const previous=useRef('');
  async function submit(e:React.FormEvent){e.preventDefault();if(submitted.current)return;submitted.current=true;setBusy(true);setError('');const body=outcome==='COMPLETED_NO_RX'?{reason}:{outcome};const serialized=JSON.stringify(body);if(previous.current&&previous.current!==serialized)key.current=crypto.randomUUID();previous.current=serialized;try{await post(`consultations/${a.id}/${outcome==='COMPLETED_NO_RX'?'complete-no-rx':'complete'}`,body,key.current);setConfirmed(true);onCompleted();}catch(e){setError(errorText(e));}finally{submitted.current=false;setBusy(false);}}
  if(confirmed)return <State empty="Consultation completion confirmed by the server"/>;
  return <form className="web-form" onSubmit={submit}><p>Complete the consultation with {a.patient_display_name}. The server records the outcome and financial state exactly once.</p><label>Clinical outcome<select value={outcome} disabled={busy} onChange={e=>setOutcome(e.target.value)}><option value="COMPLETED_WITH_RX">Complete with prescription</option><option value="COMPLETED_NO_RX">Complete without prescription</option><option value="REFERRED">Refer patient</option><option value="ESCALATED">Escalate</option></select></label>{outcome==='COMPLETED_NO_RX'&&<label>Reason for no prescription<textarea required maxLength={1000} rows={3} value={reason} disabled={busy} onChange={e=>setReason(e.target.value)}/></label>}<State error={error}/><button className="sheet-btn primary" type="submit" disabled={busy||(outcome==='COMPLETED_NO_RX'&&!reason.trim())}>{busy?'Completing…':'Confirm consultation completion'}</button></form>;
}
export function SlotForm({onCreated,onClose}:{onCreated:(date:string)=>void;onClose:()=>void}) {
  const [error,setError]=useState('');const [busy,setBusy]=useState(false);const [done,setDone]=useState(false);const [batch,setBatch]=useState(true);const [selectedDate,setDate]=useState('');const [startTime,setStart]=useState('08:00');const [endTime,setEnd]=useState('09:00');const lock=useRef(false);const [created,setCreated]=useState<string[]>([]);
  const pending=useRef<{ranges:Date[][];keys:string[];saved:string[]}|null>(null);
  const start=new Date(`${selectedDate}T${startTime}:00+03:00`),end=new Date(`${selectedDate}T${endTime}:00+03:00`);
  const count=Number.isFinite(start.getTime())&&end>start?(batch?Math.ceil((end.getTime()-start.getTime())/600000):1):0;
  async function submit(){if(lock.current||done)return;setError('');if(!count){setError('Choose a date and an end time after the start time.');return;}if(start.getTime()<=Date.now()){setError('Choose a future start time in East Africa Time (UTC+3).');return;}if(count>24){setError('Choose an availability block of at most four hours.');return;}lock.current=true;setBusy(true);
    const ranges=batch?Array.from({length:count},(_,i)=>[new Date(start.getTime()+i*600000),new Date(Math.min(end.getTime(),start.getTime()+(i+1)*600000))]):[[start,end]];
    pending.current??={ranges,keys:ranges.map(()=>crypto.randomUUID()),saved:[]};const job=pending.current;const generation=runtimeGeneration();
    try{for(let i=job.saved.length;i<job.ranges.length;i++){if(generation!==runtimeGeneration())return;const[from,to]=job.ranges[i];const dto=await post<RecordDto>('doctor/slots',{start_time:from.toISOString(),end_time:to.toISOString()},job.keys[i]);if(generation!==runtimeGeneration())return;job.saved.push(String(dto.id));setCreated([...job.saved]);}setDone(true);onCreated(selectedDate);}
    catch(e){if(!job.saved.length&&e instanceof ApiError&&[409,422].includes(e.status))pending.current=null;setError((job.saved.length?`${job.saved.length} slots were created. Retry continues with the remaining slots. `:'')+errorText(e));if(job.saved.length)onCreated(selectedDate);}finally{lock.current=false;setBusy(false);}
  }
  const frozen=busy||done||!!pending.current;
  return <><Prototype name="slotGeneratorModal" action={(handler,event)=>{
    if(handler.includes('closeSlotGeneratorModal')){if(event.target===event.currentTarget)onClose();}
    else if(handler==='executeSlotGeneration()')void submit();
  }} override={n=>{
    if(id(n)==='slotGeneratorModal')return {props:{className:'',style:{display:'contents'},'data-onclick':null}};
    if(hasClass(n,'bottom-sheet'))return {props:{style:{display:'contents'}}};
    if(hasClass(n,'sheet-desc'))return {children:<>{batch?'Auto-generate sequential 10-minute slots in your schedule':'Create one availability slot'}<label style={{display:'block',marginTop:8}}>Date · East Africa Time (UTC+3)<input aria-label="Slot date" type="date" required value={selectedDate} disabled={frozen} onChange={e=>setDate(e.target.value)} style={{display:'block',width:'100%',marginTop:3,padding:'8px 9px',borderRadius:10,border:'1px solid var(--border)',fontSize:12}}/></label></>};
    if(id(n)==='slotGenStartTime'||id(n)==='slotGenEndTime'){const isStart=id(n)==='slotGenStartTime';return {tag:'input',props:{type:'time',value:isStart?startTime:endTime,defaultValue:undefined,disabled:frozen,'aria-label':isStart?'Start time':'End time',onChange:(e:React.ChangeEvent<HTMLInputElement>)=>isStart?setStart(e.target.value):setEnd(e.target.value)},children:null};}
    if(id(n)==='slotGenCountPreview')return {children:count?`${count} slot${count===1?'':'s'}`:'Select date and time'};
    if(n.tag==='b'&&n.line===8503)return {children:batch?'10 Minutes / Patient':'Selected availability block'};
    if(n.tag==='span'&&n.line===8511)return {children:'Booked slots are locked'};
    if(hasClass(n,'flow-cta'))return {props:{disabled:busy||done},children:<span>{busy?'Publishing…':done?'Published by the server ✓':pending.current?'Retry remaining slots':'Generate & Publish Slots to Calendar 📅'}</span>};
    return undefined;
  }}/><label style={{display:'flex',gap:6,alignItems:'center',fontSize:11,marginTop:10}}><input type="checkbox" checked={batch} disabled={frozen} onChange={e=>setBatch(e.target.checked)}/>Generate sequential 10-minute slots</label><State error={error} empty={done?`${created.length} slot(s) created by the server`:undefined}/></>;
}

export function History({appointment:a,onDocument}:{appointment:Appointment;onDocument:(appointmentId:string,document:RecordDto)=>void}) {
  const history=useResource<RecordDto[]>(`doctor/appointments/${a.id}/patient-history?limit=25&offset=0`);
  const [episode,setEpisode]=useState<string|null>(null);const [paging,setPaging]=useState(false);const [error,setError]=useState('');const [hasMore,setHasMore]=useState(true);
  useEffect(()=>{const timer=setInterval(()=>{setEpisode(null);void history.refresh();},60000);return()=>clearInterval(timer);},[history.refresh]);
  async function more(){if(!history.data||paging)return;setPaging(true);try{const rows=await api<RecordDto[]>(`doctor/appointments/${a.id}/patient-history?limit=25&offset=${history.data.length}`);history.setData([...history.data,...rows]);setHasMore(rows.length===25);}catch(e){setError(errorText(e));history.setData(undefined);}finally{setPaging(false);}}
  return <Prototype name="patientHistoryModal" override={n=>{
    if(id(n)==='patientHistoryModal')return {props:{className:'',style:{display:'contents'}}};
    if(hasClass(n,'bottom-sheet'))return {props:{style:{display:'contents'}}};
    if(hasClass(n,'sheet-header-top'))return {omit:true};
    if(id(n)==='histPatientTabs')return {children:null};
    if(id(n)==='histAvatar')return {children:initials(a.patient_display_name)};
    if(id(n)==='histPatientName')return {children:a.patient_display_name};
    if(id(n)==='histPatientMeta')return {children:`Appointment context: ${a.appointment_number}`};
    if(id(n)==='histBloodGroup')return {children:'—'};
    if(id(n)==='histAllergyAlert')return {omit:true};
    if(id(n)==='histContinuityBanner')return {children:<><strong>Authorized patient history</strong><span>{history.data?.length??'—'} loaded encounters</span></>};
    if(id(n)==='histEncountersTimeline')return {children:<><State loading={history.loading} error={history.error||error} empty={!history.loading&&!history.error&&history.data?.length===0?'No previous encounters':undefined} retry={()=>void history.refresh()}/>{episode?<><button className="dw-btn-mini" onClick={()=>setEpisode(null)}>← All encounters</button><Episode key={episode} appointment={a} episode={episode} onDocument={onDocument}/></>:history.data?.map(row=><div className="history-encounter-card" key={String(row.historical_appointment_id)} style={{padding:12,border:'1px solid var(--border)',borderRadius:12,marginBottom:10}}><span className="badge badge-blue">{String(row.status)}</span><h3 style={{fontSize:13,marginTop:6}}>{date(row.date as string)} · {String(row.doctor_name)}</h3><p className="web-clinical-text">{String(row.presenting_problem??'No presenting problem provided')}</p><p style={{fontSize:11}}>{String(row.specialty??'')} · {String(row.document_count??0)} documents</p>{row.historical_consultation_id?<button className="dw-btn-mini" onClick={()=>setEpisode(String(row.historical_consultation_id))}>View historical episode →</button>:<p className="web-gap">No consultation episode available</p>}</div>)}{!episode&&hasMore&&(history.data?.length??0)>=25&&<button className="slot-btn sec" disabled={paging} onClick={()=>void more()}>{paging?'Loading…':'Load more history'}</button>}</>};
    if(id(n)==='histNotesBox')return {children:<p className="web-gap">Access is anchored to this consultation and remains subject to server authorization. Public Q&amp;A does not grant clinical record access.</p>};
    if(id(n)==='histImportBtn')return {props:{disabled:true},children:'Handwritten Rx only · No automatic import'};
    if(n.tag==='button'&&n.line>=8220)return {omit:true};
    return undefined;
  }}/>;
}
function Episode({appointment:a,episode,onDocument}:{appointment:Appointment;episode:string;onDocument:(appointmentId:string,document:RecordDto)=>void}) {
  const detail=useResource<RecordDto>(`doctor/appointments/${a.id}/patient-history/${episode}`);
  const d=detail.data;
  const documents=[...((d?.documents as RecordDto[])??[]),...((d?.prescription_pages as RecordDto[])??[])].filter(row=>!row.is_no_rx_required);
  return <><State loading={detail.loading} error={detail.error} retry={()=>void detail.refresh()}/>{d&&<div className="history-notes-box"><strong>{date(d.date as string)} · {String(d.doctor_name)}</strong><p className="web-clinical-text">{String(d.presenting_problem??'No complaint provided')}</p><p className="web-clinical-text">{String(d.clinical_notes??'No clinical notes provided')}</p>{documents.map((doc,index)=><button key={String(doc.id)} className="slot-btn sec" onClick={()=>onDocument(String(d.historical_appointment_id),doc)}>View document page {String(doc.page_number??index+1)}</button>)}</div>}</>;
}
export function Documents({appointment:a,onDocument,onClose}:{appointment:Appointment;onDocument:(appointmentId:string,document:RecordDto)=>void;onClose:()=>void}) {
  const summary=useResource<RecordDto>(`doctor/consultations/${a.id}/summary`);
  const detail=useResource<RecordDto>(summary.data?.consultation_id?`doctor/appointments/${a.id}/patient-history/${String(summary.data.consultation_id)}`:null);
  const [mode,setMode]=useState('pdf');const [selected,setSelected]=useState(0);
  const documents=[...((detail.data?.documents as RecordDto[])??[]),...((detail.data?.prescription_pages as RecordDto[])??[])].filter(row=>!row.is_no_rx_required);
  const doc=documents[selected];
  return <Prototype name="labReportModal" action={handler=>{if(handler.includes('close'))onClose();else if(handler.includes('setReportViewMode'))setMode(handler.includes('table')?'table':'pdf');else if(doc)onDocument(a.id,doc);}} override={n=>{
    if(id(n)==='labReportModal')return {props:{className:'',style:{display:'contents'},'data-onclick':null}};
    if(hasClass(n,'bottom-sheet'))return {props:{style:{display:'contents'}}};
    if(hasClass(n,'sheet-pill-tag'))return {children:'Authorized Patient Documents'};
    if(id(n)==='btnModeTable')return {omit:true};
    if(id(n)==='labReportStatusBadge')return {children:'Server authorization required',props:{className:'badge badge-blue'}};
    if(id(n)==='btnModePdf'||id(n)==='btnModeTable')return {props:{className:`pdf-mode-pill ${(id(n)==='btnModePdf'?mode==='pdf':mode==='table')?'active':''}`}};
    if(id(n)==='pdfPageIndicator')return {children:documents.length?`Document ${selected+1} of ${documents.length}`:'No documents loaded'};
    if(id(n)==='labReportTabs')return {children:documents.map((row,index)=><button key={String(row.id)} className={`lab-tab-chip ${selected===index?'active':''}`} onClick={()=>setSelected(index)}>Document {index+1}{row.page_number?` · Rx page ${String(row.page_number)}`:''}</button>)};
    if(id(n)==='reportScrollArea')return {children:<><State loading={summary.loading||detail.loading} error={summary.error||detail.error} retry={()=>{void summary.refresh();void detail.refresh();}}/>{mode==='table'?<State empty="Only uploaded clinical documents are available."/>:<div className="pdf-paper"><State empty={!summary.loading&&!detail.loading&&!summary.error&&!detail.error&&!documents.length?'No authorized documents are available for this consultation':undefined}/>{doc&&<><p className="web-clinical-text">{a.patient_display_name} · {a.appointment_number}</p><div id="pdfMultiPageBar" style={{display:'flex',alignItems:'center',justifyContent:'space-between',gap:8,margin:'12px 0'}}><button className="pdf-mode-pill" disabled={selected===0} onClick={()=>setSelected(i=>i-1)}>‹ Prev Page</button><span>Document {selected+1} of {documents.length}</span><button className="pdf-mode-pill" disabled={selected===documents.length-1} onClick={()=>setSelected(i=>i+1)}>Next Page ›</button></div><button className="sheet-btn primary" onClick={()=>onDocument(a.id,doc)}>Open authorized document</button><p className="web-gap">Document access is reauthorized by the server. Files and signed storage URLs are not persisted in browser storage.</p></>}</div>}</>};
    if(id(n)==='labAttachRxBtn')return {props:{disabled:true,title:'Handwritten prescription images are supported; importing laboratory findings into a generated prescription is unavailable'},children:'Attach to Rx · Unavailable'};
    if(n.tag==='button'&&n.line===8142)return {props:{disabled:!doc,onClick:()=>doc&&onDocument(a.id,doc)},children:'📥 Open / Download Document'};
    return undefined;
  }}/>;
}
export function DocumentViewer({current,appointmentId,document,onClose}:{current:string;appointmentId:string;document:RecordDto;onClose:()=>void}) {
  const [url,setUrl]=useState('');const [mime,setMime]=useState('');const [error,setError]=useState('');const [busy,setBusy]=useState(true);const blob=useRef('');
  const revoke=useCallback(()=>{if(blob.current)URL.revokeObjectURL(blob.current);blob.current='';setUrl('');},[]);
  const load=useCallback(async()=>{revoke();setBusy(true);setError('');try{const file=await authorizedDocument(`/api/document/${appointmentId}/${String(document.id)}?current_appointment_id=${current}`);setMime(file.type);blob.current=URL.createObjectURL(file);setUrl(blob.current);}catch(e){if(!(e instanceof DOMException&&e.name==='AbortError'))setError(errorText(e));}finally{setBusy(false);}},[appointmentId,document.id,current,revoke]);
  useEffect(()=>{void load();const timer=setTimeout(()=>{revoke();setError('Document access expired. Reauthorize to view it again.');},60000);return()=>{clearTimeout(timer);if(blob.current)URL.revokeObjectURL(blob.current);};},[load,revoke]);
  return <Modal title="Authorized Document Viewer" onClose={onClose} wide><State loading={busy} error={error} retry={()=>void load()}/>{url&&<>{mime.startsWith('image/')?<img src={url} alt="Authorized clinical document" style={{width:'100%',maxHeight:'65dvh',objectFit:'contain'}}/>:<p className="web-gap">PDF document ready for download. Open it using your browser&apos;s PDF viewer.</p>}<a className="sheet-btn primary" href={url} download={mime==='application/pdf'?'clinical-document.pdf':'clinical-document.jpg'}>Download authorized document</a></>}</Modal>;
}
export function Receipt({row,onClose}:{row:RecordDto;onClose:()=>void}) {
  return <Prototype name="docConsultationFeeModal" action={onClose} override={n=>{
    if(id(n)==='docConsultationFeeModal')return {props:{className:'',style:{display:'contents'}}};
    if(hasClass(n,'bottom-sheet'))return {props:{style:{display:'contents'}}};
    if(id(n)==='dcfmTrxId')return {children:`REF: ${String(row.transaction_number)}`};
    if(id(n)==='dcfmPatientTitle')return {children:`Fee Breakdown: ${String(row.patient_display_name)}`};
    if(id(n)==='dcfmSessionSub')return {children:date(String(row.created_at))};
    const values:Record<string,React.ReactNode>={dcfmGross:money(row.gross_amount),dcfmWithheld:money(row.platform_fee_amount),dcfmNet:money(row.net_amount)};
    if(id(n) in values)return {children:values[id(n)]};
    if(hasClass(n,'doc-earnings-step-label'))return {children:n.line<8940?'1. Gross eligible fee':n.line<8952?'2. Platform charge':'3. Doctor earning (net)'};
    if(hasClass(n,'doc-fee-pill'))return {omit:true};
    if(hasClass(n,'doc-earnings-calc-note'))return {children:<div>Financial state: {String(row.payment_status).replaceAll('_',' ')}. Eligible earning does not confirm a provider payout.</div>};
    return undefined;
  }}/>;
}
