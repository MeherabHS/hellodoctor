'use client';
import { consultationAction, consultationActionLabel } from '../lib/contracts';
import { useEffect, useState } from 'react';
import { ChatThread } from './Chat';
import { useResource } from '../lib/client';
import { Appointment, Profile, RecordDto, Conversation, Question, money } from '../lib/contracts';
import { Prototype, hasClass, id, text } from './Prototype';
import { State } from './Common';
import { ClinicalAction, QueueState, date, dayKey, initials, statusLabel, time } from './Queue';
import { useAgoraCall } from '../lib/useAgoraCall';
import { callStatusText, callStatusTone } from '../lib/call-status';

export function DesktopAppointments({queue,action,addSlot,search,setSearch,schedule,gap}:{queue:QueueState;action:ClinicalAction;addSlot:()=>void;search:string;setSearch:(value:string)=>void;schedule:()=>void;gap:(message:string)=>void}) {
  const [filter,setFilter]=useState('upcoming');const [visible,setVisible]=useState(25);
  const rows=queue.data.filter(a=>a.queue_actionable===true&&`${a.patient_display_name} ${a.appointment_number}`.toLowerCase().includes(search.toLowerCase()));
  return <Prototype name="dwPage_appointments" action={handler=>{if(handler.includes('filterDoctorWebAppts'))setFilter(handler.match(/'([^']+)'/)?.[1]??'all');else addSlot();}} override={n=>{
    if(id(n)==='dwPage_appointments')return {props:{className:'dw-page-view active',style:{display:'flex',flex:1}}};
    if(hasClass(n,'dw-filter-pills'))return {children:<>{[['upcoming','Current and remaining today']].map(([value,label])=><button key={value} className={`dw-filter-pill ${value===filter?'active':''}`} onClick={()=>setFilter(value)}>{label}</button>)}<input className="auth-input" style={{margin:0,maxWidth:250,padding:'6px 12px',fontSize:11.5,borderRadius:20}} aria-label="Search appointments" placeholder="Search patient or appointment…" value={search} onChange={e=>setSearch(e.target.value)}/><button className="dw-btn-outline-sm" onClick={()=>void queue.refresh()}>Refresh</button></>};
    if(n.tag==='button'&&hasClass(n,'dw-btn-outline'))return {children:date(new Date().toISOString()),props:{onClick:schedule,'aria-label':'Manage consultation availability'}};
    if(n.tag==='button'&&hasClass(n,'dw-btn-primary'))return {children:'+ Add Schedule Slot',props:{onClick:addSlot}};
    if(id(n)==='dwApptCardsGrid')return {children:<><State loading={queue.loading} error={queue.error} empty={!queue.loading&&!queue.error&&!rows.length?'No matching appointments':undefined} retry={()=>void queue.refresh()}/>{rows.slice(0,visible).map(a=><div className="dw-pt-card" key={a.id} data-appointment-id={a.id} style={{borderLeft:`4px solid ${a.status==='COMPLETED'?'#94A3B8':'#10B981'}`}}><div style={{display:'flex',justifyContent:'space-between',alignItems:'flex-start',gap:10}}><div><span className="badge badge-green" style={{fontSize:9.5,fontWeight:800}}>{statusLabel(a,queue.data[0]?.id)}</span><div style={{fontSize:14.5,fontWeight:800,color:'#0F291E',marginTop:4}}>{a.patient_display_name}</div><div style={{fontSize:11,color:'#64748B'}}>{a.appointment_number} · {date(a.start_time)}</div></div><div style={{textAlign:'right'}}><div style={{fontSize:13,fontWeight:800,color:'#059669'}}>{time(a.start_time)} – {time(a.end_time)}</div><div style={{fontSize:10.5,color:'#64748B'}}>{a.modality}</div></div></div><div style={{fontSize:11.5,color:'#334155',lineHeight:1.45,background:'#F8FAFC',padding:'8px 10px',borderRadius:8}}><b>Reason:</b> {a.clinical_outcome??'Open authorized intake for presenting complaint.'}</div><div style={{display:'flex',gap:6,flexWrap:'wrap',paddingTop:6,marginTop:'auto'}}><button className="dw-btn-primary" disabled={!consultationAction(a)} onClick={()=>consultationAction(a)&&action(consultationAction(a)!,a)}>{consultationActionLabel(a)}</button><button className="dw-btn-outline-sm" onClick={()=>action('chat',a)}>Chat</button><button className="dw-btn-outline-sm" onClick={()=>action('rx',a)}>Write Rx</button><button className="dw-btn-outline-sm" onClick={()=>action('summary',a)}>Review Chart</button>{!['COMPLETED','CANCELLED'].includes(a.status)&&<button className="dw-btn-outline-sm" onClick={()=>gap('Rescheduling a booked consultation is not supported by the current Doctor contract. Booked slots remain locked.')}>Reschedule</button>}{a.status==='COMPLETED'&&<button className="dw-btn-outline-sm" onClick={()=>action('documents',a)}>View Signed Rx</button>}</div></div>)}{rows.length>visible&&<button className="dw-btn-outline" onClick={()=>setVisible(v=>v+25)}>Load more appointments</button>}</>};
    return undefined;
  }}/>;
}
export function Dashboard({profile,queue,onAppointments,earnings,qna,action,paymentRequired=true}:{profile:Profile;queue:QueueState;onAppointments:()=>void;earnings:()=>void;qna:()=>void;action:ClinicalAction;paymentRequired?:boolean}) {
  const wallet=useResource<RecordDto>(paymentRequired?'doctor/wallet/summary':null);
  const questions=useResource<Question[]>('doctor/qna/questions?status=OPEN&limit=100&offset=0');
  const [now,setNow]=useState(Date.now());
  useEffect(()=>{const timer=setInterval(()=>setNow(Date.now()),60000);return()=>clearInterval(timer);},[]);
  const today=dayKey(new Date(now).toISOString());
  const activeQueue=queue.data.filter(a=>a.queue_actionable===true);
  const todaysConsultations=queue.data.filter(a=>dayKey(a.start_time)===today&&!['CANCELLED','REFUNDED','EXPIRED'].includes(a.status)).length;
  const next=activeQueue.find(a=>Date.parse(a.start_time)>=now);
  return <section id="dwPage_dashboard" className="dw-page-view active">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>Good morning, {profile.full_name}</h2><p>{profile.primary_specialty} &middot; {date(new Date(now).toISOString())}</p></div><button className="dw-btn-outline" disabled={queue.loading||wallet.loading||questions.loading} onClick={()=>{void queue.refresh();void wallet.refresh();void questions.refresh();}}>Refresh overview</button></header>
    <div className="dw-kpi-grid web-dashboard-summary">
      <section className="dw-kpi-card" aria-label="Today's Consultations"><div className="dw-kpi-top"><h3 className="dw-kpi-lbl">Today's Consultations</h3><span className="dw-kpi-icon" aria-hidden="true"><span className="web-icon web-icon-calendar"/></span></div><div className="dw-kpi-val">{queue.loading||queue.error?'—':todaysConsultations}</div><p className="dw-kpi-sub">Scheduled and completed today</p></section>
      <section className="dw-kpi-card" aria-label="Next Consultation"><div className="dw-kpi-top"><h3 className="dw-kpi-lbl">Next Consultation</h3></div><div className="dw-kpi-val">{queue.loading||queue.error?'—':next?time(next.start_time):'None'}</div>{next&&<p className="dw-kpi-sub">{next.patient_display_name} &middot; {date(next.start_time)}</p>}</section>
      <section className="dw-kpi-card" aria-label="Eligible Earnings"><div className="dw-kpi-top"><h3 className="dw-kpi-lbl">Eligible Earnings</h3></div>{!paymentRequired?<p className="dw-kpi-sub">Financial module not yet active</p>:wallet.data?<><div className="dw-kpi-val">{money(wallet.data.final_net)}</div><p className="dw-kpi-sub">Lifetime eligible net</p><p className="dw-kpi-sub">Gross {money(wallet.data.total_gross)} &middot; Platform {money(wallet.data.withheld_fee)}</p></>:<State loading={wallet.loading} error={wallet.error} retry={()=>void wallet.refresh()}/>}{paymentRequired&&<button className="dw-btn-mini" onClick={earnings}>View Wallet</button>}</section>
      <section className="dw-kpi-card" aria-label="Open Q&A"><div className="dw-kpi-top"><h3 className="dw-kpi-lbl">Open Q&amp;A</h3></div>{questions.data?<><div className="dw-kpi-val">{questions.data.length}{questions.data.length===100?'+':''}</div><p className="dw-kpi-sub">Awaiting a Doctor response</p></>:<State loading={questions.loading} error={questions.error} retry={()=>void questions.refresh()}/>}<button className="dw-btn-mini" onClick={qna}>Review questions</button></section>
    </div>
    <section className="dw-section-card web-dashboard-queue"><div className="dw-section-header"><h3 className="dw-section-title">Upcoming / Active Consultations</h3><button className="dw-btn-outline-sm" onClick={onAppointments}>View appointments &rarr;</button></div>
      <State loading={queue.loading} error={queue.error} empty={!queue.loading&&!queue.error&&!activeQueue.length?'No consultations scheduled today.':undefined} retry={()=>void queue.refresh()}/>
      {!queue.loading&&!queue.error&&activeQueue.length>0&&<div className="web-section-body">{activeQueue.slice(0,5).map(a=><article key={a.id} data-appointment-id={a.id} className={`web-overview-appointment ${next?.id===a.id?'next':''}`}><div className="web-overview-time"><strong>{time(a.start_time)}</strong><span>{date(a.start_time)}</span></div><div className="web-overview-patient"><strong>{a.patient_display_name}</strong><p>{statusLabel(a,next?.id,now)} &middot; {a.modality}</p></div><div className="web-action-row"><button className="dw-btn-primary" disabled={!consultationAction(a)} onClick={()=>consultationAction(a)&&action(consultationAction(a)!,a)}>{consultationActionLabel(a)}</button><button className="dw-btn-outline-sm" onClick={()=>action('chat',a)}>Chat</button></div></article>)}</div>}
    </section>
  </section>;
}
export function Patients({queue,action,gap}:{queue:QueueState;action:ClinicalAction;gap:(message:string)=>void}) {
  const [search,setSearch]=useState('');
  const rows=queue.data.filter(a=>`${a.patient_display_name} ${a.appointment_number}`.toLowerCase().includes(search.toLowerCase()));
  return <Prototype name="dwPage_patients" action={handler=>{if(handler.includes('Category'))gap('Allergy/chronic-condition filters require data that is not exposed by the appointment list. Open an authorized clinical episode.');else gap('Patient registration is not a Doctor capability in the current contract.');}} override={n=>{
    if(id(n)==='dwPage_patients')return {props:{className:'dw-page-view active',style:{display:'flex',flex:1}}};
    if(n.tag==='button'&&String(n.props['data-onclick']).includes('Patient Registration'))return {omit:true};
    if(hasClass(n,'dw-page-title-group'))return {children:<><h2>Patient Longitudinal Directory</h2><p>Own appointment contexts &middot; Clinical records require current consultation authorization</p></>};
    if(id(n)==='dwPatientSearchInput')return {props:{value:search,onChange:(e:React.ChangeEvent<HTMLInputElement>)=>setSearch(e.target.value)}};
    if(hasClass(n,'dw-filter-pills'))return {children:<><button className="dw-filter-pill active">All consultation contexts ({queue.data.length})</button>{['Allergy Alerts','Chronic Conditions','Recent Encounters'].map(label=><button key={label} className="dw-filter-pill" onClick={()=>gap('The directory cannot infer medical attributes. Open a currently authorized encounter.')}>{label}</button>)}</>};
    if(n.tag==='tbody')return {children:<>{(queue.loading||queue.error||!rows.length)&&<tr><td colSpan={7}><State loading={queue.loading} error={queue.error} empty={!queue.loading&&!queue.error&&!rows.length?'No matching patients':undefined} retry={()=>void queue.refresh()}/></td></tr>}{rows.map(a=><tr key={a.id}><td><div style={{display:'flex',gap:10,alignItems:'center'}}><div className="dw-doc-avatar-img">{initials(a.patient_display_name)}</div><div><b>{a.patient_display_name}</b><div style={{fontSize:10.5,color:'#64748B'}}>{a.appointment_number}</div></div></div></td><td>—</td><td>—</td><td>Not supplied</td><td>Not supplied</td><td>{date(a.start_time)}<div style={{fontSize:10.5}}>{a.status}</div></td><td><div style={{display:'flex',gap:6}}><button className="dw-btn-mini" disabled={!consultationAction(a)} onClick={()=>consultationAction(a)&&action(consultationAction(a)!,a)}>{consultationActionLabel(a)}</button><button className="dw-btn-mini" onClick={()=>action('history',a)}>Dossier</button><button className="dw-btn-mini" onClick={()=>action('documents',a)}>Documents</button></div></td></tr>)}</>};
    return undefined;
  }}/>;
}
export function Telehealth({appointment:a,profile,action,onChat,complete,gap}:{appointment:Appointment|null;profile:Profile;action:ClinicalAction;onChat:()=>void;complete:()=>void;gap:(message:string)=>void}) {
  const [drawer,setDrawer]=useState('history');const [tab,setTab]=useState('encounters');const [full,setFull]=useState(false);
  const history=useResource<RecordDto[]>(a&&drawer==='history'&&!full?`doctor/appointments/${a.id}/patient-history?limit=25&offset=0`:null);
  const chats=useResource<Conversation[]>(a&&drawer==='chat'&&!full?'doctor/chats?limit=100&offset=0':null);
  const chat=chats.data?.find(row=>row.appointment_id===a?.id);
  const eligible=a!==null&&consultationAction(a)==='call';
  const call=useAgoraCall(eligible?a!.id:null);
  const callActive=['connecting','waiting-for-remote','connected','reconnecting'].includes(call.state.phase);
  const labels:Record<string,React.ReactNode>={dwMeetRoomCode:'Consultation workspace',dwMeetCornerPtName:a?.patient_display_name??'No active consultation',dwMeetClock:time(new Date().toISOString()),dwMeetSessionTag:a?.appointment_number??'No appointment selected',dwHistPtAvatar:a?initials(a.patient_display_name):'👤',dwHistPtName:a?.patient_display_name??'No appointment selected',dwHistPtId:a?.appointment_number??'—',dwHistPtBlood:'—',dwHistPtMeta:'Records require current consultation authorization',dwHistVitBp:'—',dwHistVitPulse:'—',dwHistVitTemp:'—',dwHistVitSpo2:'—',dwHistEncCount:'—',dwChatPtSubheader:a?.patient_display_name??'No consultation selected'};
  return <Prototype name="dwPage_telehealth" action={handler=>{
    if(handler.includes('toggleDoctorWebMic'))call.toggleMic();
    else if(handler.includes('toggleDoctorWebCamera'))call.toggleCamera();
    else if(handler.includes('showAppToast')&&handler.includes('Screen'))call.toggleScreenShare();
    else if(handler.includes('toggleDoctorWebDrawer')){const target=handler.match(/'([^']+)'/)?.[1]??'history';setDrawer(target==='close'||drawer===target?'':target);}
    else if(handler.includes('setDoctorWebSideView'))setDrawer(handler.match(/'([^']+)'/)?.[1]??'history');
    else if(handler.includes('setDoctorWebHistView'))setTab(handler.match(/'([^']+)'/)?.[1]??'encounters');
    else if(handler.includes('Fullscreen'))setFull(!full);
    else if(handler.includes('QuickRx')&&a)action('rx',a);
    else if(handler.includes('PatientHistory')&&a)action('history',a);
    else if(handler.includes('LabReport')&&a)action('documents',a);
    else if(handler.includes('LabOrder'))gap('Laboratory ordering has no current Doctor backend contract.');
    else if(handler.includes('endVideoCall')){call.leave();complete();}
    else gap('Live reactions are not part of the current Doctor contract.');
  }} override={n=>{
    if(id(n)==='dwPage_telehealth')return {props:{className:'dw-page-view active',style:{display:'flex',flex:1,padding:0,background:'transparent'}}};
    if(['dwHistLabBtn','dwHistTabLabs'].includes(id(n))||n.tag==='button'&&String(n.props['data-onclick']).includes('triggerDoctorWebLabOrder'))return {omit:true};
    if(id(n)in labels)return {children:labels[id(n)]};
    if(id(n)==='dwRemoteAvatar')return {children:a?initials(a.patient_display_name):'👤'};
    if(id(n)==='dwVideoPtNameLarge')return {children:a?.patient_display_name??'Select a consultation'};
    if(hasClass(n,'dw-meet-canvas'))return {before:call.state.phase==='connected'&&call.state.remotePresent?<div key="remote-video" ref={call.remoteVideoRef} style={{position:'absolute',inset:0,zIndex:5}}/>:null};
    if(hasClass(n,'dw-meet-status-badge'))return eligible&&(call.state.phase==='idle'||call.state.phase==='failed')
      ?{children:<div style={{display:'flex',flexDirection:'column',alignItems:'center',gap:4}}>{call.state.error&&<span style={{color:'#B91C1C',fontSize:10.5,fontWeight:600}}>{call.state.error}</span>}<button className="dw-btn-primary" style={{padding:'4px 16px',fontSize:11.5}} onClick={call.join}>{call.state.phase==='failed'?'Retry joining':'Join Video Consultation'}</button></div>}
      :{children:<span id="dwVideoPtMetaLine">{callStatusText(a,eligible,call.state.phase,call.state.remotePresent)}</span>};
    if(hasClass(n,'dw-meet-pip-avatar'))return {children:callActive?<div ref={call.localVideoRef} style={{width:'100%',height:'100%',borderRadius:'50%',overflow:'hidden',transform:'scaleX(-1)'}}/>:initials(profile.full_name)};
    if(hasClass(n,'dw-meet-pip-meta'))return {children:`${profile.full_name} (You)`};
    if(hasClass(n,'dw-meet-room-tag'))return {children:<><span>Consultation workspace</span><span className={`badge badge-${callStatusTone(call.state.phase,eligible)}`}>{callStatusText(a,eligible,call.state.phase,call.state.remotePresent)}</span></>};
    if(hasClass(n,'badge')&&n.line===9116)return {children:'Idle'};
    if(id(n)==='dwHistAllergyAlert')return {omit:true};
    if(id(n)==='dwMeetSideDrawer')return {props:{className:`dw-meet-side-panel ${!drawer||full?'collapsed':''}`}};
    if(id(n)==='dwMeetHistView')return {props:{className:`dw-swap-view ${drawer==='history'?'active':''}`,style:{display:drawer==='history'?'flex':'none'}}};
    if(id(n)==='dwMeetChatView')return {props:{className:`dw-swap-view ${drawer==='chat'?'active':''}`,style:{display:drawer==='chat'?'flex':'none'}},children:chat?<ChatThread chat={chat} profile={profile} appointment={a??undefined} onBack={()=>setDrawer('history')} action={action} gap={gap}/>:<div className="web-section-body"><State loading={chats.loading} error={chats.error} retry={()=>void chats.refresh()}/><strong>{a?.patient_display_name??'Consultation conversations'}</strong><p className="sheet-desc">Open your authorized consultation chats.</p><button className="dw-btn-primary" onClick={()=>a?action('chat',a):onChat()}>Open Consultation Chats</button></div>};
    if(id(n)==='dwHistScrollContent')return {children:<><State loading={history.loading} error={history.error} empty={!a?'Select a consultation to view authorized records':!history.loading&&!history.error&&history.data?.length===0?'No previous encounters':undefined} retry={()=>void history.refresh()}/>{tab==='encounters'?history.data?.map(row=><article className="dw-hist-card" key={String(row.historical_appointment_id)}><div className="dw-hist-enc-header"><div><div className="dw-hist-enc-date">{date(row.date as string)}</div><div className="dw-hist-enc-type">{String(row.doctor_name)} &middot; {String(row.specialty??'')}</div></div><span className="badge badge-green">{String(row.status)}</span></div><p className="dw-hist-field-row"><b>Chief Complaints:</b> {String(row.presenting_problem??'No presenting problem supplied')}</p><div className="dw-hist-outcome-box">{String(row.document_count??0)} authorized encounter documents</div>{a&&<button className="dw-btn-mini" onClick={()=>action('history',a)}>View historical episode &rarr;</button>}</article>):a&&<article className="dw-hist-card"><div className="dw-hist-enc-date">{tab==='meds'?'Handwritten Prescription Archive':tab==='labs'?'Diagnostics & Labs':'Physician Notes'}</div><p className="dw-hist-field-row">{tab==='meds'?'Prescription images remain available through authorized encounter documents.':tab==='labs'?'Open authorized clinical documents. Structured lab panels and lab ordering are unavailable.':'Clinical notes remain attached to their authorized consultation episode.'}</p><button className="dw-btn-mini" onClick={()=>action(tab==='labs'?'documents':'history',a)}>Open authorized records &rarr;</button>{tab==='meds'&&<button className="dw-btn-mini" onClick={()=>action('rx',a)}>Write handwritten Rx</button>}</article>}</>};
    if(hasClass(n,'dw-meet-badge-count'))return {omit:true};
    if(hasClass(n,'end-call'))return {props:{onClick:()=>{call.leave();complete();},disabled:!a},children:'Complete consultation'};
    if(['dwMeetMicBtn','dwMeetCamBtn'].includes(id(n))){const on=id(n)==='dwMeetMicBtn'?call.state.micEnabled:call.state.cameraEnabled;return {props:{disabled:!callActive,'aria-pressed':!on,title:`${on?'Turn off':'Turn on'} ${id(n)==='dwMeetMicBtn'?'microphone':'camera'}`}};}
    if(id(n)==='dwMeetPanelTitle')return {children:drawer==='chat'?'Consultation Chat':'Patient Case History & EMR'};
    if(['dwMeetSwapTabHist','dwMeetSwapTabChat','dwMeetToggleHistBtn','dwMeetToggleChatBtn'].includes(id(n))){const selected=id(n).includes('Chat')?drawer==='chat':drawer==='history';return {props:{className:`${hasClass(n,'dw-side-swap-tab')?'dw-side-swap-tab':'dw-meet-toggle-btn'} ${selected?'active':''}`,'aria-pressed':selected}};}
    if(id(n).startsWith('dwHistTab')){const selected={dwHistTabEncounters:'encounters',dwHistTabMeds:'meds',dwHistTabLabs:'labs',dwHistTabNotes:'notes'}[id(n)];return {props:{className:`dw-meet-subtab-btn ${tab===selected?'active':''}`}};}
    if(n.tag==='button'&&['btnDwSendRx','dwHistLabBtn'].includes(id(n)))return {props:{disabled:!a}};
    return undefined;
  }}/>;
}
