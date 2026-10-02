export interface DoctorProfile {
  id: string;
  user_id: string;
  full_name: string;
  license_number: string;
  license_authority: string;
  license_country: string;
  primary_specialty: string;
  experience_years: number;
  current_hospital: string;
  qualifications: string[];
  consultation_fee_video: string;
  consultation_fee_chat: string;
  residential_address: string;
  verified_phone: string;
  is_on_duty: boolean;
  is_verified: boolean;
  disciplinary_warnings?: string[];
}

export interface DoctorWalletSummary {
  doctor_name: string;
  license_number: string;
  total_gross: string;
  platform_charge_percent: number;
  withheld_fee: string;
  final_net: string;
  pending_disbursement: string;
  basis_note: string;
}

export interface GrievanceReport {
  id: string;
  grievance_number: string;
  patient_id: string;
  consultation_id: string;
  target_type: 'DOCTOR' | 'SYSTEM';
  category: string;
  claim_summary: string;
  status: 'PENDING_REVIEW' | 'UNDER_INVESTIGATION' | 'REFUNDED' | 'WARNED' | 'DISMISSED';
  created_at: string;
  telemetry?: {
    call_duration_seconds: number;
    premature_end: boolean;
    packet_loss_percent?: string;
  };
}

export interface TransactionItem {
  id: string;
  transaction_number: string;
  appointment_id: string;
  payment_session_id: string;
  gateway: 'BKASH' | 'NAGAD' | 'CARD' | 'MPESA';
  gateway_reference?: string;
  gross_amount: string;
  platform_fee_amount: string;
  net_amount: string;
  payment_status: 'INITIATED' | 'PAYMENT_HELD' | 'SETTLED_TO_DOCTOR' | 'DISBURSED' | 'REFUNDED_TO_PATIENT' | 'FAILED';
  created_at: string;
}

export interface DisbursementBatch {
  id: string;
  batch_number: string;
  initiated_by: string;
  period_start: string;
  period_end: string;
  total_doctors: number;
  total_gross: string;
  total_platform_fee: string;
  total_net_disbursed: string;
  status: string;
  created_at: string;
}

export interface SlotItem {
  id: string;
  doctor_id: string;
  doctor_name: string;
  start_time: string;
  end_time: string;
  status: 'AVAILABLE' | 'LOCKED_IN_PAYMENT' | 'BOOKED' | 'BLOCKED' | 'COMPLETED';
  lock_session_id?: string;
  lock_expires_at?: string;
}

export interface PatientRecord {
  id: string;
  phone: string;
  display_name: string;
  gender: string;
  age: number;
  emergency_contact: string;
  registered_at: string;
  total_consultations: number;
  last_consultation_date: string;
}

export interface AuditEvent {
  id: string;
  actor_role: 'FINANCE_ADMIN' | 'CLINICAL_ADMIN' | 'PLATFORM_ADMIN' | 'SECURITY_ADMIN' | 'SYSTEM_WORKER';
  action: string;
  entity_type: string;
  entity_id: string;
  ip_address: string;
  rls_enforced: boolean;
  timestamp: string;
  details: string;
}

export interface ActiveRoom {
  channel_name: string;
  doctor_name: string;
  patient_phone: string;
  duration_seconds: number;
  modality: 'VIDEO' | 'AUDIO';
  video_packet_loss: string;
  bitrate_kbps: number;
}

export interface DoctorEncounter {
  time: string;
  ptName: string;
  mode: string;
  fee: string;
  diagnosis: string;
  rx: string;
}

export interface DoctorDisbursementRecord {
  date: string;
  amount: string;
  method: string;
  txId: string;
  commissionDeducted: string;
  status: string;
}

export interface DoctorHistoryDetail extends DoctorProfile {
  avatar: string;
  bgColor: string;
  email: string;
  completedVisits: number;
  chatSessions: number;
  grossEarnings: string;
  platformCut: string;
  netPayout: string;
  lifetimeConsultations: number;
  chatTotalSessions: number;
  lifetimeGross: string;
  lifetimeNet: string;
  payoutMethod: string;
  statusText: string;
  encounters: DoctorEncounter[];
  disbursements: DoctorDisbursementRecord[];
}

export interface PatientEncounter {
  date: string;
  doctor: string;
  spec: string;
  mode: string;
  diag: string;
  rx: string;
}

export interface PatientVaultDoc {
  name: string;
  meta: string;
}

export interface PatientHistoryDetail extends PatientRecord {
  avatar: string;
  bgColor: string;
  bp: string;
  pulse: string;
  cohort: string;
  labs: string;
  encounters: PatientEncounter[];
  vaultDocuments: PatientVaultDoc[];
}
