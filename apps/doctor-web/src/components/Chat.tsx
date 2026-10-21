'use client';
import { startScheduleSync } from '../lib/schedule-sync';
import { ClinicalAttachment, PaperclipIcon } from './ClinicalAttachment';
import { compressRxPhoto } from '../lib/rx-compression';
import { useEffect, useRef, useState } from 'react';
import { api, errorText, post, useResource, runtimeGeneration } from '../lib/client';
import { Appointment, Conversation, Message, Profile, chatLocked, chatActionLabel, messageSide, consultationContext, compareMessageInstants } from '../lib/contracts';
import { State } from './Common';
import { ClinicalAction, date, initials, time } from './Queue';

export function Chat({ profile, appointments, selectedAppointment, focusAppointment, visible=true, action, gap }: { profile: Profile; appointments: Appointment[]; selectedAppointment?: string; focusAppointment?: Appointment; visible?:boolean; action: ClinicalAction; gap: (message:string)=>void }) {
  const list = useResource<Conversation[]>('doctor/chats?limit=100&offset=0');
  const [selected,setSelected] = useState<Conversation | null>(null);
  const [search,setSearch] = useState('');
  const [filter,setFilter] = useState('all');
  const [moreBusy,setMoreBusy] = useState(false);
  const [moreError,setMoreError] = useState('');
  const [hasMore,setHasMore] = useState(false);
  const [now,setNow]=useState(Date.now());
  useEffect(()=>{const timer=setInterval(()=>setNow(Date.now()),10000);return()=>clearInterval(timer);},[]);
  useEffect(()=>{ if(list.data) setHasMore(list.data.length===100); },[list.data]);
  useEffect(()=>{
    if (!selectedAppointment) return;
    const found=list.data?.find(c=>c.appointment_id===selectedAppointment);
    if (found) {setSelected(found); return;}
    const context=consultationContext(appointments,selectedAppointment,focusAppointment);
    if (!context) return;
    let cancelled=false;
    api<Conversation>(`appointments/${selectedAppointment}/conversation`).then(c=>{
      if (!cancelled) setSelected({...c, patient_display_name:context.patient_display_name,
        modality:context.modality, appointment_status:context.status});
    }).catch(e=>{if(!cancelled)setMoreError(errorText(e));});
    return ()=>{cancelled=true;};
  },[selectedAppointment,list.data,appointments,focusAppointment]);
  async function loadMore() {
    if (!list.data || moreBusy) return;
    setMoreBusy(true);setMoreError('');
    try { const next=await api<Conversation[]>(`doctor/chats?limit=100&offset=${list.data.length}`); list.setData([...list.data,...next]);setHasMore(next.length===100); }
    catch(e) { setMoreError(errorText(e)); } finally { setMoreBusy(false); }
  }
  const filtered = (list.data??[]).filter(c=>c.patient_display_name.toLowerCase().includes(search.toLowerCase()) && (filter==='all' || filter==='active'&&!chatLocked(c,now) || filter==='locked'&&chatLocked(c,now)));
  const current=selected?(list.data?.find(c=>c.id===selected.id)??selected):null;
  const appointment=consultationContext(appointments,current?.appointment_id,focusAppointment);
  return <section className="web-chat-page">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>Patient Chats</h2><p>Consultation conversations and follow-up messages</p></div><button className="dw-btn-outline" disabled={list.loading} onClick={()=>void list.refresh()}>Refresh inbox</button></header>
    <div className="web-chat-workspace" id="webChatWorkspace">
      <aside className="dw-section-card web-chat-list" aria-label="Patient conversations"><div className="dw-section-header"><h3 className="dw-section-title">Conversations</h3></div><div className="web-section-body">
        <input className="auth-input" aria-label="Search conversations" placeholder="Search patient conversations..." value={search} onChange={e=>setSearch(e.target.value)}/>
        <div className="dw-filter-pills">{[['all','All'],['active','Active'],['locked','Read-only']].map(([value,label])=><button key={value} className={`dw-filter-pill ${filter===value?'active':''}`} onClick={()=>setFilter(value)}>{label}</button>)}</div>
        <State loading={list.loading} error={list.error||moreError} retry={()=>void list.refresh()}/>
        {!list.loading&&!list.error&&!filtered.length&&<div className="web-chat-list-empty" role="status"><ChatIcon/><strong>{!search&&filter==='all'?'No conversations yet':'No matching conversations'}</strong><p>Consultation conversations will appear here.</p></div>}
        {filtered.map(c=>{const context=appointments.find(a=>a.id===c.appointment_id);return <button className="dw-pt-card web-conversation-card" key={c.id} aria-pressed={current?.id===c.id} data-conversation-id={c.id} onClick={()=>setSelected(c)}><div className="web-action-row"><div className="dw-doc-avatar-img">{initials(c.patient_display_name)}</div><strong>{c.patient_display_name}</strong></div><span className={`badge ${chatLocked(c,now)?'badge-slate':'badge-green'}`}>{chatLocked(c,now)?'Read-only':'Active'}</span><span className="sheet-desc">{context?<>{context.appointment_number} &middot; {date(context.start_time)}</>:'Consultation conversation'}</span><span className="sheet-desc">{chatActionLabel(c,now)}</span><span className="sheet-desc">{c.last_message_at?<>Last message &middot; {date(c.last_message_at)} {time(c.last_message_at)}</>:'No messages yet'}</span></button>;})}
        {hasMore&&<button className="dw-btn-outline" disabled={moreBusy} onClick={()=>void loadMore()}>{moreBusy?'Loading...':'Load more conversations'}</button>}
      </div></aside>
      <div className="web-chat-thread">{current?<ChatThread visible={visible} key={current.id} chat={current} profile={profile} appointment={appointment} onBack={()=>setSelected(null)} action={action} gap={gap}/>:<div className="web-chat-welcome" role="status"><ChatIcon/><h3>Select a conversation</h3><p>Choose a patient conversation from the list to view messages.</p></div>}</div>
    </div>
  </section>;

}

export function ChatThread({ chat, profile, appointment, onBack, action, gap, visible=true }: { chat:Conversation; profile:Profile; appointment?:Appointment; onBack:()=>void; action:ClinicalAction; gap:(message:string)=>void;visible?:boolean }) {
  const latest=useResource<Message[]>(`chat/${chat.id}/messages`);
  const [thread,setThread]=useState<Message[]>([]);
  const [hasOlder,setHasOlder]=useState(true);
  const [loadingOlder,setLoadingOlder]=useState(false);
  const paging=useRef(false);
  const initialized=useRef(false);
  const feed=useRef<HTMLDivElement>(null);
  const threadRef=useRef(thread);threadRef.current=thread;
  const acknowledged=useRef(new Set<string>());
  const merge=(old:Message[],next:Message[])=>[...new Map([...old,...next].map(m=>[m.id,m])).values()].sort((a,b)=>compareMessageInstants(a.sent_at,b.sent_at)||a.id.localeCompare(b.id));
  const messages={...latest,data:thread,setData:setThread};
  useEffect(()=>{
    if(!latest.data)return;
    setThread(old=>merge(old,latest.data!));
    if(!initialized.current){setHasOlder(latest.data.length===50);initialized.current=true;}
  },[latest.data]);
  useEffect(()=>{
    const acknowledge=(event:string)=>post(`appointments/${chat.appointment_id}/trace-ack`,{event,conversation_id:chat.id});
    void acknowledge('CHAT_OPENED').catch(()=>{});
    void acknowledge('CONSULTATION_OPENED').catch(()=>{});
  },[chat.id,chat.appointment_id]);
  useEffect(()=>{
    if(!visible)return;
    let stopped=false;
    let lastRefreshFailed=false;
    const refresh=async()=>{
      if(paging.current||chatLocked(chat))return;
      paging.current=true;
      try {
        const known=threadRef.current;
        if(!known.length){const next=await api<Message[]>(`chat/${chat.id}/messages`);if(!stopped)setThread(old=>merge(old,next));}
        else {let after=known[known.length-1];for(let page=0;page<4&&!stopped;page++){
          const query=new URLSearchParams({after_time:after.sent_at,after_id:after.id});
          const next=await api<Message[]>(`chat/${chat.id}/messages?${query}`);
          if(stopped)break;
          setThread(old=>merge(old,next));
          if(next.length<50)break;
          after=next[next.length-1];
        }}
        lastRefreshFailed=false;
      }
      catch(e){if(!stopped)setError(errorText(e));lastRefreshFailed=true;}finally{paging.current=false;}
    };
    const stop=startScheduleSync(refresh,{window,document,online:()=>navigator.onLine,every:(cb,ms)=>window.setInterval(cb,ms),cancel:id=>window.clearInterval(id as number)},10000,{hasError:()=>lastRefreshFailed});
    return()=>{stopped=true;stop();};
  },[chat.id,visible]);
  useEffect(()=>{
    if(!feed.current || typeof IntersectionObserver==='undefined')return;
    const observer=new IntersectionObserver(entries=>{
      const ids=entries.filter(e=>e.isIntersecting).map(e=>(e.target as HTMLElement).dataset.messageId!).filter(id=>!acknowledged.current.has(id)).slice(0,50);
      if(!ids.length)return;
      ids.forEach(id=>acknowledged.current.add(id));
      void post(`appointments/${chat.appointment_id}/trace-ack`,{event:'CHAT_READ_ACKNOWLEDGED',conversation_id:chat.id,message_ids:ids}).catch(()=>{ids.forEach(id=>acknowledged.current.delete(id));});
    },{root:feed.current,threshold:0.1});
    feed.current.querySelectorAll('[data-incoming="true"]').forEach(element=>observer.observe(element));
    return()=>observer.disconnect();
  },[thread,chat.id,chat.appointment_id]);
  async function olderMessages(){
    if(loadingOlder||!hasOlder||!thread.length)return;
    setLoadingOlder(true);
    try {const first=thread[0];const query=new URLSearchParams({before_time:first.sent_at,before_id:first.id});const older=await api<Message[]>(`chat/${chat.id}/messages?${query}`);setThread(old=>merge(old,older));setHasOlder(older.length===50);}
    catch(e){setError(errorText(e));}finally{setLoadingOlder(false);}
  }
  const [content,setContent]=useState('');
  const [sending,setSending]=useState(false);
  const [error,setError]=useState('');
  const [tick,setTick]=useState(Date.now());
  const [serverLocked,setServerLocked]=useState(false);
  const submission=useRef<{content:string;key:string}|null>(null);
  const busyRef=useRef(false);
  const picker=useRef<HTMLInputElement>(null);
  const pendingImage=useRef<{body:{mime_type:string;base64_content:string};key:string}|null>(null);
  const [uploading,setUploading]=useState(false);
  const uploadBusy=useRef(false);
  async function uploadImage(file?:File) {
    if(locked||uploadBusy.current)return;
    uploadBusy.current=true;setUploading(true);setError('');
    const generation=runtimeGeneration();
    try {
      if(file){
        if(!['image/jpeg','image/png'].includes(file.type))throw new Error('Choose a JPEG or PNG clinical image.');
        const prepared=await compressRxPhoto(file,()=>generation===runtimeGeneration());
        const base64_content=await new Promise<string>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(',')[1]);reader.onerror=()=>reject(new Error('Unable to read image.'));reader.readAsDataURL(prepared);});
        pendingImage.current={body:{mime_type:prepared.type,base64_content},key:crypto.randomUUID()};
      }
      if(!pendingImage.current)return;
      if(generation!==runtimeGeneration())throw new DOMException('Session changed','AbortError');
      const message=await post<Message>(`chat/${chat.id}/attachments`,pendingImage.current.body,pendingImage.current.key);
      messages.setData(previous=>[...(previous??[]).filter(m=>m.id!==message.id),message]);pendingImage.current=null;
    }catch(e){setError(errorText(e));}finally{uploadBusy.current=false;setUploading(false);}
  }
  const locked=serverLocked || chatLocked(chat,tick);
  useEffect(()=>{const interval=setInterval(()=>setTick(Date.now()),10000);return()=>clearInterval(interval);},[]);
  async function send(value=content) {
    if(locked || busyRef.current || !value.trim()) return;
    busyRef.current=true;setSending(true);setError('');
    if(submission.current?.content!==value) submission.current={content:value,key:crypto.randomUUID()};
    try {
      const message=await post<Message>(`chat/${chat.id}/messages`,{content:value},submission.current.key);
      messages.setData(previous=>[...(previous??[]).filter(m=>m.id!==message.id),message]);setContent('');submission.current=null;
    } catch(e) { setError(errorText(e)); if(e instanceof Error && 'status' in e && (e.status===403 || e.status===409)) setServerLocked(true); }
    finally {busyRef.current=false;setSending(false);}
  }
  function clinical(which:'history'|'documents'|'call'|'rx') {if(appointment)action(which,appointment);else gap('No authorized appointment context is available for this conversation.');}
  return <>
    <header className="web-chat-header"><div className="web-action-row"><div className="dw-doc-avatar-img">{initials(chat.patient_display_name)}</div><div><h2 className="dw-section-title">{chat.patient_display_name}</h2>{appointment&&<p className="sheet-desc">{appointment.appointment_number} &middot; {date(appointment.start_time)} &middot; {time(appointment.start_time)}</p>}<p className="sheet-desc">{locked?'Read-only - Consultation window closed':`Follow-up ends ${date(chat.expires_at)} ? ${time(chat.expires_at)} (${Math.max(0,Math.ceil((Date.parse(chat.expires_at)-tick)/60000))} min remaining)`}</p></div><button className="dw-btn-mini" onClick={onBack}>All chats</button></div><div className="web-action-row" style={{marginTop:10}}>{[['history','Patient History'],['documents','Documents'],...(chat.modality==='VIDEO'&&!locked?[['call','Start video consultation']]:[]),...(!locked?[['rx','Write Rx']]:[])].map(([which,label])=><button className="dw-btn-outline-sm" key={which} disabled={!appointment} onClick={()=>clinical(which as 'history'|'documents'|'call'|'rx')}>{label}</button>)}<button className="dw-btn-outline-sm" onClick={()=>gap(`The server controls chat expiry. This conversation expires at ${time(chat.expires_at)}.`)}>24-hour window</button></div></header>
    <div ref={feed} className="dw-meet-chat-feed" id="dwChatMsgStream">{hasOlder&&thread.length>0&&<button className="dw-btn-mini" disabled={loadingOlder} onClick={()=>void olderMessages()}>{loadingOlder?'Loading history...':'Load earlier messages'}</button>}<State loading={messages.loading} error={messages.error||error} retry={()=>void messages.refresh()}/>{messages.data?.map(m=><div key={m.id} data-message-id={m.id} data-incoming={m.sender_id!==profile.user_id} className={`dw-chat-bubble-row ${messageSide(m,profile.user_id)}`} data-sender-side={messageSide(m,profile.user_id)}><div className={`dw-chat-avatar ${m.sender_id===profile.user_id?'doc':'pt'}`}>{initials(m.sender_id===profile.user_id?profile.full_name:chat.patient_display_name)}</div><div className="dw-chat-bubble-content"><div className="dw-chat-author-line"><span>{m.sender_id===profile.user_id?profile.full_name:chat.patient_display_name}</span><time>{time(m.sent_at)}</time></div>{m.content&&<p className="web-clinical-text">{m.content}</p>}{m.attachment_id&&<ClinicalAttachment conversation={chat.id} message={m.id} size={m.attachment?.size_bytes}/>}</div></div>)}{messages.data?.length===0&&<State empty="No messages yet"/>}<button className="dw-btn-mini" onClick={()=>void messages.refresh()}>Refresh messages</button></div>
    {locked&&<p className="web-gap" role="status">Consultation window closed. This conversation is read-only.</p>}
    {!locked&&<form className="dw-meet-chat-input-bar" onSubmit={e=>{e.preventDefault();void send();}}><input ref={picker} type="file" accept="image/jpeg,image/png" hidden aria-label="Clinical image file" onChange={e=>{const file=e.target.files?.[0];e.target.value='';if(file)void uploadImage(file);}}/><button className="clinical-attach-button" type="button" disabled={locked||uploading} aria-label="Attach clinical image" onClick={()=>picker.current?.click()}><PaperclipIcon/></button>{pendingImage.current&&!uploading&&<button type="button" className="dw-btn-mini" onClick={()=>void uploadImage()}>Retry image upload</button>}<div className="dw-meet-input-box"><textarea rows={1} id="dwChatInputField" aria-label="Chat message" value={content} disabled={locked||sending} placeholder={locked?'Conversation is read-only':'Message...'} maxLength={2000} onChange={e=>setContent(e.target.value)}/></div><button className="dw-meet-send-btn" type="submit" disabled={locked||sending||!content.trim()}>{sending?'Sending...':'Send'}</button></form>}
  </>;
}

function ChatIcon(){return <svg className="web-chat-empty-icon" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><path d="M21 11.5a8.4 8.4 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.4 8.4 0 0 1-3.8-.9L3 21l1.9-5.7a8.4 8.4 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6A8.4 8.4 0 0 1 12.5 3H13a8.5 8.5 0 0 1 8 8v.5Z"/></svg>;}
