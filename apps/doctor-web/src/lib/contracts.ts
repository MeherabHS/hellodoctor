// Current Rust DTO fields; no financial or authorization decisions are made here.
export type RecordDto = Record<string, unknown>;
export interface Profile { id: string; user_id: string; full_name: string; license_number: string; license_authority: string; current_hospital: string; primary_specialty: string; is_verified: boolean; professional_verification_status: 'PENDING' | 'VERIFIED' | 'REJECTED'; license_country: string; professional_verification_note?: string | null; qualifications: string[] }
export interface Appointment { queue_actionable?: boolean; id: string; appointment_number: string; patient_id: string; patient_display_name: string; modality: string; status: string; doctor_lifecycle: string; start_time: string; end_time: string; history_access_from: string; history_access_until: string | null; history_access_active: boolean; clinical_outcome: string | null; conversation_id?: string | null; chat_expires_at?: string | null; chat_locked?: boolean | null }
export interface Slot { bookable?: boolean; id: string; status: string; start_time: string; end_time: string; appointment_id: string | null; patient_display_name: string | null }
export interface Conversation { id: string; appointment_id: string; patient_display_name: string; expires_at: string; is_locked: boolean; last_message_at: string | null; modality?: string; appointment_status?: string }
export interface Message { id: string; sender_id: string; content: string; sent_at: string; attachment_id?: string; attachment?: {id:string;display_name:string;mime_type:string;size_bytes:number}|null }
export interface Question { id: string; question_text: string; status: string; category: string | null; created_at: string; answer_text: string | null; answering_doctor: string | null }
export interface DoctorNotification { metadata?:{appointment_id?:string}; id:string; notification_type:string; title:string; body:string; related_entity_type:string|null; related_entity_id:string|null; created_at:string; read_at:string|null }
export interface Statement extends RecordDto { period: string; currency: string; status: string; amount_label: string; gross_earnings: string; platform_charge: string; basis_note: string; adjustments: string; payable: string; payout_status: string; entries: RecordDto[]; next_offset: number | null }
export interface RxPage { key: string; file: File; preview: string; state: 'LOCAL' | 'UPLOADING' | 'UPLOADED' | 'FAILED'; requestKey: string; documentId?: string; error?: string; uploadNotes?: string; originalBytes?: number; uploadProgress?: number | null }

export function messageSide(message: Pick<Message,'sender_id'>, authenticatedUserId: string): 'doctor'|'patient' {
  return message.sender_id === authenticatedUserId ? 'doctor' : 'patient';
}
export function upcomingFirst(appointments: Appointment[]): Appointment[] {
  const serverOrdered = appointments.every(a => typeof a.queue_actionable === 'boolean');
  if (serverOrdered) return [...appointments];
  const terminal = (a: Appointment) => ['COMPLETED', 'CANCELLED', 'EXPIRED', 'PREMATURE_TERMINATION', 'DISPUTED', 'REFUNDED'].includes(a.status) || a.doctor_lifecycle === 'DONE';
  return [...appointments].sort((a, b) => Number(terminal(a)) - Number(terminal(b)) || (terminal(a) ? Date.parse(b.start_time) - Date.parse(a.start_time) : Date.parse(a.start_time) - Date.parse(b.start_time)) || a.id.localeCompare(b.id));
}
export function consultationAction(a: Pick<Appointment,'modality'|'status'|'chat_expires_at'|'chat_locked'> & {queue_actionable?:boolean}, now=Date.now()): 'chat'|'call'|null {
  if (a.queue_actionable===false || !['CONFIRMED','WAITING','IN_CONSULTATION'].includes(a.status)) return null;
  if (a.modality==='VIDEO' && a.status==='WAITING') return null;
  if (a.modality==='CHAT' && (a.chat_locked || (a.chat_expires_at && Date.parse(a.chat_expires_at)<=now))) return null;
  return a.modality === 'CHAT' ? 'chat' : a.modality === 'VIDEO' ? 'call' : null;
}
export function consultationActionLabel(a: Pick<Appointment,'modality'|'status'|'chat_expires_at'|'chat_locked'>): string {
  const action = consultationAction(a);
  return action === 'chat' ? 'Start chat consultation' : action === 'call' ? 'Start video consultation' : 'Consultation unavailable';
}
export function chatActionLabel(c: Conversation, now=Date.now()): string {
  return chatLocked(c,now) ? 'View consultation history' : c.modality==='CHAT' && !c.last_message_at ? 'Start chat consultation' : 'Open chat';
}
export function chatLocked(chat: Conversation, now = Date.now()): boolean { return ['CANCELLED','REFUNDED','EXPIRED','PREMATURE_TERMINATION','DISPUTED','PENDING_PAYMENT'].includes(chat.appointment_status ?? '') || chat.is_locked || !Number.isFinite(Date.parse(chat.expires_at)) || Date.parse(chat.expires_at) <= now; }
export function money(value: unknown): string {
  if (value === null || value === undefined || value === '') return '—';
  // Format decimal strings without rounding through binary floating point.
  const raw = String(value);
  if (!/^-?\d+(\.\d+)?$/.test(raw)) return '—';
  const negative = raw.startsWith('-');
  const [whole, decimals = ''] = raw.replace(/^-/, '').split('.');
  return `KES ${negative ? '-' : ''}${whole.replace(/\B(?=(\d{3})+(?!\d))/g, ',')}.${decimals.padEnd(2, '0').slice(0, 2)}`;
}
export function statementLabel(statement: Pick<Statement, 'status' | 'amount_label'>): string {
  return statement.status === 'FINALIZED' && statement.amount_label === 'FINAL_PAYABLE' ? 'Final payable' : 'Estimated payout';
}
export function payoutLabel(status: unknown, provider: unknown): string {
  if (provider === 'DEV_SIMULATED') return `Development simulation · ${String(status ?? 'Not allocated')}`;
  return status === 'PAID' ? 'Paid' : String(status ?? 'Not allocated').replaceAll('_', ' ');
}
export function safeBackendPath(path: string, method: string): boolean {
  const id = '[0-9a-fA-F-]{36}';
  const routes: [string, string][] = [
    ['GET|POST', 'doctor/articles'], ['GET|PATCH', `doctor/articles/${id}`], ['POST', `doctor/articles/${id}/(?:publish|archive)`],
    ['GET|PATCH', 'doctor/profile'], ['GET|PUT', 'doctor/duty'], ['GET|POST', 'doctor/slots'],
    ['DELETE|PATCH', `doctor/slots/${id}`], ['GET', 'doctor/appointments'], ['GET', `appointments/${id}/conversation`], ['POST', `appointments/${id}/trace-ack`], ['GET', 'doctor/chats'],
    ['GET', `doctor/appointments/${id}/patient-history(?:/${id})?`],
    ['GET', `doctor/consultations/${id}/summary`], ['GET', 'doctor/qna/questions'],
    ['POST', `doctor/qna/questions/${id}/answers`], ['GET', 'doctor/wallet/(?:summary|ledger|statements|payout-methods)'],
    ['POST', `chat/${id}/attachments`], ['GET', `chat/${id}/messages/${id}/attachment`],
    ['GET|POST', `chat/${id}/messages`], ['POST', `consultations/${id}/(?:complete|complete-no-rx|prescription)`],
    ['POST', `consultations/${id}/(?:rtc-token|telemetry|telemetry/events)`],
    ['GET', 'notifications(?:/unread-count)?'], ['POST', `notifications/(?:read-all|${id}/read)`]
  ];
  return routes.some(([methods, pattern]) => new RegExp(`^(?:${methods})$`).test(method) && new RegExp(`^(?:${pattern})$`).test(path));
}

/** Retain a selected historical encounter even beyond the loaded operational page. */
export function consultationContext(appointments: Appointment[], id: string | undefined, selected?: Appointment): Appointment | undefined {
  if (!id) return undefined;
  return appointments.find(a => a.id === id) || (selected?.id === id ? selected : undefined);
}

/** Order UTC/offset instants without losing PostgreSQL microsecond precision. */
export function compareMessageInstants(left: string, right: string): number {
  const a=Date.parse(left),b=Date.parse(right);
  if (!Number.isFinite(a) || !Number.isFinite(b)) return left.localeCompare(right);
  const tail=(value:string)=>Number((value.match(/\.(\d+)(?:Z|[+-])/i)?.[1] || '').padEnd(6,'0').slice(3,6));
  return a-b || tail(left)-tail(right);
}

// Preserve the BFF query boundary; selected-day reads are specific to Doctor slots.
export function allowedProxyQuery(path:string, query:URLSearchParams):boolean {
  const allowed=new Set(['limit','offset','status','month','view','before_time','before_id','after_time','after_id']);
  if(path==='doctor/slots')allowed.add('day');
  return [...query.keys()].every(key=>allowed.has(key));
}
