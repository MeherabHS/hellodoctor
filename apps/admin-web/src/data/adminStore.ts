// Types and initial empty state stores for HelloDoctor Admin Portal
// Connected to Axum Rust backend - no hardcoded mock users.

export interface DoctorHistoryItem {
  key: string;
  name: string;
  bmdc: string;
  phone: string;
  residence: string;
  email: string;
  avatar: string;
  bgColor: string;
  meta: string;
  kpiVisits: string;
  kpiChats: string;
  kpiGross: string;
  kpiDisbursed: string;
  encounters: {
    time: string;
    ptName: string;
    mode: string;
    badgeCls: string;
    fee: string;
    diagnosis: string;
    rx: string;
  }[];
  disbursements: {
    date: string;
    amount: string;
    method: string;
    txId: string;
    commission: string;
    status: string;
  }[];
}

export interface PatientDossierItem {
  key: string;
  name: string;
  ageGender: string;
  phone: string;
  nid: string;
  blood: string;
  heightWeight: string;
  allergies: string;
  emergency: string;
  avatar: string;
  cohortBadge: string;
  encounters: {
    time: string;
    doctorName: string;
    mode: string;
    badgeCls: string;
    fee: string;
    chiefComplaint: string;
    rxNo: string;
  }[];
  vault: {
    title: string;
    meta: string;
    size: string;
  }[];
}

export interface ErrorLogItem {
  id: string;
  traceId: string;
  subsystem: string;
  time: string;
  badgeCls: string;
  badgeText: string;
  errCode: string;
  summary: string;
  stackTrace: string;
  resolved: boolean;
}

export interface GrievanceItem {
  id: string;
  code: string;
  patient: string;
  doctor: string;
  catCls: string;
  catText: string;
  channel: string;
  claim: string;
  statusCls: string;
  statusText: string;
  telemetry: {
    duration: string;
    packetLoss: string;
    audioStream: string;
    bandwidth: string;
    callState: string;
  };
  details: {
    filedAt: string;
    ptStatement: string;
    adjudicationNote: string;
  };
}

export interface AdminTransactionItem {
  id: string;
  txId: string;
  badgeCls: string;
  method: string;
  user: string;
  phone: string;
  type: string;
  gross: string;
  fee: string;
  net: string;
  date: string;
  statusCls: string;
  statusText: string;
  payer: string;
  recipient: string;
  service: string;
  notes: string;
  jsonLog: string;
}

// Initial Empty Stores (Backend empty state on start)
export const ADMIN_DOCTORS_STORE: Record<string, DoctorHistoryItem> = {};
export const ADMIN_PATIENT_STORE: Record<string, PatientDossierItem> = {};
export const INITIAL_ERROR_LOGS: ErrorLogItem[] = [];
export const INITIAL_GRIEVANCES: GrievanceItem[] = [];
export const INITIAL_TRANSACTIONS: AdminTransactionItem[] = [];
