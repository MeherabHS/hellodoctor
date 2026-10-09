'use client';
import { useEffect, useRef, useState } from 'react';
import { api, errorText, post, useResource } from '../lib/client';
import { Appointment, Conversation, Message, Profile, chatLocked } from '../lib/contracts';
import { State } from './Common';
import { ClinicalAction, date, initials, time } from './Queue';

export function Chat({ profile, appointments, selectedAppointment, action, gap }: { profile: Profile; appointments: Appointment[]; selectedAppointment?: string; action: ClinicalAction; gap: (message:string)=>void }) {
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
  useEffect(()=>{ if(selectedAppointment && list.data) { const found=list.data.find(c=>c.appointment_id===selectedAppointment); if(found) setSelected(found); } },[selectedAppointment,list.data]);
  async function loadMore() {
    if (!list.data || moreBusy) return;
    setMoreBusy(true);setMoreError('');
    try { const next=await api<Conversation[]>(`doctor/chats?limit=100&offset=${list.data.length}`); list.setData([...list.data,...next]);setHasMore(next.length===100); }
    catch(e) { setMoreError(errorText(e)); } finally { setMoreBusy(false); }
  }
  const filtered = (list.data??[]).filter(c=>c.patient_display_name.toLowerCase().includes(search.toLowerCase()) && (filter==='all' || filter==='active'&&!chatLocked(c,now) || filter==='locked'&&chatLocked(c,now)));
  const current=selected?(list.data?.find(c=>c.id===selected.id)??selected):null;
  const appointment=current?appointments.find(a=>a.id===current.appointment_id):undefined;
  return <section className="web-chat-page">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>Patient Chats</h2><p>Consultation conversations and follow-up messages</p></div><button className="dw-btn-outline" disabled={list.loading} onClick={()=>void list.refresh()}>Refresh inbox</button></header>
    <div className="web-chat-workspace" id="webChatWorkspace">
      <aside className="dw-section-card web-chat-list" aria-label="Patient conversations"><div className="dw-section-header"><h3 className="dw-section-title">Conversations</h3></div><div className="web-section-body">
        <input className="auth-input" aria-label="Search conversations" placeholder="Search patient conversations..." value={search} onChange={e=>setSearch(e.target.value)}/>
        <div className="dw-filter-pills">{[['all','All'],['active','Active'],['locked','Read-only']].map(([value,label])=><button key={value} className={`dw-filter-pill ${filter===value?'active':''}`} onClick={()=>setFilter(value)}>{label}</button>)}</div>
        <State loading={list.loading} error={list.error||moreError} retry={()=>void list.refresh()}/>
        {!list.loading&&!list.error&&!filtered.length&&<div className="web-chat-list-empty" role="status"><ChatIcon/><strong>{!search&&filter==='all'?'No conversations yet':'No matching conversations'}</strong><p>Consultation conversations will appear here.</p></div>}
        {filtered.map(c=>{const context=appointments.find(a=>a.id===c.appointment_id);return <button className="dw-pt-card web-conversation-card" key={c.id} aria-pressed={current?.id===c.id} data-conversation-id={c.id} onClick={()=>setSelected(c)}><div className="web-action-row"><div className="dw-doc-avatar-img">{initials(c.patient_display_name)}</div><strong>{c.patient_display_name}</strong></div><span className={`badge ${chatLocked(c,now)?'badge-slate':'badge-green'}`}>{chatLocked(c,now)?'Read-only':'Active'}</span><span className="sheet-desc">{context?<>{context.appointment_number} &middot; {date(context.start_time)}</>:'Consultation conversation'}</span><span className="sheet-desc">{c.last_message_at?<>Last message &middot; {date(c.last_message_at)} {time(c.last_message_at)}</>:'No messages yet'}</span></button>;})}
        {hasMore&&<button className="dw-btn-outline" disabled={moreBusy} onClick={()=>void loadMore()}>{moreBusy?'Loading...':'Load more conversations'}</button>}
      </div></aside>
      <div className="web-chat-thread">{current?<ChatThread key={current.id} chat={current} profile={profile} appointment={appointment} onBack={()=>setSelected(null)} action={action} gap={gap}/>:<div className="web-chat-welcome" role="status"><ChatIcon/><h3>Select a conversation</h3><p>Choose a patient conversation from the list to view messages.</p></div>}</div>
    </div>
  </section>;

}

export function ChatThread({ chat, profile, appointment, onBack, action, gap }: { chat:Conversation; profile:Profile; appointment?:Appointment; onBack:()=>void; action:ClinicalAction; gap:(message:string)=>void }) {
  const messages=useResource<Message[]>(`chat/${chat.id}/messages`);
  const [content,setContent]=useState('');
  const [sending,setSending]=useState(false);
  const [error,setError]=useState('');
  const [tick,setTick]=useState(Date.now());
  const [serverLocked,setServerLocked]=useState(false);
  const submission=useRef<{content:string;key:string}|null>(null);
  const busyRef=useRef(false);
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
    <header className="web-chat-header"><div className="web-action-row"><div className="dw-doc-avatar-img">{initials(chat.patient_display_name)}</div><div><h2 className="dw-section-title">{chat.patient_display_name}</h2>{appointment&&<p className="sheet-desc">{appointment.appointment_number} &middot; {date(appointment.start_time)} &middot; {time(appointment.start_time)}</p>}<p className="sheet-desc">{locked?'Read-only - Consultation window closed':`Follow-up ends ${date(chat.expires_at)} ? ${time(chat.expires_at)} (${Math.max(0,Math.ceil((Date.parse(chat.expires_at)-tick)/60000))} min remaining)`}</p></div><button className="dw-btn-mini" onClick={onBack}>All chats</button></div><div className="web-action-row" style={{marginTop:10}}>{[['history','Patient History'],['documents','Documents'],['call','Call Patient'],['rx','Write Rx']].map(([which,label])=><button className="dw-btn-outline-sm" key={which} disabled={!appointment} onClick={()=>clinical(which as 'history'|'documents'|'call'|'rx')}>{label}</button>)}<button className="dw-btn-outline-sm" onClick={()=>gap(`The server controls chat expiry. This conversation expires at ${time(chat.expires_at)}.`)}>24-hour window</button></div></header>
    <div className="dw-meet-chat-feed" id="dwChatMsgStream"><State loading={messages.loading} error={messages.error||error} retry={()=>void messages.refresh()}/>{messages.data?.map(m=><div key={m.id} className={`dw-chat-bubble-row ${m.sender_id===profile.user_id?'doctor':'patient'}`}><div className={`dw-chat-avatar ${m.sender_id===profile.user_id?'doc':'pt'}`}>{initials(m.sender_id===profile.user_id?profile.full_name:chat.patient_display_name)}</div><div className="dw-chat-bubble-content"><div className="dw-chat-author-line"><span>{m.sender_id===profile.user_id?profile.full_name:chat.patient_display_name}</span><time>{time(m.sent_at)}</time></div><p className="web-clinical-text">{m.content}</p></div></div>)}{messages.data?.length===0&&<State empty="No messages yet"/>}<button className="dw-btn-mini" onClick={()=>void messages.refresh()}>Refresh messages</button></div>
    <div className="web-chat-canned"><button className="dw-btn-mini" disabled={locked||sending} onClick={()=>void send('Take all medicines after meals with warm water.')}>Take after meals</button><button className="dw-btn-mini" disabled={locked||sending} onClick={()=>void send('Please recheck body temperature in 4 hours.')}>Check fever</button><button className="dw-btn-mini" onClick={()=>clinical('documents')}>View Documents</button><button className="dw-btn-mini" onClick={()=>clinical('history')}>Patient History</button><button className="dw-btn-mini" onClick={()=>clinical('rx')}>Upload/Send Rx</button></div>
    {locked&&<p className="web-gap" role="status">Consultation window closed. This conversation is read-only.</p>}
    <form className="dw-meet-chat-input-bar" onSubmit={e=>{e.preventDefault();void send();}}><button className="dw-btn-mini" type="button" disabled={locked||!appointment} aria-label="Attach prescription photos" onClick={()=>clinical('rx')}>Attach</button><div className="dw-meet-input-box"><input id="dwChatInputField" aria-label="Chat message" value={content} disabled={locked||sending} placeholder={locked?'Conversation is read-only':'Message...'} maxLength={2000} onChange={e=>setContent(e.target.value)}/></div><button className="dw-meet-send-btn" type="submit" disabled={locked||sending||!content.trim()}>{sending?'Sending...':'Send'}</button></form>
  </>;
}

function ChatIcon(){return <svg className="web-chat-empty-icon" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><path d="M21 11.5a8.4 8.4 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.4 8.4 0 0 1-3.8-.9L3 21l1.9-5.7a8.4 8.4 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6A8.4 8.4 0 0 1 12.5 3H13a8.5 8.5 0 0 1 8 8v.5Z"/></svg>;}
