import { createInflightReads } from './inflight.mjs';
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
    | "FAILED";
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
  }>;
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
const inflightReads=createInflightReads();
export function clearAdminRuntime(){runtimeGeneration++;inflightReads.clear();}
function request<T>(path:string,options?:RequestInit):Promise<T>{
 return (options?.method??'GET').toUpperCase()==='GET'&&!options?.signal&&!options?.headers
  ? inflightReads.run(`${runtimeGeneration}:${path}`,()=>performRequest<T>(path,options)) : performRequest<T>(path,options);
}
async function performRequest<T>(path: string, options?: RequestInit): Promise<T> {
  const generation=runtimeGeneration;
  const url = `${API_BASE}${path}`;
  const res = await fetch(url, {
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
  outcome: "HARD_DELETED" | "ANONYMIZED";
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

export async function simulateAdminLog(payload: {
  action: string;
  subsystem: string;
  severity: string;
  error_summary: string;
  stack_trace?: string;
}): Promise<any> {
  return request<any>("/admin/logs/simulate", {
    method: "POST",
    body: JSON.stringify(payload),
  });
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

export async function simulateSystemError(payload: {
  scenario: string;
  severity: string;
  custom_note?: string;
  impacted_user_id?: string;
}): Promise<SystemErrorLog> {
  return request<SystemErrorLog>("/admin/system-errors/simulate", {
    method: "POST",
    body: JSON.stringify(payload),
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
