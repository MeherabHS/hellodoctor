'use client';
import { useRef, useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { DoctorNotification } from '../lib/contracts';
import { State } from './Common';
import { date, time } from './Queue';

export function Notifications({onChanged,onOpen}:{onChanged:()=>void;onOpen:(notice:DoctorNotification)=>void}) {
  const inbox=useResource<DoctorNotification[]>('notifications?limit=25&offset=0');
  const [busy,setBusy]=useState('');const [error,setError]=useState('');const [more,setMore]=useState(true);const lock=useRef(false);
  async function mark(notice?:DoctorNotification,open=false){
    if(lock.current)return;lock.current=true;setBusy(notice?.id??'all');setError('');
    try{if(!notice||!notice.read_at)await api(notice?`notifications/${notice.id}/read`:'notifications/read-all',{method:'POST'});await inbox.refresh();setMore(true);onChanged();if(notice&&open)onOpen(notice);}
    catch(e){setError(errorText(e));}finally{lock.current=false;setBusy('');}
  }
  async function loadMore(){
    if(lock.current||!inbox.data)return;lock.current=true;setBusy('more');setError('');
    try{const next=await api<DoctorNotification[]>(`notifications?limit=25&offset=${inbox.data.length}`);inbox.setData([...inbox.data,...next]);setMore(next.length===25);}
    catch(e){setError(errorText(e));}finally{lock.current=false;setBusy('');}
  }
  return <section className="web-notification-panel" aria-label="Notification inbox">
    <div className="web-action-row"><button className="dw-btn-outline" disabled={!!busy} onClick={()=>{setMore(true);void inbox.refresh();onChanged();}}>Refresh notifications</button><button className="dw-btn-outline" disabled={!!busy||!inbox.data?.some(n=>!n.read_at)} onClick={()=>void mark()}>Mark all read</button></div>
    <State loading={inbox.loading} error={inbox.error||error} empty={!inbox.loading&&!inbox.error&&!inbox.data?.length?'No notifications yet':undefined} retry={()=>void inbox.refresh()}/>
    {inbox.data?.map(notice=><article className={`web-notification-card ${notice.read_at?'':'unread'}`} key={notice.id} data-notification-id={notice.id}>
      <div className="web-action-row"><strong>{notice.title}</strong>{!notice.read_at&&<span className="badge badge-green">Unread</span>}</div>
      <p className="web-clinical-text">{notice.body}</p><time className="sheet-desc">{date(notice.created_at)} &middot; {time(notice.created_at)}</time>
      <div className="web-action-row">{!notice.read_at&&<button className="dw-btn-mini" disabled={!!busy} onClick={()=>void mark(notice)}>{busy===notice.id?'Marking...':'Mark read'}</button>}{['DAILY_CONSULTATION_SUMMARY','CONSULTATION_REMINDER_15M','EARNING_CREDITED','PAYOUT_PAID','REGISTRATION_VERIFIED','REGISTRATION_REJECTED'].includes(notice.notification_type)&&<button className="dw-btn-mini" disabled={!!busy} onClick={()=>void mark(notice,true)}>View details</button>}</div>
    </article>)}
    {more&&(inbox.data?.length??0)>=25&&<button className="dw-btn-outline" disabled={!!busy} onClick={()=>void loadMore()}>{busy==='more'?'Loading...':'Load more notifications'}</button>}
  </section>;
}
