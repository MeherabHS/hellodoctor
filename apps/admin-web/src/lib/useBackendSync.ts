// useBackendSync.ts — Fetches live data from the Axum backend and converts
// it into the exact same shapes that page.tsx already consumes, so we can
// merge backend records into the stores WITHOUT touching any UI markup.

import { useEffect, useCallback, useRef } from 'react';
import type {
  DoctorHistoryItem,
  PatientRecordItem,
  AppErrorLogItem,
  GrievanceItem,
  TransactionItem,
} from '@/data/adminStore';
import {
  getAdminDoctors,
  getDoctorDossier,
  getAdminPatients,
  getPatientDossier,
  getAdminLogs,
  getAdminGrievances,
  getAdminTransactions,
} from '@/lib/api';

const AVATAR_COLORS = ['#059669', '#2563EB', '#7C3AED', '#D97706', '#DC2626', '#0891B2'];

export function backendDocToStoreItem(doc: any, idx: number): DoctorHistoryItem {
  const initials = (doc.full_name || 'Dr. Doctor')
    .replace(/^Dr\.\s*/, '')
    .split(' ')
    .map((w: string) => w[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);

  return {
    key: doc.id,
    name: doc.full_name,
    bmdc: doc.license_number,
    phone: doc.verified_phone,
    residence: doc.residential_address || 'Dhaka, Bangladesh',
    email: `${(doc.full_name || 'doctor').toLowerCase().replace(/[^a-z]/g, '.')}@helodoc.com`,
    avatar: initials,
    bgColor: AVATAR_COLORS[idx % AVATAR_COLORS.length],
    meta: `${doc.primary_specialty} • ${doc.current_hospital} • BMDC Registered Specialist`,
    kpiVisits: String(doc.consultations_count || 0),
    kpiChats: '0',
    kpiGross: `৳ ${Number(doc.lifetime_gross || 0).toLocaleString()}`,
    kpiDisbursed: `৳ ${Number(doc.lifetime_net || 0).toLocaleString()}`,
    encounters: (doc.encounters || []).map((enc: any) => ({
      time: enc.created_at ? new Date(enc.created_at).toLocaleString() : 'Recent',
      ptName: enc.patient_name || 'Patient',
      mode: enc.modality === 'VIDEO' ? '📹 10m Video Visit' : '💬 24h Chat',
      badgeCls: enc.modality === 'VIDEO' ? 'badge-green' : 'badge-blue',
      fee: `৳ ${enc.consultation_fee || enc.fee || 0}`,
      diagnosis: enc.clinical_outcome || 'Consultation',
      rx: enc.clinical_outcome === 'COMPLETED_WITH_RX' ? 'Rx Synced' : 'No Rx',
    })),
    disbursements: (doc.disbursements || []).map((d: any) => ({
      date: d.created_at ? new Date(d.created_at).toLocaleDateString() : 'Recent',
      amount: `৳ ${Number(d.net_amount || 0).toLocaleString()}`,
      method: 'bKash Merchant',
      txId: d.id ? d.id.slice(0, 10) : 'N/A',
      commission: `৳ ${Number(d.platform_fee || 0).toLocaleString()} (20%)`,
      status: d.status === 'COMPLETED' ? 'Settled ✓' : d.status || 'Pending',
    })),
  };
}

export function backendPatientToStoreItem(pt: any, idx: number): PatientRecordItem {
  const initials = (pt.full_name || 'Patient')
    .split(' ')
    .map((w: string) => w[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);

  return {
    key: pt.id,
    id: `#PT-${pt.id.slice(0, 5).toUpperCase()}`,
    name: pt.full_name,
    chatActive: true,
    avatar: initials,
    bgColor: AVATAR_COLORS[(idx + 2) % AVATAR_COLORS.length],
    meta: `${pt.date_of_birth || '30'} • ${pt.gender || 'F'} • ${pt.emergency_contact_phone || 'Dhaka'}`,
    bp: '120/80 mmHg',
    pulse: '76 bpm',
    cohort: pt.blood_group ? `Blood ${pt.blood_group}` : 'General Patient',
    labs: '0 Documents in Vault',
    category: 'general chat',
    encounters: (pt.encounters || []).map((enc: any) => ({
      date: enc.created_at ? new Date(enc.created_at).toLocaleDateString() : 'Recent',
      doctor: enc.doctor_name || 'Attending Physician',
      spec: 'Telemedicine',
      mode: enc.modality === 'VIDEO' ? '📹 Video Visit' : '💬 24h Chat',
      badgeCls: enc.modality === 'VIDEO' ? 'badge-green' : 'badge-blue',
      diag: enc.clinical_outcome || enc.status || 'General Consult',
      rx: enc.clinical_outcome === 'COMPLETED_WITH_RX' ? 'Rx Synced' : 'No Rx',
    })),
  };
}

export function backendLogToStoreItem(log: any): AppErrorLogItem {
  return {
    id: log.id,
    traceId: log.request_id || `TRC-${log.id.slice(0, 8).toUpperCase()}`,
    timestamp: log.created_at || new Date().toISOString(),
    timeFormatted: log.created_at ? new Date(log.created_at).toLocaleTimeString() : 'Just now',
    relativeTime: 'Live',
    user: {
      id: log.actor_id || 'USR-SYS',
      name: log.actor_role || 'System Service',
      ageGender: 'System',
      role: log.actor_role || 'PlatformAdmin',
      phone: '+880 16263',
      avatar: 'SYS',
      avatarBg: '#059669',
      device: 'Core Backend Cluster',
      appVersion: 'v2.4.0',
      network: 'Edge Uplink',
      ip: '127.0.0.1',
    },
    severity: log.result === 'FAILURE' || log.result === 'ERROR' ? 'CRITICAL' : 'WARNING',
    subsystem: log.resource_type || 'Core Platform',
    component: log.action || 'API Request',
    actionAttempted: log.action || 'System Audit Event',
    failedPart: log.resource_type || 'Subsystem',
    errorCode: log.result || 'OK',
    errorMessage: log.reason || 'Audit log event captured by backend telemetry',
    stackTrace: `Action: ${log.action}\nResource: ${log.resource_type}\nResult: ${log.result}`,
    status: log.result === 'FAILURE' ? 'UNRESOLVED' : 'RESOLVED',
    resolvedAt: null,
    resolutionNote: null,
  };
}

export function backendGrievanceToStoreItem(g: any): GrievanceItem {
  return {
    id: g.grievance_number || g.id,
    timestamp: g.created_at || new Date().toISOString(),
    timeFormatted: g.created_at ? new Date(g.created_at).toLocaleTimeString() : 'Today',
    relativeTime: 'Recent',
    patient: {
      id: g.patient_id,
      name: g.patient_name || 'Patient',
      phone: '+880 1711-000000',
      ageGender: 'Adult',
    },
    doctor: {
      id: g.consultation_id,
      name: g.doctor_name || 'Doctor',
      bmdc: 'BMDC Registered',
      specialty: 'Clinical Medicine',
      hospital: 'Hospital',
      fee: '৳800',
    },
    consultationId: g.consultation_id,
    consultationType: 'Video Consultation (10 min)',
    target: g.target_type || 'DOCTOR',
    category: g.category || 'Consultation Quality',
    severity: 'HIGH',
    claimSummary: g.claim_summary,
    patientStatement: g.claim_summary,
    boardRemedy:
      g.status === 'REFUNDED'
        ? 'Escrow Refund Disbursed'
        : g.status === 'WARNED'
        ? 'Internal Platform Compliance Warning Logged'
        : g.status === 'DISMISSED'
        ? 'Claim Reviewed and Dismissed'
        : g.status === 'UNDER_INVESTIGATION'
        ? 'Under Active Investigation'
        : 'Under Governance Review',
    telemetryEvidence: {
      callDuration: g.telemetry ? `${g.telemetry.duration_seconds}s` : 'Unknown',
      connectionStatus: g.telemetry?.connection_state || 'Connected',
      rxIssued: g.telemetry?.prescription_issued ? 'Rx Synced' : 'Not Issued',
      paymentStatus: 'Recorded',
      webrtcDiagnostics: {
        iceConnectionState: g.telemetry?.connection_state || 'Connected',
        audioQuality: 'Normal',
        videoQuality: 'Normal',
        packetLossPercent: `${g.telemetry?.packet_loss_percent || '0'}%`,
        networkPingRtt: `${g.telemetry?.rtt_ms || 24}ms`,
        jitter: '12ms',
        reconnectCount: 0,
      },
    },
    status:
      g.status === 'REFUNDED' || g.status === 'WARNED' || g.status === 'DISMISSED' || g.status === 'UNDER_INVESTIGATION'
        ? g.status
        : 'PENDING_REVIEW',
    adjudication: null,
  };
}

export function backendTransactionToStoreItem(tx: any): TransactionItem {
  const gross = Number(tx.gross_amount || 0);
  const fee = Number(tx.platform_fee_amount || 0);
  const net = Number(tx.net_amount || 0);

  return {
    txId: tx.transaction_number || tx.id,
    gatewayRef: tx.gateway_reference || 'GW-REF',
    timestamp: tx.created_at ? new Date(tx.created_at).toLocaleString() : 'Today',
    consultationId: tx.appointment_id || 'CONS',
    sessionType: 'Video Consultation',
    type: tx.payment_status === 'REFUNDED_TO_PATIENT' ? 'REFUND' : 'ESCROW',
    typeLabel: 'Consultation Inflow',
    gateway: tx.gateway === 'BKASH' ? 'bKash' : tx.gateway === 'NAGAD' ? 'Nagad' : 'Card',
    gatewayLabel: `${tx.gateway} Gateway`,
    patient: {
      name: tx.patient_name || 'Patient',
      phone: '+880 1711-000000',
      id: 'USR',
      avatar: 'PT',
      avatarBg: '#2563EB',
    },
    doctor: {
      name: tx.doctor_name || 'Physician',
      bmdc: 'BMDC Registered',
      specialty: 'Specialist',
      avatar: 'DR',
      avatarBg: '#059669',
    },
    grossAmount: gross,
    platformFeePercent: 20,
    platformFeeAmount: fee,
    netAmount: net,
    direction: tx.payment_status === 'REFUNDED_TO_PATIENT' ? 'REFUND' : 'INFLOW',
    escrowStatus: tx.payment_status === 'SETTLED_TO_DOCTOR' ? 'SETTLED' : 'ESCROW_LOCKED',
    escrowStatusLabel: tx.payment_status,
    statusBadgeCls: tx.payment_status === 'SETTLED_TO_DOCTOR' ? 'badge-green' : 'badge-amber',
    gatewayPayload: { tx_number: tx.transaction_number },
    auditTrail: [{ time: 'Recent', event: `Payment recorded via ${tx.gateway}` }],
  };
}

// ─── Main Hook ──────────────────────────────────────────────────────────────

interface BackendSyncCallbacks {
  onDoctorsUpdate: (merge: Record<string, DoctorHistoryItem>) => void;
  onPatientsUpdate: (merge: Record<string, PatientRecordItem>) => void;
  onLogsUpdate: (logs: AppErrorLogItem[]) => void;
  onGrievancesUpdate: (grvs: GrievanceItem[]) => void;
  onTransactionsUpdate: (txs: TransactionItem[]) => void;
}

export function useBackendSync(callbacks: BackendSyncCallbacks) {
  const cbRef = useRef(callbacks);
  cbRef.current = callbacks;

  const fetchAll = useCallback(async () => {
    try {
      // 1. Fetch doctors with dossier
      const backendDocs = await getAdminDoctors();
      const docStore: Record<string, DoctorHistoryItem> = {};
      for (let i = 0; i < backendDocs.length; i++) {
        const doc = backendDocs[i];
        try {
          const dossier = await getDoctorDossier(doc.id);
          docStore[doc.id] = backendDocToStoreItem({ ...doc, ...dossier }, i);
        } catch {
          docStore[doc.id] = backendDocToStoreItem(doc, i);
        }
      }
      // Replace (not merge) — once the backend answers, its data is the truth,
      // including a correctly-empty result, instead of layering onto mock seed data.
      cbRef.current.onDoctorsUpdate(docStore);

      // 2. Fetch patients
      const backendPts = await getAdminPatients();
      const ptStore: Record<string, PatientRecordItem> = {};
      for (let i = 0; i < backendPts.length; i++) {
        const pt = backendPts[i];
        try {
          const dossier = await getPatientDossier(pt.id);
          const merged = { ...pt, ...(dossier.patient || {}), encounters: dossier.encounters || [] };
          ptStore[pt.id] = backendPatientToStoreItem(merged, i);
        } catch {
          ptStore[pt.id] = backendPatientToStoreItem(pt, i);
        }
      }
      cbRef.current.onPatientsUpdate(ptStore);

      // 3. Fetch logs
      const backendLogs = await getAdminLogs();
      cbRef.current.onLogsUpdate(backendLogs.map(backendLogToStoreItem));

      // 4. Fetch grievances
      const backendGrvs = await getAdminGrievances();
      cbRef.current.onGrievancesUpdate(backendGrvs.map(backendGrievanceToStoreItem));

      // 5. Fetch transactions
      const backendTxs = await getAdminTransactions();
      cbRef.current.onTransactionsUpdate(backendTxs.map(backendTransactionToStoreItem));
    } catch (err) {
      // Backend not reachable at all — keep whatever is currently on screen
      // (mock data on first load, or the last good backend snapshot) instead of clearing it.
      console.warn('[useBackendSync] Backend unreachable, keeping last known data:', err);
    }
  }, []);

  useEffect(() => {
    fetchAll();
    const interval = setInterval(fetchAll, 15000);
    return () => clearInterval(interval);
  }, [fetchAll]);
}
