import { tr } from './i18n/core.ts';
import { createSoftReads } from './inflight.mjs';
import { adminFetch } from './adminQueue.mjs';
import { clearStepUpExpiry, confirmStepUp, isReauthRequired } from './stepUp.mjs';
// API client connecting HelloDoctor Admin Portal to the Axum Rust Backend.
//
// Every call goes through this Next.js server's own same-origin proxy
// (`/api/proxy/*`, see src/app/api/proxy/[...path]/route.ts), never directly
// to the backend — the browser never holds a bearer token at all; the proxy
// attaches it server-side from an HttpOnly cookie. This is the BFF pattern:
// it exists specifically so a token can never be read by client-side
// JavaScript (and therefore never exfiltrated via an XSS payload the way a
// `localStorage`-stored token could be).
export const API_BASE = "/api/proxy";

export interface ApiResponse<T> {
  success: boolean;
  data: T;
  error?: string;
}

export interface AdminOverview {
  modality_split?:{videoCount:number;chatCount:number;videoRevenue:string;chatRevenue:string};
  total_doctors: number;
  on_duty_doctors: number;
  pending_professional_verification: number;
  active_consultations: number;
  total_patients: number;
  gross_volume: string;
  platform_fees: string;
  pending_disbursement: string;
  pending_grievances: number;
  unresolved_traces: number;
  system_health: "OPTIMAL" | "DEGRADED";
}

export interface AdminDoctor {
  id: string;
  user_id: string;
  full_name: string;
  license_number: string;
  license_authority: string;
  primary_specialty: string;
  experience_years: number;
  current_hospital: string;
  residential_address: string;
  verified_phone: string;
  qualifications: string[];
  consultation_fee_video: string;
  consultation_fee_chat: string;
  is_on_duty: boolean;
  is_verified: boolean; // derived compatibility only
  professional_verification_status: "PENDING" | "VERIFIED" | "REJECTED";
  license_country: string;
  consultations_count: number;
  lifetime_gross: string;
  lifetime_fee_withheld: string;
  lifetime_net: string;
  pending_disbursement: string;
  last_disbursement_at: string | null;
  last_disbursement_amount: string;
}

export interface DoctorDossier {
  id: string;
  full_name: string;
  license_number: string;
  license_authority: string;
  license_country: string;
  primary_specialty: string;
  experience_years: number;
  current_hospital: string;
  residential_address: string;
  verified_phone: string;
  qualifications: string[];
  consultation_fee_video: string;
  consultation_fee_chat: string;
  is_on_duty: boolean;
  is_verified: boolean; // derived compatibility only
  professional_verification_status: "PENDING" | "VERIFIED" | "REJECTED";
  wallet: {
    lifetime_gross: string;
    lifetime_fee_withheld: string;
    lifetime_net: string;
    pending_disbursement: string;
    last_disbursement_at: string | null;
    last_disbursement_amount: string;
  };
  encounters: Array<{
    appointment_id: string;
    appointment_number: string;
    patient_name: string;
    modality: string;
    status: string;
    consultation_fee: string;
    created_at: string;
    clinical_outcome: string | null;
  }>;
  disbursements: Array<{
    id: string;
    batch_id: string;
    gross_amount: string;
    platform_fee: string;
    net_amount: string;
    status: string;
    created_at: string;
  }>;
  disciplinary_warnings: string[];
}

export interface AdminTransaction {
  id: string;
  transaction_number: string;
  appointment_id: string;
  payment_session_id: string;
  patient_name: string;
  doctor_name: string;
  gateway: "BKASH" | "NAGAD" | "CARD" | "MPESA" | "BANK_TRANSFER";
  gateway_reference: string | null;
  gross_amount: string;
  platform_fee_amount: string;
  net_amount: string;
  payment_status:
    | "INITIATED"
    | "PAYMENT_HELD"
    | "SETTLED_TO_DOCTOR"
    | "DISBURSED"
    | "REFUNDED_TO_PATIENT"
    | "REFUND_PENDING"
    | "FAILED";
  reconciliation_required?: boolean;
  created_at: string;
  settled_at: string | null;
}

export interface FinanceSummary {
  total_gross: string;
  total_platform_fee: string;
  total_net: string;
  pending_disbursement: string;
  // Kenya/KES-only deployment — bKash/Nagad are not live gateways here (see
  // services/backend/src/api/v1/admin_handlers.rs::get_finance_summary).
  gateway_volume: {
    card: string;
    mpesa: string;
  };
}

export interface CommandTelemetry {
  active_specialists: number;
  completed_today: number;
  active_sessions_count: number;
  active_streams: Array<{
    session_id: string;
    appointment_id: string;
    channel_name: string;
    doctor_name: string;
    patient_name: string;
    started_at: string | null;
    packet_loss_percent: string | null;
    rtt_ms: number | null;
    connection_state: string;
    session_status?: string;
    recovery_state?: string;
    empty_room_deadline?: string | null;
    interruption_time?: string | null;
    patient_present?: boolean;
    doctor_present?: boolean;
    patient_presence_sequence?: number;
    doctor_presence_sequence?: number;
    needs_review?: boolean;
    payment_status?: string | null;
    timeline_url?: string;
    /** Join window closed and nobody in the room: not live, left unclosed. */
    room_closed?: boolean;
    /** Still running: open, not awaiting review, join window not over. */
    ongoing?: boolean;
    join_window_closes_at?: string | null;
    /** The consultation trace is deleted this many days after each event. */
    trace_retention_days?: number;
    trace_event_count?: number;
    trace_expires_at?: string;
    /** A grievance holds the trace past its normal deletion. */
    trace_on_hold?: boolean;
    provider_presence_summary?: {
      source: string;
      patient_present: boolean;
      doctor_present: boolean;
      patient_sequence: number;
      doctor_sequence: number;
    };
  }>;
  needs_review_count?: number;
  /** Peer chat/call sessions running now. */
  peer_ongoing_count?: number;
  /** Peer sessions with activity in the tracking window (times, statuses and counts only). */
  peer_sessions?: PeerTrackingRow[];
}

export interface PeerTrackingRow {
  request_id: string; peer_id: string; peer_name: string; patient_name: string;
  support_type: 'CHAT' | 'CALL'; status: string;
  start_time: string; end_time: string; requested_at: string;
  decided_at: string | null; completed_at: string | null; cancelled_at: string | null;
  messages: number; last_message_at: string | null;
  call_status: string | null; call_started_at: string | null; call_ended_at: string | null;
  patient_present: boolean; peer_present: boolean;
  ongoing: boolean; last_activity_at: string; tracked_until: string; timeline_url: string;
}

export interface AdminSlot {
  id: string;
  doctor_id: string;
  doctor_name: string;
  start_time: string;
  end_time: string;
  status: "AVAILABLE" | "BLOCKED" | "LOCKED_IN_PAYMENT" | "BOOKED" | "CANCELLED";
  created_at: string;
}

export interface AdminPatient {
  id: string;
  user_id: string;
  full_name: string;
  date_of_birth: string;
  gender: string;
  blood_group: string | null;
  weight_kg: string | null;
  emergency_contact_phone: string | null;
  consultations_count: number;
  last_consult_at: string | null;
  created_at: string;
}

export interface PatientDossier {
  patient: AdminPatient;
  encounters: Array<{
    appointment_id: string;
    appointment_number: string;
    doctor_name: string;
    modality: string;
    status: string;
    fee: string;
    created_at: string;
    clinical_outcome: string | null;
  }>;
  vault_records: Array<{
    id: string;
    appointment_id: string;
    page_number: number;
    mime_type: string;
    file_size_bytes: number;
    security_status: string;
    created_at: string;
  }>;
}

export interface PendingDoctorVerification {
  id: string;
  full_name: string;
  license_number: string;
  license_authority: string;
  qualifications: string[];
  primary_specialty: string;
  experience_years: number;
  current_hospital: string;
  verified_phone: string;
  is_verified: boolean; // derived compatibility only
  professional_verification_status: "PENDING" | "VERIFIED" | "REJECTED";
  license_country: string;
  created_at: string;
}

export interface ComplianceAlert {
  id: string;
  rx_number: string;
  doctor_name: string;
  patient_name: string;
  category: string;
  flag_reason: string;
  doctor_notes: string;
  created_at: string;
  status: string;
}

export interface AdminLog {
  id: string;
  actor_id: string | null;
  actor_role: string;
  action: string;
  resource_type: string;
  resource_id: string | null;
  request_id: string | null;
  result: string;
  reason: string | null;
  created_at: string;
}

export interface AdminGrievance {
  id: string;
  grievance_number: string;
  patient_id: string;
  patient_name: string;
  consultation_id: string;
  doctor_name: string;
  target_type: "DOCTOR" | "SYSTEM";
  category: string;
  claim_summary: string;
  status: "PENDING_REVIEW" | "UNDER_INVESTIGATION" | "REFUNDED" | "WARNED" | "DISMISSED";
  created_at: string;
  telemetry: {
    duration_seconds: number;
    packet_loss_percent: string | null;
    rtt_ms: number | null;
    connection_state: string;
    premature_end: boolean;
    prescription_issued: boolean;
  } | null;
}

// Network Request Helper
let runtimeGeneration=0;
// Identical reads share one request while in flight and reuse its answer for
// 3 s (below every polling interval). Any write clears it, so the next read
// after a change always comes from the server.
const SOFT_READ_TTL_MS=3000;
const inflightReads=createSoftReads(SOFT_READ_TTL_MS);
export function clearAdminRuntime(){runtimeGeneration++;inflightReads.clear();}
/** For writes that go around request() (media uploads). */
export function clearSoftReads(){inflightReads.clear();}
export function request<T>(path:string,options?:RequestInit):Promise<T>{
 if((options?.method??'GET').toUpperCase()==='GET'){
  return !options?.signal&&!options?.headers
   ? inflightReads.run(`${runtimeGeneration}:${path}`,()=>performRequest<T>(path,options)) : performRequest<T>(path,options);
 }
 inflightReads.clear();
 return performRequest<T>(path,options).finally(()=>inflightReads.clear());
}
/** `POST /admin/reauth` for <StepUpDialog/>; never re-prompts itself. */
export function confirmAdminPassword(currentPassword: string): Promise<{ valid_for_seconds?: number }> {
  return performRequest('/admin/reauth', {method: 'POST', body: JSON.stringify({current_password: currentPassword})}, true);
}
async function performRequest<T>(path: string, options?: RequestInit, steppedUp = false): Promise<T> {
  const generation=runtimeGeneration;
  const url = `${API_BASE}${path}`;
  // Admin routes share the backend's two-request admission lane: queue them
  // (and retry reads it turned away) rather than racing into 503s.
  const send = path.startsWith("/admin") ? adminFetch : (input: string, init: RequestInit) => fetch(input, init);
  const res = await send(url, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      "X-Request-Id": crypto.randomUUID(),
      "X-Client-Type": "ADMIN_WEB",
      ...(options?.headers || {}),
    },
    credentials: "include",
    cache: "no-store",
  });

  if(generation!==runtimeGeneration)throw new DOMException('Account changed','AbortError');
  if (res.status === 401 && typeof window !== "undefined") {
    // Session cookie missing/expired and the proxy's own silent-refresh
    // attempt also failed — send the admin back to login rather than
    // surfacing a confusing in-page error.
    // Clear the authenticated document/runtime rather than retaining its caches.
    window.location.assign(new URL('/login', window.location.origin).href);
    throw new Error("Not authenticated.");
  }

  if (!res.ok) {
    const errorBody = await res.text();
    // Sensitive decisions need a fresh password: ask once, then retry once.
    if (!steppedUp && typeof window !== "undefined" && isReauthRequired(res.status, errorBody)) {
      clearStepUpExpiry();
      if (await confirmStepUp()) return performRequest<T>(path, options, true);
      throw new Error(tr('stepUp.cancelled'));
    }
    throw new Error(`API Error [${res.status}]: ${errorBody}`);
  }

  const json: ApiResponse<T> = await res.json();
  return json.data;
}


// Admin API Methods
export async function getAdminOverview(): Promise<AdminOverview> {
  return request<AdminOverview>("/admin/overview");
}

export async function getAdminDoctors(): Promise<AdminDoctor[]> {
  return request<AdminDoctor[]>("/admin/doctors");
}

export async function createDoctor(payload: {
  full_name: string;
  license_number: string;
  primary_specialty: string;
  experience_years: number;
  current_hospital: string;
  verified_phone: string;
  consultation_fee_video: number;
  consultation_fee_chat: number;
  residential_address?: string;
  qualifications?: string[];
}): Promise<any> {
  return request<any>("/admin/doctors", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function getDoctorDossier(id: string): Promise<DoctorDossier> {
  return request<DoctorDossier>(`/admin/doctors/${id}/dossier`);
}

export async function initiateDisbursement(
  period_start: string,
  period_end: string
): Promise<any> {
  return request<any>("/admin/disbursements/initiate", {
    method: "POST",
    body: JSON.stringify({ period_start, period_end }),
  });
}

export async function approveDisbursementItem(itemId: string): Promise<any> {
  return request<any>(`/admin/disbursements/${encodeURIComponent(itemId)}/approve`, {
    method: "POST",
  });
}

export async function getAdminTransactions(month?: string): Promise<AdminTransaction[]> {
  const query = month ? `?month=${encodeURIComponent(month)}` : '';
  return request<AdminTransaction[]>(`/admin/finance/transactions${query}`);
}

export async function getFinanceSummary(month?: string): Promise<FinanceSummary> {
  const query = month ? `?month=${encodeURIComponent(month)}` : '';
  return request<FinanceSummary>(`/admin/finance/summary${query}`);
}

// Server-authoritative `FINANCIAL_MODULE_MODE` (`GET /config/booking-policy`).
// The Admin Web client must never decide this independently — see
// `services/backend/src/api/v1/config_handlers.rs`.
export interface BookingPolicy { payment_required: boolean }
export async function getBookingPolicy(): Promise<BookingPolicy> {
  return request<BookingPolicy>('/config/booking-policy');
}

export async function getPayoutRouting(month?: string): Promise<any[]> {
  const query = month ? `?month=${encodeURIComponent(month)}` : '';
  return request<any[]>(`/admin/finance/payout-routing${query}`);
}

export async function finalizeFinancialPeriod(month: string): Promise<any> {
  const [year, monthNumber] = month.split('-').map(Number);
  return request<any>('/admin/finance/periods/finalize', {
    method: 'POST', body: JSON.stringify({ year, month: monthNumber }),
  });
}

// Finance refund-request resolution — the post-approval lifecycle a
// grievance refund enters once a board adjudication records it (see
// adjudicateGrievanceRefund below). Distinct from grievance adjudication
// itself: this is Finance actually moving the refund through
// APPROVED -> PROCESSING -> REFUNDED (or FAILED / RECONCILIATION_REQUIRED).
export interface FinancialRefundRequest {
  id: string;
  grievance_id: string;
  transaction_id: string;
  approved_by: string;
  approved_at: string;
  currency: string;
  amount: string;
  previous_payment_status: string;
  status: "APPROVED" | "PROCESSING" | "REFUNDED" | "FAILED" | "RECONCILIATION_REQUIRED";
  provider_reference: string | null;
  refunded_at: string | null;
}

export async function getRefundRequests(): Promise<FinancialRefundRequest[]> {
  return request<FinancialRefundRequest[]>("/admin/finance/refund-requests");
}

export async function transitionRefundRequest(
  id: string,
  status: "PROCESSING" | "REFUNDED" | "FAILED" | "RECONCILIATION_REQUIRED",
  provider_reference?: string
): Promise<FinancialRefundRequest> {
  return request<FinancialRefundRequest>(`/admin/finance/refund-requests/${encodeURIComponent(id)}/transition`, {
    method: "POST",
    body: JSON.stringify({ status, provider_reference: provider_reference || null }),
  });
}

export async function getCommandTelemetry(): Promise<CommandTelemetry> {
  return request<CommandTelemetry>("/admin/command/telemetry");
}

export async function resolveConsultationRecovery(
  sessionId: string,
  outcome: "RESOLVED_NO_SETTLEMENT" | "PENDING_FINANCE_ACTION",
  reason: string
): Promise<{ resolution_id: string; result: string }> {
  return request<{ resolution_id: string; result: string }>(`/admin/consultations/${encodeURIComponent(sessionId)}/recovery-resolution`, {
    method: "POST",
    body: JSON.stringify({ outcome, reason }),
  });
}

export async function getAdminSlots(): Promise<AdminSlot[]> {
  return request<AdminSlot[]>("/admin/slots");
}

export async function toggleSlot(slot_id: string, action: "BLOCK" | "UNBLOCK"): Promise<{ slot_id: string; status: AdminSlot['status'] }> {
  return request<{ slot_id: string; status: AdminSlot['status'] }>("/admin/slots", {
    method: "POST",
    body: JSON.stringify({ slot_id, action }),
  });
}

export async function createSlot(
  doctor_id: string,
  start_time: string,
  end_time: string
): Promise<any> {
  return request<any>("/admin/slots", {
    method: "POST",
    body: JSON.stringify({ doctor_id, start_time, end_time }),
  });
}

export async function getAdminPatients(): Promise<AdminPatient[]> {
  return request<AdminPatient[]>("/admin/patients");
}

export async function getPatientDossier(id: string): Promise<PatientDossier> {
  return request<PatientDossier>(`/admin/patients/${id}/dossier`);
}

export interface DataRightsOutcome {
  outcome: "HARD_DELETED" | "ANONYMIZED" | "PENDING";
  job_id?: string;
}

export async function executePatientDataRights(id: string, reason: string): Promise<DataRightsOutcome> {
  return request<DataRightsOutcome>(`/admin/patients/${id}/data-rights/erase`, {
    method: "POST",
    body: JSON.stringify({ reason }),
  });
}

export async function executeDoctorDataRights(id: string, reason: string): Promise<DataRightsOutcome> {
  return request<DataRightsOutcome>(`/admin/doctors/${id}/data-rights/erase`, {
    method: "POST",
    body: JSON.stringify({ reason }),
  });
}

export async function getPendingDoctorVerifications(): Promise<PendingDoctorVerification[]> {
  return request<PendingDoctorVerification[]>("/admin/doctors/pending-verification");
}

/** Documents a doctor uploaded while pending or when resubmitting after a rejection. */
export interface VerificationSubmission {
  id: string; message: string | null; previous_status: 'PENDING' | 'REJECTED'; previous_note: string | null; created_at: string;
  documents: { id: string; mime_type: string; size_bytes: number; created_at: string }[];
}
export async function getVerificationSubmissions(doctorId: string): Promise<VerificationSubmission[]> {
  return request<VerificationSubmission[]>(`/admin/doctors/${doctorId}/verification-submissions`);
}

export async function verifyDoctorProfessionalRegistration(id: string, note?: string): Promise<any> {
  return request<any>(`/admin/doctors/${id}/verify`, {
    method: "POST",
    body: JSON.stringify({ note: note || "Verified by medical administrator" }),
  });
}

export async function rejectDoctorProfessionalRegistration(id: string, note?: string): Promise<any> {
  return request<any>(`/admin/doctors/${id}/reject`, {
    method: "POST",
    body: JSON.stringify({ note: note || "Rejected by medical administrator" }),
  });
}

export async function getComplianceAlerts(): Promise<ComplianceAlert[]> {
  return request<ComplianceAlert[]>("/admin/compliance/alerts");
}

export async function getAdminLogs(): Promise<AdminLog[]> {
  return request<AdminLog[]>("/admin/logs");
}

// System Error Service — the dedicated `system_error_logs` admin surface
// (distinct from `/admin/logs`'s audit trail above). This is where genuine
// backend crashes/exceptions land, with a stable `trace_id`/`subsystem`/
// `component`/`error_code` identity (the admin panel's "codename") and,
// where the backend could capture one, a real `stack_trace` source-code
// location (the "code snippet"). See `docs/ERROR_HANDLING.md` §3 and
// `services/backend/src/system_error.rs`.
export interface SystemErrorLog {
  client_context?: {client:string;error_type:string;screen:string;operation:string;app_version:string;build_number:string;platform:string;backend_reference?:string|null;startup_reference?:string|null;source?:string} | null;
  id: string;
  trace_id: string;
  subsystem: string;
  component: string;
  severity: "INFO" | "WARNING" | "ERROR" | "CRITICAL";
  action_attempted: string;
  error_message: string;
  stack_trace: string | null;
  impacted_user_id: string | null;
  error_code: string | null;
  fingerprint: string | null;
  http_status: number | null;
  endpoint: string | null;
  occurrence_count: number;
  first_seen_at: string;
  last_seen_at: string;
  status: "OPEN" | "ACKNOWLEDGED" | "RESOLVED" | "IGNORED";
  acknowledged_by: string | null;
  acknowledged_at: string | null;
  resolved_by: string | null;
  resolved_at: string | null;
  resolution_note: string | null;
  logged_at: string;
}

export interface SystemErrorListResult {
  items: SystemErrorLog[];
  page: number;
  page_size: number;
  total: number;
  open_count: number;
  open_critical_count: number;
}

export async function getSystemErrors(params?: {
  severity?: string;
  status?: string;
  subsystem?: string;
}): Promise<SystemErrorListResult> {
  const qs = new URLSearchParams();
  qs.set("page_size", "200");
  if (params?.severity) qs.set("severity", params.severity);
  if (params?.status) qs.set("status", params.status);
  if (params?.subsystem) qs.set("subsystem", params.subsystem);
  return request<SystemErrorListResult>(`/admin/system-errors?${qs.toString()}`);
}

export async function getSystemErrorDetail(id: string): Promise<SystemErrorLog> {
  return request<SystemErrorLog>(`/admin/system-errors/${id}`);
}

export async function acknowledgeSystemError(id: string): Promise<any> {
  return request<any>(`/admin/system-errors/${id}/acknowledge`, { method: "POST" });
}

export async function resolveSystemError(id: string, resolution_note?: string): Promise<any> {
  return request<any>(`/admin/system-errors/${id}/resolve`, {
    method: "POST",
    body: JSON.stringify({ resolution_note: resolution_note || null }),
  });
}


export async function getAdminGrievances(): Promise<AdminGrievance[]> {
  return request<AdminGrievance[]>("/admin/grievances");
}

export async function adjudicateGrievanceRefund(
  id: string,
  audit_notes: string
): Promise<any> {
  return request<any>(`/admin/grievances/${id}/refund`, {
    method: "POST",
    body: JSON.stringify({ audit_notes }),
  });
}

export async function adjudicateGrievanceWarn(
  id: string,
  audit_notes: string
): Promise<any> {
  return request<any>(`/admin/grievances/${id}/warn`, {
    method: "POST",
    body: JSON.stringify({ audit_notes }),
  });
}

export async function adjudicateGrievanceDismiss(
  id: string,
  audit_notes: string
): Promise<any> {
  return request<any>(`/admin/grievances/${id}/dismiss`, {
    method: "POST",
    body: JSON.stringify({ audit_notes }),
  });
}

export interface InvestigationEvent {
 id:string; event_type?:string; action?:string; event_time_server?:string; created_at?:string;
 actor_user_id?:string; actor_role?:string; auth_session_id?:string; request_id?:string; trace_id?:string;
 resource_type?:string; resource_id?:string; result?:string; reason_code?:string;
 safe_metadata?:Record<string,unknown>; expires_at?:string; client_type?:string; client_build?:string;
}
export interface GrievanceInvestigation {
 grievance_id:string; facts:Record<string,unknown>; trace_events:InvestigationEvent[]; audit_events:InvestigationEvent[];
 records:Record<string,Record<string,unknown>[]>; preservation_holds:Record<string,unknown>[];
 telemetry:Record<string,unknown>; semantics:string;
 event_page?:{snapshot_at:string;total:number;has_more:boolean;next:{time:string;id:string;source:string}|null};
 record_page?:{offset:number;limit:number;totals:Record<string,number>};
}
export function getGrievanceInvestigation(id:string,page:Record<string,string|number|boolean>={}) {
 const query=new URLSearchParams(Object.entries({event_limit:50,record_limit:50,...page}).map(([key,value])=>[key,String(value)]));
 return request<GrievanceInvestigation>(`/admin/grievances/${encodeURIComponent(id)}/timeline?${query}`);
}
export function rescheduleAppointment(id:string,slot_id:string,reason:string,grievance_id:string) {
 return request(`/admin/appointments/${encodeURIComponent(id)}/reschedule`,{method:'POST',body:JSON.stringify({slot_id,reason,grievance_id})});
}

export interface BookableDoctorSlot {id:string;start_time:string;end_time:string;status:string}
export function getBookableDoctorSlots(doctor:string){return request<BookableDoctorSlot[]>(`/doctors/${encodeURIComponent(doctor)}/slots`);}

// Platform merchant/payment-method configuration — "the body waiting for
// the heart" (the real payment-gateway API key/driver). Never returns raw
// or encrypted credentials; the backend strips both from every response.
export interface PlatformPaymentMethod {
  id: string; provider_code: string; label: string; currency: string;
  masked_display: string; is_primary: boolean; is_active: boolean;
  created_by: string; created_at: string; updated_at: string;
}
export function getPlatformPaymentMethods() {
  return request<PlatformPaymentMethod[]>("/admin/platform-payment-methods");
}
export function savePlatformPaymentMethod(payload: {
  provider_code: string; label: string; masked_display: string;
  credentials: Record<string, string>; make_primary?: boolean;
}) {
  return request<PlatformPaymentMethod>("/admin/platform-payment-methods", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}
export function setPrimaryPlatformPaymentMethod(id: string) {
  return request<{ id: string; is_primary: boolean }>(`/admin/platform-payment-methods/${encodeURIComponent(id)}/set-primary`, { method: "POST" });
}
export function deactivatePlatformPaymentMethod(id: string) {
  return request<{ id: string; is_active: boolean }>(`/admin/platform-payment-methods/${encodeURIComponent(id)}`, { method: "DELETE" });
}

// Admin Campaigns (Milestones 6B + 6C) — draft/review/approval lifecycle,
// scheduling/pause/resume/cancel, dispatch monitoring. Dispatch itself is a
// backend scheduler pass; nothing here ever sends anything directly.
export type CampaignAudience = "PATIENTS" | "DOCTORS" | "BOTH";
export type CampaignStatus =
  | "DRAFT"
  | "PENDING_REVIEW"
  | "APPROVED"
  | "SCHEDULED"
  | "SENDING"
  | "PAUSED"
  | "COMPLETED"
  | "CANCELLED";

export interface CampaignContent {
  title_en: string;
  title_sw: string;
  body_en: string;
  body_sw: string;
  audience: CampaignAudience;
}

export interface Campaign extends CampaignContent {
  id: string;
  created_by: string;
  status: CampaignStatus;
  version: number;
  content_version: number;
  approved_content_version: number | null;
  approved_by: string | null;
  approved_at: string | null;
  created_at: string;
  updated_at: string;
  scheduled_for: string | null;
  sending_started_at: string | null;
  paused_at: string | null;
  completed_at: string | null;
  cancelled_at: string | null;
}

export interface CampaignStats {
  status: CampaignStatus;
  audience_count: number;
  queued: number;
  sent_to_provider: number;
  deferred_by_preference_or_quiet_hours: number;
  retry_pending: number;
  permanently_failed: number;
  sending_started_at: string | null;
  completed_at: string | null;
  cancelled_at: string | null;
}

export function listCampaigns(status?: CampaignStatus) {
  const query = status ? `?status=${encodeURIComponent(status)}` : "";
  return request<Campaign[]>(`/admin/campaigns${query}`);
}
export function getCampaign(id: string) {
  return request<Campaign>(`/admin/campaigns/${encodeURIComponent(id)}`);
}
export function getCampaignAudiencePreview(audience: CampaignAudience) {
  return request<{ audience: CampaignAudience; eligible_count: number }>(
    `/admin/campaigns/audience-preview?audience=${encodeURIComponent(audience)}`
  );
}
export function createCampaign(creation_key: string, content: CampaignContent) {
  return request<Campaign>("/admin/campaigns", {
    method: "POST",
    body: JSON.stringify({ creation_key, content }),
  });
}
export function updateCampaign(id: string, version: number, content: CampaignContent) {
  return request<Campaign>(`/admin/campaigns/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body: JSON.stringify({ version, content }),
  });
}
function campaignTransition(id: string, action: string, version: number, extra?: Record<string, unknown>) {
  return request<Campaign>(`/admin/campaigns/${encodeURIComponent(id)}/${action}`, {
    method: "POST",
    body: JSON.stringify({ version, ...extra }),
  });
}
export const submitCampaign = (id: string, version: number) => campaignTransition(id, "submit", version);
export const approveCampaign = (id: string, version: number) => campaignTransition(id, "approve", version);
export const pauseCampaign = (id: string, version: number) => campaignTransition(id, "pause", version);
export const resumeCampaign = (id: string, version: number) => campaignTransition(id, "resume", version);
export const cancelCampaign = (id: string, version: number) => campaignTransition(id, "cancel", version);
export const scheduleCampaign = (id: string, version: number, scheduled_for: string) =>
  campaignTransition(id, "schedule", version, { scheduled_for });
export function getCampaignStats(id: string) {
  return request<CampaignStats>(`/admin/campaigns/${encodeURIComponent(id)}/stats`);
}

/** Peer Navigators (docs/peer-navigator/PEER_NAVIGATOR_PLAN.md). No chat content is ever returned to Admins. */
export type PeerApprovalStatus = 'PENDING' | 'APPROVED' | 'REJECTED' | 'SUSPENDED';
export type PeerDecision = 'approve' | 'reject' | 'suspend' | 'reinstate';
export interface AdminPeer {
  id: string; full_name: string; email: string | null; phone_number: string; county: string;
  organization: string | null; approval_status: PeerApprovalStatus; approval_note: string | null;
  reviewed_at: string | null; created_at: string;
}
export async function getAdminPeers(): Promise<AdminPeer[]> {
  return request<AdminPeer[]>('/admin/peers');
}
/** A Peer Navigator with activity counts (Admin Peer Directory). */
export interface AdminPeerDirectoryRow extends AdminPeer {
  requests?: number; sessions_completed?: number; sessions_upcoming?: number;
  patients_supported?: number; chat_sessions?: number; call_sessions?: number;
  messages?: number; last_session_at?: string | null;
}
export async function getAdminPeerDirectory(): Promise<AdminPeerDirectoryRow[]> {
  return request<AdminPeerDirectoryRow[]>('/admin/peer-directory');
}
/** One support session; never the topic the patient wrote or any message. */
export interface AdminPeerSession {
  request_id: string; patient_id: string; patient_name: string;
  support_type: 'CHAT' | 'CALL'; status: 'PENDING' | 'ACCEPTED' | 'DECLINED' | 'CANCELLED' | 'COMPLETED';
  start_time: string; end_time: string; requested_at: string;
  decided_at: string | null; completed_at: string | null; cancelled_at: string | null;
  messages: number; call_started_at: string | null; call_ended_at: string | null;
}
export interface AdminPeerHistory {
  sessions: AdminPeerSession[];
  status_history: {event: string; at: string; by: string | null}[];
}
/** A patient's Peer Navigator sessions for their Admin record. */
export interface AdminPatientPeerSession {
  request_id: string; peer_id: string; peer_name: string; support_type: 'CHAT' | 'CALL';
  status: 'PENDING' | 'ACCEPTED' | 'DECLINED' | 'CANCELLED' | 'COMPLETED';
  start_time: string; requested_at: string; completed_at: string | null; messages: number;
}
export async function getAdminPatientPeerSessions(patientId: string): Promise<AdminPatientPeerSession[]> {
  return request<AdminPatientPeerSession[]>(`/admin/patients/${encodeURIComponent(patientId)}/peer-sessions`);
}
/** Records in the audit log that this Admin opened a record. Best effort. */
export type AuditedRecord = 'PATIENT' | 'DOCTOR_PROFILE' | 'PEER_NAVIGATOR';
const recentViews = new Map<string, number>();
export async function recordAdminView(resourceType: AuditedRecord, resourceId: string): Promise<void> {
  if (!/^[0-9a-f-]{36}$/i.test(resourceId)) return;
  // One row per opening: skip a repeat of the same view within 5 seconds
  // (development renders effects twice).
  const key = `${resourceType}:${resourceId}`; const now = Date.now();
  if (now - (recentViews.get(key) ?? 0) < 5000) return;
  recentViews.set(key, now);
  try {
    await request<unknown>('/admin/audit/views', {method: 'POST', body: JSON.stringify({resource_type: resourceType, resource_id: resourceId})});
  } catch { /* the record still opens; the server logs request failures */ }
}
export async function getAdminPeerHistory(id: string): Promise<AdminPeerHistory> {
  return request<AdminPeerHistory>(`/admin/peers/${encodeURIComponent(id)}/history`);
}
export async function decideAdminPeer(id: string, action: PeerDecision, note?: string): Promise<unknown> {
  return request<unknown>(`/admin/peers/${encodeURIComponent(id)}/${action}`, {
    method: 'POST',
    body: JSON.stringify(note && note.trim() ? {note: note.trim()} : {}),
  });
}
