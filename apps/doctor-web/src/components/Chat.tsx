'use client';
import { startScheduleSync } from '../lib/schedule-sync';
import { ClinicalAttachment } from './ClinicalAttachment';
import { compressRxPhoto } from '../lib/rx-compression';
import { useEffect, useRef, useState } from 'react';
import { api, errorText, post, useResource, runtimeGeneration } from '../lib/client';
import { Appointment, Conversation, Message, Profile, chatLocked, messageSide, consultationContext, compareMessageInstants } from '../lib/contracts';
import { Avatar, Badge, PageHeader, State, demographics } from './Common';
import { Icon } from './Icons';
import { ClinicalAction, date, time } from './Queue';

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
  return <>
    <PageHeader title="Patient chats" subtitle="Consultation conversations and follow-up messages. Each chat closes when its consultation window ends.">
      <button className="am-btn ghost" disabled={list.loading} onClick={()=>void list.refresh()}><Icon name="refresh" size={16}/>Refresh</button>
    </PageHeader>
    <div className={`am-chat${current?' has-thread':''}`} id="webChatWorkspace">
      <section className="am-card am-chat-list" aria-label="Patient conversations">
        <div className="am-card-body am-stack" style={{gap:12,paddingBottom:10}}>
          <label className="am-search" style={{maxWidth:'none'}}><Icon name="search" size={16}/><span className="am-sr">Search conversations</span><input value={search} placeholder="Search by patient name" onChange={e=>setSearch(e.target.value)}/></label>
          <div className="am-tabs" aria-label="Conversation filter">{[['all','All'],['active','Active'],['locked','Read-only']].map(([value,label])=><button key={value} className="am-tab" aria-pressed={filter===value} onClick={()=>setFilter(value)}>{label}<span className="am-tab-count">{(list.data??[]).filter(c=>value==='all'||(value==='active')!==chatLocked(c,now)).length}</span></button>)}</div>
        </div>
        <div className="am-scroll">
          <State compact loading={list.loading} error={list.error||moreError} retry={()=>void list.refresh()}/>
          {!list.loading&&!list.error&&!filtered.length&&<State compact icon="chat" empty={!search&&filter==='all'?'No conversations yet':'No matching conversations'} hint="Consultation chats appear here once a patient books you."/>}
          {filtered.map(c=>{const context=appointments.find(a=>a.id===c.appointment_id);const locked=chatLocked(c,now);return <button className="am-convo" key={c.id} aria-pressed={current?.id===c.id} data-conversation-id={c.id} onClick={()=>setSelected(c)}>
            <Avatar name={c.patient_display_name}/>
            <span className="am-grow" style={{minWidth:0}}>
              <span className="am-row between" style={{flexWrap:'nowrap'}}><strong className="am-strong" style={{overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}}>{c.patient_display_name}</strong><span className="am-faint" style={{flexShrink:0}}>{c.last_message_at?(dayKeyOf(c.last_message_at)===dayKeyOf(new Date(now).toISOString())?time(c.last_message_at):date(c.last_message_at)):''}</span></span>
              <span className="am-faint" style={{display:'block'}}>{context?`${context.appointment_number} · ${date(context.start_time)}`:c.modality==='VIDEO'?'Video consultation follow-up':'Chat consultation'}</span>
              <span className="am-row" style={{marginTop:6,gap:6}}><Badge tone={locked?'slate':'green'} dot>{locked?'Read-only':'Active'}</Badge>{!c.last_message_at&&<span className="am-faint">No messages yet</span>}</span>
            </span>
          </button>;})}
          {hasMore&&<div className="am-row" style={{justifyContent:'center',padding:10}}><button className="am-btn ghost sm" disabled={moreBusy} onClick={()=>void loadMore()}>{moreBusy?'Loading…':'Load more'}</button></div>}
        </div>
      </section>
      <section className="am-card am-thread">{current?<ChatThread visible={visible} key={current.id} chat={current} profile={profile} appointment={appointment} onBack={()=>setSelected(null)} action={action} gap={gap}/>:<div style={{margin:'auto'}}><State icon="chat" empty="Select a conversation" hint="Choose a patient on the left to read and reply."/></div>}</section>
    </div>
  </>;
}

export function ChatThread({ chat, profile, appointment, onBack, action, gap, visible=true, compact=false }: { chat:Conversation; profile:Profile; appointment?:Appointment; onBack:()=>void; action:ClinicalAction; gap:(message:string)=>void;visible?:boolean;compact?:boolean }) {
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
  function clinical(which:'history'|'documents'|'call'|'rx') {if(appointment)action(which,appointment);else gap('This conversation is not linked to a consultation you can open right now.');}
  const minutesLeft=Math.max(0,Math.ceil((Date.parse(chat.expires_at)-tick)/60000));
  return <>
    <header className="am-thread-head">
      <div className="am-row between" style={{flexWrap:'nowrap'}}>
        <div className="am-person" style={{minWidth:0}}>{!compact&&<button className="am-btn ghost sm" aria-label="All chats" onClick={onBack}><Icon name="arrowLeft" size={15}/></button>}<Avatar name={chat.patient_display_name}/><div style={{minWidth:0}}><strong>{chat.patient_display_name}</strong><span>{appointment?`${[demographics(appointment),appointment.appointment_number].filter(Boolean).join(' · ')}`:'Consultation conversation'}</span></div></div>
        <Badge tone={locked?'slate':'green'} dot>{locked?'Read-only':minutesLeft>90?`Open until ${time(chat.expires_at)}`:`${minutesLeft} min left`}</Badge>
      </div>
      {!compact&&<div className="am-row" style={{marginTop:12,gap:8}}>
        <button className="am-btn ghost sm" disabled={!appointment} onClick={()=>clinical('history')}><Icon name="history" size={15}/>History</button>
        <button className="am-btn ghost sm" disabled={!appointment} onClick={()=>clinical('documents')}><Icon name="file" size={15}/>Documents</button>
        {chat.modality==='VIDEO'&&!locked&&<button className="am-btn ghost sm" disabled={!appointment} onClick={()=>clinical('call')}><Icon name="video" size={15}/>Consultation room</button>}
        {!locked&&<button className="am-btn soft sm" disabled={!appointment} onClick={()=>clinical('rx')}><Icon name="rx" size={15}/>Prescription</button>}
      </div>}
    </header>
    <div ref={feed} className="am-feed" id="dwChatMsgStream">
      {hasOlder&&thread.length>0&&<button className="am-btn ghost sm" style={{alignSelf:'center'}} disabled={loadingOlder} onClick={()=>void olderMessages()}>{loadingOlder?'Loading…':'Load earlier messages'}</button>}
      <State compact loading={messages.loading&&!thread.length} error={messages.error||error} retry={()=>void messages.refresh()}/>
      {messages.data?.map(m=>{const side=messageSide(m,profile.user_id);return <div key={m.id} data-message-id={m.id} data-incoming={m.sender_id!==profile.user_id} className={`am-bubble-row ${side}`} data-sender-side={side}>
        <Avatar name={side==='doctor'?profile.full_name:chat.patient_display_name} size="sm"/>
        <div className="am-bubble">{m.content&&<p className="am-pre">{m.content}</p>}{m.attachment_id&&<ClinicalAttachment conversation={chat.id} message={m.id} size={m.attachment?.size_bytes}/>}<time>{date(m.sent_at)} · {time(m.sent_at)}</time></div>
      </div>;})}
      {messages.data?.length===0&&<State compact icon="chat" empty="No messages yet" hint={locked?undefined:'Say hello to start the conversation.'}/>}
    </div>
    {locked?<p className="am-callout" role="status" style={{margin:14}}><Icon name="lock"/>This consultation window has closed, so the conversation is read-only.</p>
    :<form className="am-composer" onSubmit={e=>{e.preventDefault();void send();}}>
      <input ref={picker} type="file" accept="image/jpeg,image/png" hidden aria-label="Clinical image file" onChange={e=>{const file=e.target.files?.[0];e.target.value='';if(file)void uploadImage(file);}}/>
      <button className="am-icon-btn" type="button" disabled={locked||uploading} aria-label="Attach clinical image" title="Attach image" onClick={()=>picker.current?.click()}><Icon name="paperclip"/></button>
      {pendingImage.current&&!uploading&&<button type="button" className="am-btn ghost sm" onClick={()=>void uploadImage()}>Retry image</button>}
      <textarea className="am-textarea" rows={1} id="dwChatInputField" aria-label="Chat message" value={content} disabled={locked||sending} placeholder={uploading?'Uploading image…':'Write a message…'} maxLength={2000} onChange={e=>setContent(e.target.value)} onKeyDown={e=>{if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();void send();}}}/>
      <button className="am-btn primary" type="submit" disabled={locked||sending||!content.trim()} aria-label="Send message"><Icon name="send" size={16}/>{sending?'Sending…':'Send'}</button>
    </form>}
  </>;
}
function dayKeyOf(value:string){return new Intl.DateTimeFormat('en-CA',{timeZone:'Africa/Nairobi',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date(value));}
