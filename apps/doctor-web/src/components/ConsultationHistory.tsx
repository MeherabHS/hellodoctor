"use client";
import { useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { Appointment } from '../lib/contracts';
import { ClinicalAction,date,time } from './Queue';
import { State } from './Common';
export function ConsultationHistory({action}:{action:ClinicalAction}){
 const history=useResource<Appointment[]>('doctor/appointments?view=history&limit=100&offset=0');
 const [more,setMore]=useState(true);const[busy,setBusy]=useState(false);const[error,setError]=useState('');
 async function loadMore(){if(busy||!history.data)return;setBusy(true);try{const next=await api<Appointment[]>(`doctor/appointments?view=history&limit=100&offset=${history.data.length}`);history.setData(old=>[...(old??[]),...next]);setMore(next.length===100);}catch(e){setError(errorText(e));}finally{setBusy(false);}}
 return <section><header className="dw-page-header"><div className="dw-page-title-group"><h2>Consultation History</h2><p>Your past encounters, transcripts, clinical attachments and prescriptions.</p></div><button className="dw-btn-outline" onClick={()=>void history.refresh()}>Refresh history</button></header>
 <State loading={history.loading} error={history.error||error} retry={()=>void history.refresh()}/>
 {!history.loading&&!history.data?.length&&!history.error&&<p>No consultation history yet.</p>}
 {history.data?.map(a=><article className="queue-card" key={a.id}><h3>{a.patient_display_name}</h3><p>{a.modality==='CHAT'?'Chat consultation':'Video consultation'} - {date(a.start_time)} {time(a.start_time)} - {a.status}</p><div className="web-action-row"><button className="dw-btn-outline-sm" onClick={()=>action('summary',a)}>Encounter summary</button><button className="dw-btn-outline-sm" onClick={()=>action('chat',a)}>Chat transcript</button><button className="dw-btn-outline-sm" onClick={()=>action('documents',a)}>Attachments and Rx</button></div></article>)}
 {more&&(history.data?.length??0)>=100&&<button className="dw-btn-outline" disabled={busy} onClick={()=>void loadMore()}>Load more history</button>}
 </section>;
}
