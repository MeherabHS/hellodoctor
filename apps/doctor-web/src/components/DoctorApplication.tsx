'use client';
import { startScheduleSync } from '../lib/schedule-sync';
import { Articles } from './Articles';
import { ConsultationHistory } from './ConsultationHistory';
import { isProfessionallyVerified, professionalVerificationLabel } from '../lib/professionalVerification';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, clearRuntime, diagnostic, errorText, request, runtimeGeneration, useResource, setIncidentScreen } from '../lib/client';
import { Appointment, DoctorNotification, Profile, RecordDto, RxPage } from '../lib/contracts';
import { Prototype, hasClass, id } from './Prototype';
import { Modal, State } from './Common';
import { Auth } from './Auth';
import { ClinicalAction, initials, useAppointments } from './Queue';
import { Chat } from './Chat';
import { Rx } from './Rx';
import { Schedule } from './Schedule';
import { PayoutSettings, Wallet } from './Wallet';
import { Statement } from './Statement';
import { Qna } from './Qna';
import { Completion, DocumentViewer, Documents, History, Receipt, SlotForm, Summary } from './Clinical';
import { Dashboard, DesktopAppointments, Patients, Telehealth } from './Desktop';
import { Notifications } from './Notifications';
import { AccountMenu } from './AccountMenu';
import { Settings, SettingsSection } from './Settings';

type Page='queue'|'chat'|'rx'|'schedule'|'wallet'|'qna'|'dashboard'|'appointments'|'patients'|'telehealth'|'settings'|'articles'|'consultation-history';
type Dialog='summary'|'history'|'documents'|'completion'|'slot'|'statement'|'payout'|'receipt'|'notifications'|'gap'|null;
class Boundary extends React.Component<{children:React.ReactNode},{failed:boolean}> {
  state={failed:false};
  static getDerivedStateFromError(){return {failed:true};}
  componentDidCatch(error:Error){diagnostic('render-error',error);}
  render(){return this.state.failed?<main className="web-auth"><State error="This screen could not be rendered. Reload to restore your session."/><button className="sheet-btn primary" onClick={()=>location.reload()}>Reload</button></main>:this.props.children;}
}
export function DoctorApplication({initialPage='dashboard'}:{initialPage?:'dashboard'|'chat'}){return <Boundary><Application initialPage={initialPage}/></Boundary>;}
const pages:Page[]=['chat','rx','schedule','wallet','qna','dashboard','appointments','patients','telehealth','settings','articles','consultation-history'];
function locationPage():Page {const stored=window.history.state?.doctorWebPage;return pages.includes(stored)?stored:window.location.pathname==='/doctor/chats'?'chat':'dashboard';}
function Application({initialPage}:{initialPage:'dashboard'|'chat'}){
  const [profile,setProfile]=useState<Profile|null>(null);const [restoring,setRestoring]=useState(true);const [authError,setAuthError]=useState('');const [page,setPage]=useState<Page>(initialPage);const [search,setSearch]=useState('');const [active,setActive]=useState<Appointment|null>(null);const [dialog,setDialog]=useState<Dialog>(null);const [gapMessage,setGap]=useState('');const [drawerOpen,setDrawerOpen]=useState(false);const [duty,setDuty]=useState<{value?:boolean;busy:boolean;error:string}>({busy:false,error:''});const [rxByAppointment,setRx]=useState<Record<string,RxPage[]>>({});const [receipt,setReceipt]=useState<RecordDto|null>(null);const [document,setDocument]=useState<{appointmentId:string;document:RecordDto}|null>(null);const [offline,setOffline]=useState(false);const [suspended,setSuspended]=useState(false);const [revision,setRevision]=useState(0);const channel=useRef<BroadcastChannel|null>(null);const pagesRef=useRef(rxByAppointment);pagesRef.current=rxByAppointment;const epoch=useRef(0);const dutyLock=useRef(false);
  const [scheduleDate,setScheduleDate]=useState('');
  const [settingsSection,setSettingsSection]=useState<SettingsSection>('profile');
  const enabled=isProfessionallyVerified(profile)&&!suspended&&!offline;
  const appointmentPage=['dashboard','appointments','rx','patients','telehealth','schedule','chat'].includes(page);
  const queue=useAppointments(enabled&&appointmentPage,page==='schedule'?'all':'upcoming', ['dashboard','appointments'].includes(page)&&dialog===null&&document===null);
  const unread=useResource<{unread_count:number}>(enabled?'notifications/unread-count':null);
  useEffect(()=>{if(!enabled)return;return startScheduleSync(unread.refresh,{window,document:window.document,online:()=>navigator.onLine,every:(cb,ms)=>window.setInterval(cb,ms),cancel:id=>window.clearInterval(id as number)},60000);},[enabled,unread.refresh]);
  const wipe=useCallback(()=>{epoch.current++;clearRuntime();setProfile(null);setActive(null);setDialog(null);setDocument(null);setPage(window.location.pathname==='/doctor/chats'?'chat':'dashboard');setSearch('');setSettingsSection('profile');Object.values(pagesRef.current).flat().forEach(page=>URL.revokeObjectURL(page.preview));setRx({});setDuty({busy:false,error:''});},[]);
  useEffect(()=>{
    if(!window.history.state?.doctorWebPage)window.history.replaceState({...window.history.state,doctorWebPage:initialPage},'',window.location.href);
    setPage(locationPage());
    const back=()=>{setPage(locationPage());setActive(null);setDialog(null);setDocument(null);setDrawerOpen(false);};
    window.addEventListener('popstate',back);return()=>window.removeEventListener('popstate',back);
  },[initialPage]);
  const restore=useCallback(async()=>{const version=++epoch.current;setRestoring(true);setAuthError('');try{const data=await api<Profile>('doctor/profile');if(version!==epoch.current)return;setProfile(data);const status=await api<{is_on_duty:boolean}>('doctor/duty');if(version!==epoch.current)return;setDuty({value:status.is_on_duty,busy:false,error:''});setSuspended(false);}catch(e){if(version!==epoch.current)return;if(e instanceof Error&&'status'in e&&e.status===401){wipe();setRestoring(false);}else{setAuthError(errorText(e));setProfile(null);}}finally{if(version===epoch.current)setRestoring(false);}},[wipe]);
  useEffect(()=>{void restore();const expired=()=>{wipe();setRestoring(false);setAuthError('Your session ended. Please sign in again.');};window.addEventListener('doctor-session-expired',expired);const browserError=(event:ErrorEvent)=>diagnostic('browser-error',event.error),rejection=(event:PromiseRejectionEvent)=>diagnostic('promise-rejection',event.reason);window.addEventListener('error',browserError);window.addEventListener('unhandledrejection',rejection);channel.current=new BroadcastChannel('hellodoctor-doctor-session');channel.current.onmessage=()=>{wipe();setRestoring(false);setAuthError('The session changed in another tab. Sign in to continue.');};const onOffline=()=>{setOffline(true);setSuspended(true);clearRuntime();setDialog(null);setDocument(null);},onOnline=()=>{setOffline(false);setRevision(r=>r+1);void restore();};window.addEventListener('offline',onOffline);window.addEventListener('online',onOnline);const visibility=()=>{if(window.document.hidden){setSuspended(true);setDialog(null);setDocument(null);clearRuntime();}else{setRevision(r=>r+1);void restore();}};window.document.addEventListener('visibilitychange',visibility);return()=>{window.removeEventListener('doctor-session-expired',expired);window.removeEventListener('error',browserError);window.removeEventListener('unhandledrejection',rejection);window.removeEventListener('offline',onOffline);window.removeEventListener('online',onOnline);window.document.removeEventListener('visibilitychange',visibility);channel.current?.close();};},[restore,wipe]);
  const close=useCallback(()=>{setDialog(null);setDocument(null);},[]);
  useEffect(()=>{setIncidentScreen(({chat:'Chat',rx:'SmartRx',schedule:'Schedule',wallet:'Wallet',qna:'Qna',telehealth:'Telehealth',settings:'Settings',articles:'Settings',patients:'Patients',appointments:'Queue',queue:'Queue',dashboard:'Application'} as Record<Page,string>)[page]);},[page]);
  useEffect(()=>{
    if(!drawerOpen)return;
    const rail=window.document.querySelector<HTMLElement>('#doctorWebSidebar');
    const previous=window.document.activeElement as HTMLElement|null;
    rail?.querySelector<HTMLButtonElement>('button')?.focus();
    const key=(event:KeyboardEvent)=>{
      if(event.key==='Escape'){setDrawerOpen(false);return;}
      if(event.key!=='Tab'||!rail)return;
      const buttons=[...rail.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
      const first=buttons[0],last=buttons[buttons.length-1];
      if(event.shiftKey&&window.document.activeElement===first){event.preventDefault();last?.focus();}
      else if(!event.shiftKey&&window.document.activeElement===last){event.preventDefault();first?.focus();}
    };
    window.document.addEventListener('keydown',key);
    return()=>{window.document.removeEventListener('keydown',key);previous?.focus();};
  },[drawerOpen]);
  function gap(message:string){setGap(message);setDialog('gap');}
  function go(target:Page){const destination=target==='queue'?'appointments':target;if(destination!==page)window.history.pushState({doctorWebPage:destination},'',destination==='chat'?'/doctor/chats':'/');setDrawerOpen(false);setPage(destination);setDialog(null);setDocument(null);}
  const action:ClinicalAction=(name,a)=>{setActive(a);if(name==='chat')go('chat');else if(name==='rx')go('rx');else if(name==='call'){go('telehealth');}else setDialog(name);};
  function openNotice(notice:DoctorNotification){
    if(notice.notification_type.startsWith('REGISTRATION_')){setSettingsSection('professional');go('settings');}
    else if(['EARNING_CREDITED','PAYOUT_PAID'].includes(notice.notification_type))go('wallet');
    else if(notice.notification_type==='CONSULTATION_REMINDER_15M'){const appointment=queue.data.find(a=>a.id===notice.related_entity_id);if(appointment)action(appointment.modality==='CHAT'?'chat':'call',appointment);else go('appointments');}
    else if(notice.related_entity_type==='CHAT'){const appointment=queue.data.find(a=>a.id===notice.metadata?.appointment_id||a.conversation_id===notice.related_entity_id);if(appointment)action('chat',appointment);else go('chat');}
    else if(notice.notification_type==='ARTICLE_STATUS_CHANGED')go('articles');
    else go('appointments');
  }
  async function toggleDuty(){if(dutyLock.current||duty.value===undefined)return;dutyLock.current=true;setDuty(previous=>({...previous,busy:true,error:''}));try{const data=await api<{is_on_duty:boolean}>('doctor/duty',{method:'PUT',body:JSON.stringify({is_on_duty:!duty.value})});setDuty({value:data.is_on_duty,busy:false,error:''});}catch(e){setDuty(previous=>({...previous,busy:false,error:errorText(e)}));}finally{dutyLock.current=false;}}
  async function logout(){channel.current?.postMessage({event:'logout'});try{await request('/api/auth/logout',{method:'POST'});wipe();setRestoring(false);}catch(e){wipe();setRestoring(false);setAuthError(errorText(e));}}
  function boundary(){clearRuntime();channel.current?.postMessage({event:'account-boundary'});}
  if(restoring)return <main className="web-auth"><State loading/></main>;
  if(!profile)return <><Auth onSignedIn={()=>void restore()} onBoundary={boundary}/>{authError&&<div className="web-auth"><State error={authError}/><button className="sheet-btn secondary" onClick={()=>void restore()}>Restore session</button></div>}</>;
  if(!isProfessionallyVerified(profile))return <main className="web-auth"><h1 className="sheet-title">{professionalVerificationLabel(profile)}</h1><p className="sheet-desc">{profile.full_name} · KMPDC Registration: {profile.license_number}</p><p className="sheet-desc">Kenya Medical Practitioners and Dentists Council · Kenya</p><p className="sheet-desc">{profile.professional_verification_status==='REJECTED'?'HelloDoctor rejected your submitted professional registration. Clinical access is unavailable.':'Your submitted professional registration awaits manual HelloDoctor review. Clinical access is unavailable.'}</p>{profile.professional_verification_note&&<p className="sheet-desc">Review note: {profile.professional_verification_note}</p>}<button className="sheet-btn primary" onClick={()=>void restore()}>Refresh verification status</button><button className="sheet-btn secondary" onClick={()=>void logout()}>Log out</button></main>;
  const renderGeneration=runtimeGeneration();
  const patientQueue=queue.data.filter(a=>a.queue_actionable===true);
  const chosen=active??queue.data.find(a=>a.queue_actionable===true)??queue.data[0]??null;
  function showDocument(appointmentId:string,doc:RecordDto){setDocument({appointmentId,document:doc});}
  let view:React.ReactNode;
  if(offline||suspended)view=<State error={offline?'Network unavailable. Reconnect to fetch authorized data.':'Revalidating your session…'} retry={()=>void restore()}/>;

  else if(page==='chat')view=<Chat profile={profile} appointments={queue.data} selectedAppointment={active?.id} focusAppointment={active??undefined} visible={dialog===null&&document===null&&!drawerOpen} action={action} gap={gap}/>;
  else if(page==='rx')view=<Rx profile={profile} appointments={queue.data} appointment={chosen} pages={chosen?rxByAppointment[chosen.id]??[]:[]} setPages={pages=>{if(chosen&&renderGeneration===runtimeGeneration())setRx(previous=>({...previous,[chosen.id]:pages}));}} select={setActive} action={action} complete={()=>setDialog('completion')}/>;
  else if(page==='schedule')view=<Schedule key={revision} appointments={queue.data} action={action} addSlot={()=>setDialog('slot')} gap={gap} initialDate={scheduleDate}/>;
  else if(page==='wallet')view=<Wallet statement={()=>setDialog('statement')} payout={()=>setDialog('payout')} receipt={row=>{setReceipt(row);setDialog('receipt');}}/>;
  else if(page==='qna')view=<Qna profile={profile} mobile={false} onBack={()=>go('dashboard')}/>;
  else if(page==='appointments')view=<DesktopAppointments queue={queue} action={action} search={search} setSearch={setSearch} addSlot={()=>setDialog('slot')} schedule={()=>go('schedule')} gap={gap}/>;
  else if(page==='dashboard')view=<Dashboard profile={profile} queue={queue} onAppointments={()=>go('appointments')} earnings={()=>go('wallet')} qna={()=>go('qna')} action={action}/>;
  else if(page==='consultation-history')view=<ConsultationHistory action={action}/>;
  else if(page==='articles')view=<Articles specialty={profile.primary_specialty}/>;
  else if(page==='settings')view=<Settings profile={profile} section={settingsSection} onSection={setSettingsSection} onUpdated={setProfile} duty={duty} onDuty={()=>void toggleDuty()} onWallet={()=>go('wallet')} onLogout={()=>void logout()}/>;
  else if(page==='patients')view=<Patients queue={queue} action={action} gap={gap}/>;
  else view=<Telehealth appointment={chosen} profile={profile} action={action} onChat={()=>go('chat')} complete={()=>chosen?setDialog('completion'):gap('Select a consultation before completing it.')} gap={gap}/>;
  if(appointmentPage&&page!=='chat'&&queue.hasMore)view=<>{view}<button className="sheet-btn secondary" disabled={queue.moreBusy} onClick={()=>void queue.loadMore()}>{queue.moreBusy?'Loading consultations...':'Load more consultations'}</button></>;
  return <><Prototype name="doctorWebShell" action={handler=>{
    if(handler.includes('switchDoctorWebPage'))go((handler.match(/'([^']+)'/)?.[1]??'queue') as Page);
    else if(handler.includes('handleDoctorWebGlobalSearch'))go('appointments');
    else if(handler.includes('Settings')){setSettingsSection('profile');go('settings');}
    else if(handler.includes('Shift'))go('schedule');
    else setDialog('notifications');
  }} override={n=>{
    if(id(n)==='doctorWebShell')return {props:{style:{display:'flex'},className:`doctor-web-shell ${drawerOpen?'web-sidebar-open':''}`}};
    if(id(n)==='docWebOfflineBanner')return {props:{style:{display:offline?'flex':'none'}},children:'Network unavailable · Mutations require connectivity'};
    if(hasClass(n,'dw-topbar-brand'))return {children:<><button className="dw-icon-btn web-menu-toggle" aria-label={drawerOpen?'Close navigation':'Open navigation'} aria-expanded={drawerOpen} aria-controls="doctorWebSidebar" onClick={()=>setDrawerOpen(!drawerOpen)}><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 6h18M3 12h18M3 18h18"/></svg></button><span className="brand-icon"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M12 2v20M2 12h20"/></svg></span><span>HelloDoctor Web</span></>};
    if(hasClass(n,'dw-sidebar'))return {props:{id:'doctorWebSidebar','aria-label':'Doctor navigation'}};
    if(hasClass(n,'dw-topbar-actions'))return {children:<>
      <button className="dw-icon-btn" aria-label="Write an article" title="Write an article" onClick={()=>go('articles')}><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="M13 5H4v16h16v-9M15 3l6 6M11 13l1-4 7-7 4 4-7 7-5 1Z"/></svg></button>
      <button className="dw-icon-btn" aria-label="Open consultation schedule" title="Consultation schedule" onClick={()=>go('schedule')}><span className="web-icon web-icon-calendar" aria-hidden="true"/></button>
      <button className="dw-icon-btn" aria-label="Open notifications" title={unread.error?'Notifications unavailable':`${unread.data?.unread_count??0} unread notifications`} onClick={()=>setDialog('notifications')}><span className="web-icon web-icon-bell" aria-hidden="true"/>{!!unread.data?.unread_count&&<span className="web-unread-badge" aria-label={`${unread.data.unread_count} unread`}>{unread.data.unread_count>99?'99+':unread.data.unread_count}</span>}</button>
      <AccountMenu profile={profile} onProfile={()=>{setSettingsSection('professional');go('settings');}} onSettings={()=>{setSettingsSection('profile');go('settings');}} onNotifications={()=>setDialog('notifications')} onLogout={()=>void logout()}/>
    </>};
    if(hasClass(n,'dw-user-avatar')||hasClass(n,'dw-doc-avatar-img'))return {children:initials(profile.full_name)};
    if(hasClass(n,'dw-user-name')||hasClass(n,'dw-doc-name-text'))return {children:profile.full_name};
    if(hasClass(n,'dw-notif-dot'))return {omit:true};
    if(hasClass(n,'dw-nav-badge'))return id(n)==='dwQnaBadgeCount'||!patientQueue.length?{omit:true}:{children:patientQueue.length};
    if(id(n)==='dwGlobalSearchInput')return {props:{placeholder:'Search owned appointments…',value:search,onChange:(event:React.ChangeEvent<HTMLInputElement>)=>{setSearch(event.target.value);go('appointments');}}};
    if(hasClass(n,'dw-nav-item')){
      const target=(n.props['data-page']??'settings') as Page;
      const selected=target===page||target==='telehealth'&&page==='rx';
      const extra=target==='appointments'?<button className={`dw-nav-item ${page==='schedule'?'active':''}`} data-page="schedule" aria-current={page==='schedule'?'page':undefined} onClick={()=>go('schedule')}><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M16 3v4M8 3v4M3 11h18M12 14v4M10 16h4"/></svg><span>Schedule &amp; Slots</span></button>:target==='telehealth'?<button className={`dw-nav-item ${page==='chat'?'active':''}`} data-page="chat" aria-current={page==='chat'?'page':undefined} onClick={()=>{setActive(null);go('chat');}}><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="M21 11.5a8.4 8.4 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.4 8.4 0 0 1-3.8-.9L3 21l1.9-5.7a8.4 8.4 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6A8.4 8.4 0 0 1 12.5 3H13a8.5 8.5 0 0 1 8 8v.5Z"/></svg><span>Patient Chats</span></button>:target==='settings'?<button className={`dw-nav-item ${page==='wallet'?'active':''}`} data-page="wallet" aria-current={page==='wallet'?'page':undefined} onClick={()=>go('wallet')}><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><rect x="2" y="5" width="20" height="15" rx="2"/><path d="M2 10h20M16 15h3"/></svg><span>Earnings &amp; Wallet</span></button>:undefined;
      return {tag:'button',before:<>{extra}{target==='appointments'&&<button className={`dw-nav-item ${page==='consultation-history'?'active':''}`} aria-current={page==='consultation-history'?'page':undefined} onClick={()=>go('consultation-history')}>Consultation History</button>}{target==='settings'&&<button className={`dw-nav-item ${page==='articles'?'active':''}`} aria-current={page==='articles'?'page':undefined} onClick={()=>go('articles')}>My Articles</button>}</>,props:{className:`dw-nav-item ${selected?'active':''}`,'data-page':target,onClick:()=>go(target),'aria-current':selected?'page':undefined}};
    }
    if(n.tag==='div'&&n.children.some(child=>typeof child==='object'&&child.children.some(text=>typeof text==='string'&&text.includes('Patient Queue ('))))return {omit:true};
    if(id(n)==='dwMainContentArea')return {children:<>{drawerOpen&&<button className="web-sidebar-backdrop" aria-label="Close navigation" onClick={()=>setDrawerOpen(false)}/>}<main className="web-primary-view" key={`${page}-${revision}`} data-web-screen={page}>{page==='rx'&&<div className="web-workflow-back"><button className="dw-btn-outline-sm" onClick={()=>go('telehealth')}>&larr; Consultation workspace</button></div>}{view}</main></>};
    if(id(n)==='dwToastContainer')return {omit:true};
    return undefined;
  }}/>{dialog&&<Modal title={({summary:'Consultation Detail',history:'Patient Case History',documents:'Consultation Documents',completion:'Complete Consultation',slot:'Create Consultation Slots',statement:'Monthly Earnings Statement',payout:'Payout Settings',receipt:'Consultation Fee Audit',notifications:'Notifications',gap:'Service Availability'} as Record<string,string>)[dialog]} onClose={close} wide={['statement','history','documents','completion','notifications'].includes(dialog)} bare={['statement','slot','receipt'].includes(dialog)}>
    {dialog==='notifications'&&<Notifications onChanged={()=>void unread.refresh()} onOpen={openNotice}/>}
    {dialog==='summary'&&chosen&&<Summary appointment={chosen} history={()=>setDialog('history')} documents={()=>setDialog('documents')} complete={()=>setDialog('completion')}/>}
    {dialog==='history'&&chosen&&<History appointment={chosen} onDocument={showDocument}/>}
    {dialog==='documents'&&chosen&&<Documents appointment={chosen} onDocument={showDocument} onClose={close}/>}
    {dialog==='completion'&&chosen&&<Completion appointment={chosen} onCompleted={()=>void queue.refresh()}/>}
    {dialog==='slot'&&<SlotForm onClose={close} onCreated={date=>{setScheduleDate(date);setPage('schedule');setRevision(r=>r+1);void queue.refresh();}}/>}
    {dialog==='statement'&&<Statement profile={profile} onClose={close}/>}
    {dialog==='payout'&&<PayoutSettings/>}
    {dialog==='receipt'&&receipt&&<Receipt row={receipt} onClose={close}/>}
    {dialog==='gap'&&<p className="web-gap">{gapMessage}</p>}
  </Modal>}{document&&chosen&&<DocumentViewer current={chosen.id} appointmentId={document.appointmentId} document={document.document} onClose={()=>setDocument(null)}/>}</>;
}
