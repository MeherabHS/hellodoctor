// Type definitions shared between the live backend sync layer (useBackendSync.ts)
// and the admin portal UI (page.tsx). This file intentionally holds no mock/seed
// data — every admin-web record comes from the real backend at runtime.

export interface DoctorHistoryItem {
  key: string;
  name: string;
  registrationNumber: string;
  professionalVerificationStatus?: "PENDING" | "VERIFIED" | "REJECTED";
  phone: string;
  residence: string;
  email: string;
  avatar: string;
  bgColor: string;
  meta: string;
  kpiVisits: string;
  kpiChats: string;
  kpiGross: string;
  kpiPlatformCut?: string;
  kpiDisbursed: string;
  pendingDisbursement?: number;
  payoutRouting?: {
    statementPeriod: string;
    currency: string;
    finalPayable: string;
    gross?: string;
    platformCharge?: string;
    provider: string | null;
    maskedDestination: string | null;
    readiness: string;
    statementStatus: string;
    disbursementStatus: string;
    disbursementItemId?: string | null;
  };
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

export interface PatientRecordItem {
  key: string;
  name: string;
  id: string;
  chatActive: boolean;
  avatar: string;
  bgColor: string;
  meta: string;
  cohort: string;
  labs: string;
  category: string;
  videoVisits?: number;
  chatSessions?: number;
  totalSpend?: number;
  vaultRecords?: {
    id: string;
    appointmentId: string;
    mimeType: string;
    fileSizeBytes: number;
    securityStatus: string;
    createdAt: string;
  }[];
  encounters: {
    date: string;
    doctor: string;
    spec: string;
    mode: string;
    badgeCls: string;
    diag: string;
    rx: string;
  }[];
}

export interface AppErrorLogItem {
  id: string;
  traceId: string;
  timestamp: string;
  timeFormatted: string;
  relativeTime: string;
  user: {
    id: string;
    name: string;
    ageGender: string;
    role: string;
    phone: string;
    avatar: string;
    avatarBg: string;
    device: string;
    appVersion: string;
    network: string;
    ip: string;
  };
  severity: 'INFO' | 'WARNING' | 'ERROR' | 'CRITICAL';
  subsystem: string;
  component: string;
  actionAttempted: string;
  failedPart: string;
  errorCode: string;
  errorMessage: string;
  stackTrace: string;
  status: 'UNRESOLVED' | 'INVESTIGATING' | 'RESOLVED';
  resolvedAt: string | null;
  resolutionNote: string | null;
  occurrenceCount: number;
  httpStatus: number | null;
  endpoint: string | null;
  fingerprint: string | null;
}

export interface GrievanceItem {
  id: string;
  backendId?: string;
  timestamp: string;
  timeFormatted: string;
  relativeTime: string;
  patient: {
    id: string;
    name: string;
    phone: string;
    ageGender: string;
  };
  doctor: {
    id: string;
    name: string;
    registrationNumber: string;
    specialty: string;
    hospital: string;
    fee: string;
  };
  consultationId: string;
  consultationType: string;
  target: 'DOCTOR' | 'SYSTEM';
  category: string;
  severity: 'CRITICAL' | 'HIGH' | 'MEDIUM' | 'UNASSESSED';
  claimSummary: string;
  patientStatement: string;
  boardRemedy: string;
  telemetryEvidence: {
    callDuration: string;
    connectionStatus: string;
    rxIssued: string;
    paymentStatus: string;
    webrtcDiagnostics?: {
      iceConnectionState: string;
      audioQuality: string;
      videoQuality: string;
      packetLossPercent: string;
      networkPingRtt: string;
      jitter: string;
      reconnectCount: number | null;
    };
    deviceInfo?: string;
    escrowStatus?: string;
    auditTimeline?: { time: string; event: string }[];
  };
  status: 'PENDING_REVIEW' | 'UNDER_INVESTIGATION' | 'REFUND_APPROVED' | 'REFUNDED' | 'WARNED' | 'DISMISSED';
  adjudication: {
    action: string;
    date: string;
    admin: string;
    notes: string;
  } | null;
}

export interface TransactionItem {
  txId: string;
  gatewayRef: string;
  timestamp: string;
  consultationId: string;
  sessionType: string;
  type: 'INFLOW' | 'ESCROW' | 'PAYOUT' | 'REFUND';
  typeLabel: string;
  gateway: 'bKash' | 'Nagad' | 'Card' | 'MFS Bulk API';
  gatewayLabel: string;
  patient: { name: string; phone: string; id: string; avatar: string; avatarBg: string };
  doctor: { name: string; registrationNumber: string; specialty: string; avatar: string; avatarBg: string };
  grossAmount: number;
  platformFeePercent: number;
  platformFeeAmount: number;
  netAmount: number;
  direction: 'INFLOW' | 'PAYOUT' | 'REFUND';
  escrowStatus: 'ESCROW_LOCKED' | 'SETTLED' | 'REFUNDED';
  escrowStatusLabel: string;
  statusBadgeCls: string;
  gatewayPayload: Record<string, any>;
  auditTrail: { time: string; event: string }[];
}
