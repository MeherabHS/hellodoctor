// API client connecting HelloDoctor Admin Portal to the Axum Rust Backend

export const API_BASE =
  process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080/api/v1";

export interface ApiResponse<T> {
  success: boolean;
  data: T;
  error?: string;
}

export interface AdminOverview {
  total_doctors: number;
  on_duty_doctors: number;
  pending_bmdc: number;
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
  is_verified: boolean;
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
  is_verified: boolean;
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
  gateway: "BKASH" | "NAGAD" | "CARD" | "MPESA";
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
  gateway_volume: {
    bkash: string;
    nagad: string;
    card: string;
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
    packet_loss_percent: string;
    rtt_ms: number;
    connection_state: string;
  }>;
}

export interface AdminSlot {
  id: string;
  doctor_id: string;
  doctor_name: string;
  start_time: string;
  end_time: string;
  status: "AVAILABLE" | "LOCKED_IN_PAYMENT" | "BOOKED" | "CANCELLED";
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
    page_number: number;
    mime_type: string;
    file_size_bytes: number;
    created_at: string;
  }>;
}

export interface BmdcDoctor {
  id: string;
  full_name: string;
  license_number: string;
  primary_specialty: string;
  experience_years: number;
  current_hospital: string;
  verified_phone: string;
  is_verified: boolean;
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
    packet_loss_percent: string;
    rtt_ms: number;
    connection_state: string;
    premature_end: boolean;
    prescription_issued: boolean;
  } | null;
}

// Network Request Helper
async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const url = `${API_BASE}${path}`;
  const res = await fetch(url, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options?.headers || {}),
    },
    cache: "no-store",
  });

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

export async function getAdminTransactions(): Promise<AdminTransaction[]> {
  return request<AdminTransaction[]>("/admin/finance/transactions");
}

export async function getFinanceSummary(): Promise<FinanceSummary> {
  return request<FinanceSummary>("/admin/finance/summary");
}

export async function getCommandTelemetry(): Promise<CommandTelemetry> {
  return request<CommandTelemetry>("/admin/command/telemetry");
}

export async function getAdminSlots(): Promise<AdminSlot[]> {
  return request<AdminSlot[]>("/admin/slots");
}

export async function toggleSlot(slot_id: string): Promise<any> {
  return request<any>("/admin/slots", {
    method: "POST",
    body: JSON.stringify({ slot_id }),
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

export async function getBmdcQueue(): Promise<BmdcDoctor[]> {
  return request<BmdcDoctor[]>("/admin/bmdc/queue");
}

export async function verifyBmdcDoctor(id: string, note?: string): Promise<any> {
  return request<any>(`/admin/bmdc/${id}/verify`, {
    method: "POST",
    body: JSON.stringify({ note: note || "Verified by medical administrator" }),
  });
}

export async function rejectBmdcDoctor(id: string, note?: string): Promise<any> {
  return request<any>(`/admin/bmdc/${id}/reject`, {
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
