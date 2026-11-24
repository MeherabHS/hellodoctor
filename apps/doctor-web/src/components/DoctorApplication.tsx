'use client';
import { startScheduleSync } from '../lib/schedule-sync';
import { restoresSilently, type RestoreTrigger } from '../lib/sessionRestore';
import { isProfessionallyVerified } from '../lib/professionalVerification';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, clearRuntime, diagnostic, errorText, request, runtimeGeneration, useResource, setIncidentScreen } from '../lib/client';
import { Appointment, DoctorNotification, paymentRequiredFromPolicy, Profile, Question, RecordDto, RxPage, consultationAction, rxCandidates } from '../lib/contracts';
import { Modal, State, Drawer } from './Common';
import { Icon } from './Icons';
import { Auth } from './Auth';
import { ClinicalAction, useAppointments } from './Queue';
import { Shell, type Page } from './Shell';
import { Dashboard } from './Dashboard';
import { Appointments } from './Appointments';
import { Schedule } from './Schedule';
import { Telehealth } from './Telehealth';
import { ConsultationHistory } from './ConsultationHistory';
import { Patients } from './Patients';
import { Chat } from './Chat';
import { Rx } from './Rx';
import { Qna } from './Qna';
import { Articles } from './Articles';
import { Settings, SettingsSection } from './Settings';
import { DeferredFinancialNotice, PayoutSettings, Wallet } from './Wallet';
import { Statement } from './Statement';
import { Notifications } from './Notifications';
import { Verification } from './Verification';
import { Completion, DocumentViewer, Documents, History, Receipt, SlotForm, Summary } from './Clinical';
import { ReadyToast } from './ReadyToast';

type Dialog = 'summary' | 'history' | 'documents' | 'completion' | 'slot' | 'statement' | 'payout' | 'receipt' | 'notifications' | 'gap' | null;
class Boundary extends React.Component<{ children: React.ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(error: Error) { diagnostic('render-error', error); }
  render() { return this.state.failed ? <CenteredCard><State error="This screen could not be shown. Reload to restore your session."/><button className="am-btn primary block" onClick={() => location.reload()}>Reload</button></CenteredCard> : this.props.children; }
}
function CenteredCard({ children }: { children: React.ReactNode }) {
  return <main style={{ minHeight: '100dvh', display: 'grid', placeItems: 'center', padding: 24 }}><div className="am-card pad" style={{ width: '100%', maxWidth: 440 }}>{children}</div></main>;
}
export function DoctorApplication({ initialPage = 'dashboard' }: { initialPage?: 'dashboard' | 'chat' }) { return <Boundary><Application initialPage={initialPage}/></Boundary>; }
const pages: Page[] = ['chat', 'rx', 'schedule', 'wallet', 'qna', 'dashboard', 'appointments', 'patients', 'telehealth', 'settings', 'articles', 'consultation-history'];
function locationPage(): Page { const stored = window.history.state?.doctorWebPage; return pages.includes(stored) ? stored : window.location.pathname === '/doctor/chats' ? 'chat' : 'dashboard'; }
const screens: Record<Page, string> = { chat: 'Chat', rx: 'SmartRx', schedule: 'Schedule', wallet: 'Wallet', qna: 'Qna', telehealth: 'Telehealth', settings: 'Settings', articles: 'Articles', patients: 'Patients', appointments: 'Queue', 'consultation-history': 'History', dashboard: 'Application' };
const titles: Record<string, string> = { summary: 'Consultation details', history: 'Patient case history', documents: 'Consultation documents', completion: 'Complete consultation', slot: 'Add availability', statement: 'Monthly earnings statement', payout: 'Payout settings', receipt: 'Consultation fee breakdown', notifications: 'Notifications', gap: 'Not available yet' };

function Application({ initialPage }: { initialPage: 'dashboard' | 'chat' }) {
  const [profile, setProfile] = useState<Profile | null>(null); const [restoring, setRestoring] = useState(true); const [authError, setAuthError] = useState('');
  const [page, setPage] = useState<Page>(initialPage); const [search, setSearch] = useState(''); const [active, setActive] = useState<Appointment | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null); const [gapMessage, setGap] = useState(''); const [navOpen, setNavOpen] = useState(false);
  const [duty, setDuty] = useState<{ value?: boolean; busy: boolean; error: string }>({ busy: false, error: '' });
  const [rxByAppointment, setRx] = useState<Record<string, RxPage[]>>({}); const [receipt, setReceipt] = useState<RecordDto | null>(null);
  const [document, setDocument] = useState<{ appointmentId: string; document: RecordDto; current?: string } | null>(null);
  const [offline, setOffline] = useState(false); const [suspended, setSuspended] = useState(false); const [revision, setRevision] = useState(0);
  const [photo, setPhoto] = useState<string | undefined>(); const [photoVersion, setPhotoVersion] = useState(0);
  const channel = useRef<BroadcastChannel | null>(null); const pagesRef = useRef(rxByAppointment); pagesRef.current = rxByAppointment;
  const epoch = useRef(0); const dutyLock = useRef(false); const profileRef = useRef(profile); profileRef.current = profile;
  const [scheduleDate, setScheduleDate] = useState(''); const [settingsSection, setSettingsSection] = useState<SettingsSection>('profile');
  const enabled = isProfessionallyVerified(profile) && !suspended && !offline;
  const appointmentPage = ['dashboard', 'appointments', 'rx', 'patients', 'telehealth', 'schedule', 'chat'].includes(page);
  const queue = useAppointments(enabled && appointmentPage, page === 'schedule' || page === 'patients' ? 'all' : 'upcoming', ['dashboard', 'appointments', 'telehealth'].includes(page) && dialog === null && document === null);
  const unread = useResource<{ unread_count: number }>(enabled ? 'notifications/unread-count' : null);
  const openQuestions = useResource<Question[]>(enabled ? 'doctor/qna/questions?status=OPEN&limit=100&offset=0' : null);
  // Server-authoritative financial mode (`GET /config/booking-policy`); defaults to true
  // until the server confirms the module is deferred.
  const bookingPolicy = useResource<{ payment_required: boolean }>(enabled ? 'config/booking-policy' : null);
  const paymentRequired = paymentRequiredFromPolicy(bookingPolicy.data);
  useEffect(() => { if (!enabled) return; return startScheduleSync(unread.refresh, { window, document: window.document, online: () => navigator.onLine, every: (cb, ms) => window.setInterval(cb, ms), cancel: id => window.clearInterval(id as number) }, 60000); }, [enabled, unread.refresh]);
  // Own photo through the binary BFF route; the object URL is revoked on change and sign-out.
  useEffect(() => {
    if (!profile) return;
    let url = ''; let cancelled = false;
    fetch('/api/photo', { credentials: 'same-origin', cache: 'no-store' }).then(async res => { if (!res.ok || cancelled) return; url = URL.createObjectURL(await res.blob()); if (!cancelled) setPhoto(url); }).catch(() => {});
    return () => { cancelled = true; if (url) URL.revokeObjectURL(url); setPhoto(undefined); };
  }, [profile?.id, photoVersion]);
  const wipe = useCallback(() => { epoch.current++; clearRuntime(); setProfile(null); setActive(null); setDialog(null); setDocument(null); setPage(window.location.pathname === '/doctor/chats' ? 'chat' : 'dashboard'); setSearch(''); setSettingsSection('profile'); Object.values(pagesRef.current).flat().forEach(p => URL.revokeObjectURL(p.preview)); setRx({}); setDuty({ busy: false, error: '' }); }, []);
  useEffect(() => {
    if (!window.history.state?.doctorWebPage) window.history.replaceState({ ...window.history.state, doctorWebPage: initialPage }, '', window.location.href);
    setPage(locationPage());
    const back = () => { setPage(locationPage()); setActive(null); setDialog(null); setDocument(null); setNavOpen(false); };
    window.addEventListener('popstate', back); return () => window.removeEventListener('popstate', back);
  }, [initialPage]);
  // `silent` restores (visibility/online) still revalidate the session but never replace
  // the tree with a blocking spinner (which would destroy in-progress forms).
  const restore = useCallback(async (trigger: RestoreTrigger = 'manual-retry') => { const silent = restoresSilently(trigger); const version = ++epoch.current; if (!silent) setRestoring(true); setAuthError(''); try { const data = await api<Profile>('doctor/profile'); if (version !== epoch.current) return; setProfile(data); const status = await api<{ is_on_duty: boolean }>('doctor/duty'); if (version !== epoch.current) return; setDuty({ value: status.is_on_duty, busy: false, error: '' }); setSuspended(false); } catch (e) { if (version !== epoch.current) return; if (e instanceof Error && 'status' in e && e.status === 401) { wipe(); setRestoring(false); } else if (!silent) { setAuthError(errorText(e)); setProfile(null); } } finally { if (version === epoch.current) setRestoring(false); } }, [wipe]);
  useEffect(() => { void restore('mount'); const expired = () => { wipe(); setRestoring(false); setAuthError('Your session ended. Please sign in again.'); }; window.addEventListener('doctor-session-expired', expired); const browserError = (event: ErrorEvent) => diagnostic('browser-error', event.error), rejection = (event: PromiseRejectionEvent) => diagnostic('promise-rejection', event.reason); window.addEventListener('error', browserError); window.addEventListener('unhandledrejection', rejection); channel.current = new BroadcastChannel('hellodoctor-doctor-session'); channel.current.onmessage = () => { wipe(); setRestoring(false); setAuthError('The session changed in another tab. Sign in to continue.'); }; const onOffline = () => { setOffline(true); setSuspended(true); clearRuntime(); setDialog(null); setDocument(null); }, onOnline = () => { setOffline(false); if (profileRef.current) void restore('reconnected'); }; window.addEventListener('offline', onOffline); window.addEventListener('online', onOnline); const visibility = () => { if (window.document.hidden) { setSuspended(true); setDialog(null); setDocument(null); clearRuntime(); } else if (profileRef.current) { void restore('visibility-regained'); } }; window.document.addEventListener('visibilitychange', visibility); return () => { window.removeEventListener('doctor-session-expired', expired); window.removeEventListener('error', browserError); window.removeEventListener('unhandledrejection', rejection); window.removeEventListener('offline', onOffline); window.removeEventListener('online', onOnline); window.document.removeEventListener('visibilitychange', visibility); channel.current?.close(); }; }, [restore, wipe]);
  const close = useCallback(() => { setDialog(null); setDocument(null); }, []);
  useEffect(() => { setIncidentScreen(screens[page]); }, [page]);
  function gap(message: string) { setGap(message); setDialog('gap'); }
  function go(target: Page) { if (target !== page) window.history.pushState({ doctorWebPage: target }, '', target === 'chat' ? '/doctor/chats' : '/'); setNavOpen(false); setPage(target); setDialog(null); setDocument(null); window.scrollTo({ top: 0 }); }
  const action: ClinicalAction = (name, a) => { setActive(a); if (name === 'chat') go('chat'); else if (name === 'rx') go('rx'); else if (name === 'call') go('telehealth'); else setDialog(name); };
  function openNotice(notice: DoctorNotification) {
    setDialog(null);
    if (notice.notification_type.startsWith('REGISTRATION_')) { setSettingsSection('professional'); go('settings'); }
    else if (['EARNING_CREDITED', 'PAYOUT_PAID'].includes(notice.notification_type)) go('wallet');
    else if (notice.notification_type === 'CONSULTATION_REMINDER_15M') { const appointment = queue.data.find(a => a.id === notice.related_entity_id); if (appointment) action(appointment.modality === 'CHAT' ? 'chat' : 'call', appointment); else go('appointments'); }
    else if (notice.related_entity_type === 'CHAT') { const appointment = queue.data.find(a => a.id === notice.metadata?.appointment_id || a.conversation_id === notice.related_entity_id); if (appointment) action('chat', appointment); else go('chat'); }
    else if (notice.notification_type === 'ARTICLE_STATUS_CHANGED') go('articles');
    else go('appointments');
  }
  async function toggleDuty() { if (dutyLock.current || duty.value === undefined) return; dutyLock.current = true; setDuty(previous => ({ ...previous, busy: true, error: '' })); try { const data = await api<{ is_on_duty: boolean }>('doctor/duty', { method: 'PUT', body: JSON.stringify({ is_on_duty: !duty.value }) }); setDuty({ value: data.is_on_duty, busy: false, error: '' }); } catch (e) { setDuty(previous => ({ ...previous, busy: false, error: errorText(e) })); } finally { dutyLock.current = false; } }
  async function logout() { channel.current?.postMessage({ event: 'logout' }); try { await request('/api/auth/logout', { method: 'POST' }); wipe(); setRestoring(false); } catch (e) { wipe(); setRestoring(false); setAuthError(errorText(e)); } }
  function boundary() { clearRuntime(); channel.current?.postMessage({ event: 'account-boundary' }); }

  if (restoring) return <main style={{ minHeight: '100dvh', display: 'grid', placeItems: 'center' }}><div className="am-stack" style={{ alignItems: 'center' }}><img src="/brand/afya_logo.png" alt="" width={56} height={56}/><State loading/></div></main>;
  if (!profile) return <Auth onSignedIn={() => void restore()} onBoundary={boundary} sessionError={authError} onRetry={() => void restore()}/>;
  if (!isProfessionallyVerified(profile)) return <Verification profile={profile} onRefresh={() => void restore()} onLogout={() => void logout()} onUpdated={setProfile}/>;

  const renderGeneration = runtimeGeneration();
  const liveQueue = queue.data.filter(a => a.queue_actionable === true);
  const chosen = active ?? liveQueue[0] ?? queue.data[0] ?? null;
  function showDocument(appointmentId: string, doc: RecordDto, current?: string) { setDocument({ appointmentId, document: doc, current: current ?? chosen?.id }); }
  const openProfile = () => { setSettingsSection('professional'); go('settings'); };
  const openSettings = () => { setSettingsSection('profile'); go('settings'); };
  let view: React.ReactNode;
  if (offline || suspended) view = <div className="am-card"><State error={offline ? 'You are offline. Reconnect to load patient information.' : 'Checking your session…'} retry={() => void restore()}/></div>;
  else if (page === 'chat') view = <Chat profile={profile} appointments={queue.data} selectedAppointment={active?.id} focusAppointment={active ?? undefined} visible={dialog === null && document === null && !navOpen} action={action} gap={gap}/>;
  else if (page === 'rx') view = <Rx profile={profile} appointments={rxCandidates(queue.data, active)} appointment={active} pages={active ? rxByAppointment[active.id] ?? [] : []} setPages={next => { if (active && renderGeneration === runtimeGeneration()) setRx(previous => ({ ...previous, [active.id]: next })); }} select={setActive} action={action} complete={() => setDialog('completion')} back={() => go('telehealth')}/>;
  else if (page === 'schedule') view = <Schedule key={revision} appointments={queue.data} action={action} addSlot={() => setDialog('slot')} gap={gap} initialDate={scheduleDate}/>;
  else if (page === 'wallet') view = paymentRequired ? <Wallet statement={() => setDialog('statement')} payout={() => setDialog('payout')} receipt={row => { setReceipt(row); setDialog('receipt'); }}/> : <DeferredFinancialNotice/>;
  else if (page === 'qna') view = <Qna profile={profile} onChanged={() => void openQuestions.refresh()}/>;
  else if (page === 'appointments') view = <Appointments queue={queue} action={action} search={search} setSearch={setSearch} addSlot={() => setDialog('slot')} schedule={() => go('schedule')}/>;
  else if (page === 'dashboard') view = <Dashboard profile={profile} queue={queue} duty={duty} go={go} action={action} paymentRequired={paymentRequired} openQuestions={openQuestions} search={() => { go('appointments'); window.setTimeout(() => window.document.querySelector<HTMLInputElement>('.am-search input')?.focus(), 50); }}/>;
  else if (page === 'consultation-history') view = <ConsultationHistory search={search} onDocument={showDocument} action={action}/>;
  else if (page === 'articles') view = <Articles specialty={profile.primary_specialty}/>;
  else if (page === 'settings') view = <Settings profile={profile} photo={photo} onPhotoChanged={() => setPhotoVersion(v => v + 1)} section={settingsSection} onSection={setSettingsSection} onUpdated={setProfile} duty={duty} onDuty={() => void toggleDuty()} onWallet={() => go('wallet')} onLogout={() => void logout()} paymentRequired={paymentRequired}/>;
  else if (page === 'patients') view = <Patients queue={queue} action={action} search={search}/>;
  else view = <Telehealth appointment={chosen} appointments={liveQueue} select={setActive} profile={profile} photo={photo} action={action} onDocument={showDocument} complete={() => chosen ? setDialog('completion') : gap('Select a consultation before completing it.')} gap={gap}/>;
  const pagedList = appointmentPage && page !== 'chat' && page !== 'telehealth' && queue.hasMore;
  const ready = liveQueue.find(a => consultationAction(a) !== null);

  return <>
    <Shell profile={profile} photo={photo} page={page} go={go} navOpen={navOpen} setNavOpen={setNavOpen} search={search} onSearch={value => { setSearch(value); if (!['appointments', 'patients', 'consultation-history'].includes(page)) go('appointments'); }}
      duty={duty} onDuty={() => void toggleDuty()} unread={unread.data?.unread_count} onNotifications={() => setDialog('notifications')} onProfile={openProfile} onSettings={openSettings} onLogout={() => void logout()}
      paymentRequired={paymentRequired} liveCount={liveQueue.length} openQuestions={openQuestions.data?.length} offline={offline}>
      <div key={`${page}-${revision}`} className="am-stack" style={{ gap: 22 }}>{view}</div>
      {pagedList && <div className="am-row" style={{ justifyContent: 'center' }}><button className="am-btn ghost" disabled={queue.moreBusy} onClick={() => void queue.loadMore()}>{queue.moreBusy ? 'Loading consultations…' : 'Load more consultations'}</button></div>}
    </Shell>
    {ready && page !== 'telehealth' && page !== 'chat' && !dialog && <ReadyToast appointment={ready} onStart={() => action(consultationAction(ready)!, ready)} onDetails={() => action('summary', ready)}/>}
    {dialog === 'notifications' && <Drawer title="Notifications" subtitle="Updates about your consultations and account" onClose={close}><Notifications onChanged={() => void unread.refresh()} onOpen={openNotice}/></Drawer>}
    {dialog === 'summary' && chosen && <Drawer title={titles.summary} subtitle={chosen.appointment_number} onClose={close}><Summary appointment={chosen} history={() => setDialog('history')} documents={() => setDialog('documents')} complete={() => setDialog('completion')} action={action} onDocument={showDocument}/></Drawer>}
    {dialog === 'history' && chosen && <Drawer title={titles.history} subtitle={`${chosen.patient_display_name} · access tied to ${chosen.appointment_number}`} onClose={close}><History appointment={chosen} onDocument={showDocument}/></Drawer>}
    {dialog === 'documents' && chosen && <Drawer title={titles.documents} subtitle={`${chosen.patient_display_name} · ${chosen.appointment_number}`} onClose={close}><Documents appointment={chosen} onDocument={showDocument}/></Drawer>}
    {dialog && !['notifications', 'summary', 'history', 'documents'].includes(dialog) && <Modal title={titles[dialog]} onClose={close} wide={['statement'].includes(dialog)} bare={dialog === 'completion'}>
      {dialog === 'completion' && chosen && <Completion appointment={chosen} onCompleted={() => void queue.refresh()} onClose={close} onView={() => setDialog('summary')}/>}
      {dialog === 'slot' && <SlotForm onClose={close} onCreated={date => { setScheduleDate(date); setPage('schedule'); setRevision(r => r + 1); void queue.refresh(); }}/>}
      {dialog === 'statement' && (paymentRequired ? <Statement profile={profile}/> : <DeferredFinancialNotice/>)}
      {dialog === 'payout' && (paymentRequired ? <PayoutSettings/> : <DeferredFinancialNotice/>)}
      {dialog === 'receipt' && receipt && <Receipt row={receipt}/>}
      {dialog === 'gap' && <div className="am-callout blue"><Icon name="info"/><p>{gapMessage}</p></div>}
    </Modal>}
    {document?.current && <DocumentViewer current={document.current} appointmentId={document.appointmentId} document={document.document} onClose={() => setDocument(null)}/>}
  </>;
}
