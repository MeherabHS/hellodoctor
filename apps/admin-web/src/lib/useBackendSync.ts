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
  getSystemErrors,
  getAdminGrievances,
  getAdminTransactions,
  getAdminOverview,
  getFinanceSummary,
  getPayoutRouting,
  getCommandTelemetry,
  getPendingDoctorVerifications,
  getComplianceAlerts,
  getRefundRequests,
  AdminOverview,
  FinanceSummary,
  CommandTelemetry,
  PendingDoctorVerification,
  ComplianceAlert,
  SystemErrorLog,
  FinancialRefundRequest,
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

  const encounters: any[] = doc.encounters || [];
  const videoCount = encounters.filter((e) => e.modality === 'VIDEO').length;
  const chatCount = encounters.filter((e) => e.modality === 'CHAT').length;

  return {
    key: doc.id,
    name: doc.full_name,
    professionalVerificationStatus: doc.professional_verification_status,
    registrationNumber: doc.license_number,
    phone: doc.verified_phone,
    residence: doc.residential_address || 'Not provided',
    email: doc.email || 'Not provided',
    avatar: initials,
    bgColor: AVATAR_COLORS[idx % AVATAR_COLORS.length],
    meta: `${doc.primary_specialty} • ${doc.current_hospital} • KMPDC registration`,
    kpiVisits: String(videoCount),
    kpiChats: String(chatCount),
    kpiGross: `KES  ${Number(doc.lifetime_gross || 0).toLocaleString()}`,
    kpiPlatformCut: `KES  ${Number(doc.lifetime_fee_withheld || 0).toLocaleString()}`,
    kpiDisbursed: `${doc.currency || doc.wallet?.currency || 'KES'} ${Number(doc.total_paid || doc.wallet?.total_paid || 0).toLocaleString()}`,
    pendingDisbursement: Number(doc.pending_disbursement || 0),
    payoutRouting: doc.payout_routing ? {
      statementPeriod: doc.payout_routing.statement_period,
      currency: doc.payout_routing.currency,
      finalPayable: doc.payout_routing.final_payable,
      gross: doc.payout_routing.gross_eligible_earnings,
      platformCharge: doc.payout_routing.platform_charge,
      provider: doc.payout_routing.provider_code || null,
      maskedDestination: doc.payout_routing.masked_destination || null,
      readiness: doc.payout_routing.payout_readiness || 'PAYOUT_SETUP_REQUIRED',
      statementStatus: doc.payout_routing.statement_status || 'OPEN',
      disbursementStatus: doc.payout_routing.disbursement_status || 'NOT_ALLOCATED',
      disbursementItemId: doc.payout_routing.disbursement_item_id || null,
    } : undefined,
    encounters: encounters.map((enc: any) => ({
      time: enc.created_at ? new Date(enc.created_at).toLocaleString() : 'Recent',
      ptName: enc.patient_name || 'Patient',
      mode: enc.modality === 'VIDEO' ? '📹 10m Video Visit' : '💬 24h Chat',
      badgeCls: enc.modality === 'VIDEO' ? 'badge-green' : 'badge-blue',
      fee: `KES  ${enc.consultation_fee || enc.fee || 0}`,
      diagnosis: enc.clinical_outcome || 'Consultation',
      rx: enc.clinical_outcome === 'COMPLETED_WITH_RX' ? 'Rx Synced' : 'No Rx',
    })),
    disbursements: (doc.disbursements || []).map((d: any) => ({
      date: d.created_at ? new Date(d.created_at).toLocaleDateString() : 'Recent',
      amount: `KES  ${Number(d.net_amount || 0).toLocaleString()}`,
      method: `${d.provider_code || 'Provider not connected'} ${d.masked_destination_snapshot || ''}`,
      txId: d.id ? d.id.slice(0, 10) : 'N/A',
      commission: `KES  ${Number(d.platform_fee || 0).toLocaleString()} (20%)`,
      status: d.status || 'Pending',
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

  const encounters: any[] = pt.encounters || [];
  const videoVisits = encounters.filter((e) => e.modality === 'VIDEO').length;
  const chatSessions = encounters.filter((e) => e.modality === 'CHAT').length;
  const totalSpend = encounters.reduce((sum, e) => sum + Number(e.fee ?? e.consultation_fee ?? 0), 0);

  return {
    key: pt.id,
    id: `#PT-${pt.id.slice(0, 5).toUpperCase()}`,
    name: pt.full_name,
    chatActive: chatSessions > 0,
    avatar: initials,
    bgColor: AVATAR_COLORS[(idx + 2) % AVATAR_COLORS.length],
    meta: `${pt.date_of_birth || '30'} • ${pt.gender || 'F'} • ${pt.emergency_contact_phone || 'Kenya'}`,
    cohort: pt.blood_group ? `Blood ${pt.blood_group}` : 'General Patient',
    labs: `${(pt.vault_records || []).length} Documents in Vault`,
    category: 'general chat',
    videoVisits,
    chatSessions,
    totalSpend,
    encounters: encounters.map((enc: any) => ({
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

// Maps the backend's dedicated `system_error_logs` row (genuine crashes and
// exceptions — see `services/backend/src/system_error.rs`) into the shape
// the "App Error & Incident Logs" admin page already renders. Deliberately
// NOT derived from `/admin/logs` (the separate `audit_events` administrator
// action trail) — the two domains are kept apart end to end, per
// `docs/CHECKPOINT.md` §8's explicit "never merge the two domains" rule.
// `subsystem`/`component` are the panel's "codename" fields; `stack_trace`
// (when the backend could capture one via `#[track_caller]`) is the "code
// snippet" field.
export function backendSystemErrorToStoreItem(log: SystemErrorLog): AppErrorLogItem {
  const statusMap: Record<SystemErrorLog['status'], AppErrorLogItem['status']> = {
    OPEN: 'UNRESOLVED',
    ACKNOWLEDGED: 'INVESTIGATING',
    RESOLVED: 'RESOLVED',
    IGNORED: 'RESOLVED',
  };
  return {
    id: log.id,
    traceId: log.trace_id,
    timestamp: log.logged_at,
    timeFormatted: new Date(log.logged_at).toLocaleTimeString(),
    relativeTime: log.occurrence_count > 1 ? `${log.occurrence_count}x, last ${new Date(log.last_seen_at).toLocaleString()}` : 'Live',
    user: {
      id: log.impacted_user_id || 'USR-SYS',
      name: log.impacted_user_id ? `User ${log.impacted_user_id.slice(0, 8)}` : 'System / unauthenticated',
      ageGender: 'N/A',
      role: log.client_context ? 'Doctor' : 'N/A',
      phone: 'Not provided',
      avatar: log.impacted_user_id ? 'U' : 'SYS',
      avatarBg: log.impacted_user_id ? '#2563EB' : '#059669',
      device: log.client_context ? `${log.client_context.client} / ${log.client_context.platform}` : 'Core Backend Cluster',
      appVersion: log.client_context ? `${log.client_context.app_version} (build ${log.client_context.build_number})` : 'Not captured',
      network: 'Not captured',
      ip: 'Not captured',
    },
    severity: log.severity,
    subsystem: log.subsystem,
    component: log.component,
    actionAttempted: log.action_attempted,
    failedPart: log.client_context ? `${log.client_context.screen} / ${log.client_context.operation}` : log.endpoint || log.subsystem,
    errorCode: log.error_code || log.status,
    errorMessage: log.error_message + (log.client_context?.source === 'deferred_startup_report' ? ' Deferred startup report; associated doctor is the uploader.' : '') + (typeof log.client_context?.startup_reference === 'string' ? ` Startup reference: ${log.client_context.startup_reference}` : ''),
    stackTrace: log.stack_trace || '(no source-code location was captured for this incident)',
    status: statusMap[log.status],
    resolvedAt: log.resolved_at,
    resolutionNote: log.resolution_note,
    occurrenceCount: log.occurrence_count,
    httpStatus: log.http_status,
    endpoint: log.endpoint,
    fingerprint: log.fingerprint,
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
      phone: 'Not provided',
      ageGender: 'Adult',
    },
    doctor: {
      id: g.consultation_id,
      name: g.doctor_name || 'Doctor',
      registrationNumber: g.doctor_license || 'Not provided',
      specialty: 'Clinical Medicine',
      hospital: 'Hospital',
      fee: 'Not supplied by this API',
    },
    consultationId: g.consultation_id,
    consultationType: 'Video Consultation (10 min)',
    target: g.target_type || 'DOCTOR',
    category: g.category || 'Consultation Quality',
    severity: 'HIGH',
    claimSummary: g.claim_summary,
    patientStatement: g.claim_summary,
    boardRemedy:
      g.status === 'REFUND_APPROVED'
        ? 'Refund approved — provider execution pending'
        : g.status === 'REFUNDED'
        ? 'Refund confirmed'
        : g.status === 'WARNED'
        ? 'Internal Platform Compliance Warning Logged'
        : g.status === 'DISMISSED'
        ? 'Claim Reviewed and Dismissed'
        : g.status === 'UNDER_INVESTIGATION'
        ? 'Under Active Investigation'
        : 'Under Governance Review',
    telemetryEvidence: {
      callDuration: g.telemetry ? `${g.telemetry.duration_seconds}s` : 'Unknown',
      connectionStatus: g.telemetry?.connection_state || 'Unmeasured',
      rxIssued: g.telemetry ? (g.telemetry.prescription_issued ? 'Rx Synced' : 'Not Issued') : 'Unmeasured',
      paymentStatus: 'Recorded',
      webrtcDiagnostics: {
        iceConnectionState: g.telemetry?.connection_state || 'Unmeasured',
        audioQuality: 'Unmeasured',
        videoQuality: 'Unmeasured',
        packetLossPercent: g.telemetry?.packet_loss_percent == null ? 'Unmeasured' : `${g.telemetry.packet_loss_percent}%`,
        networkPingRtt: g.telemetry?.rtt_ms == null ? 'Unmeasured' : `${g.telemetry.rtt_ms}ms`,
        jitter: 'Unmeasured',
        reconnectCount: null,
      },
    },
    status:
      g.status === 'REFUND_APPROVED' || g.status === 'REFUNDED' || g.status === 'WARNED' || g.status === 'DISMISSED' || g.status === 'UNDER_INVESTIGATION'
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
    gatewayRef: tx.gateway_reference || 'No provider reference',
    timestamp: tx.created_at ? new Date(tx.created_at).toLocaleString() : 'Today',
    consultationId: tx.appointment_id || 'CONS',
    sessionType: 'Video Consultation',
    type: tx.payment_status === 'REFUNDED_TO_PATIENT' ? 'REFUND' : 'ESCROW',
    typeLabel: 'Consultation Inflow',
    gateway: tx.gateway === 'MPESA' ? 'MFS Bulk API' : tx.gateway === 'CARD' ? 'Card' : 'MFS Bulk API',
    gatewayLabel: tx.gateway === 'MPESA' ? 'M-Pesa provider' : tx.gateway === 'BANK_TRANSFER' ? 'Kenyan bank provider' : tx.gateway === 'CARD' ? 'Card provider' : 'Provider deferred',
    patient: {
      name: tx.patient_name || 'Patient',
      phone: 'Not provided',
      id: 'USR',
      avatar: 'PT',
      avatarBg: '#2563EB',
    },
    doctor: {
      name: tx.doctor_name || 'Physician',
      registrationNumber: tx.doctor_license || 'Not provided',
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

export interface ModalitySplit {
  videoRevenue: number;
  chatRevenue: number;
  videoCount: number;
  chatCount: number;
}

// ─── Main Hook ──────────────────────────────────────────────────────────────

interface BackendSyncCallbacks {
  onDoctorsUpdate: (merge: Record<string, DoctorHistoryItem>) => void;
  onPatientsUpdate: (merge: Record<string, PatientRecordItem>) => void;
  onLogsUpdate: (logs: AppErrorLogItem[]) => void;
  onGrievancesUpdate: (grvs: GrievanceItem[]) => void;
  onTransactionsUpdate: (txs: TransactionItem[]) => void;
  onOverviewUpdate: (overview: AdminOverview) => void;
  onFinanceSummaryUpdate: (summary: FinanceSummary) => void;
  onCommandTelemetryUpdate: (telemetry: CommandTelemetry) => void;
  onVerificationQueueUpdate: (doctors: PendingDoctorVerification[]) => void;
  onModalitySplitUpdate: (split: ModalitySplit) => void;
  onComplianceAlertsUpdate: (alerts: ComplianceAlert[]) => void;
  onRefundRequestsUpdate: (requests: FinancialRefundRequest[]) => void;
}

export function useBackendSync(callbacks: BackendSyncCallbacks, statementMonth: string) {
  const cbRef = useRef(callbacks);
  cbRef.current = callbacks;
  const criticalEpoch = useRef(0);
  const activeMonthRef = useRef(statementMonth);
  activeMonthRef.current = statementMonth;

  const fetchAll = useCallback(async () => {
    const criticalJob = ++criticalEpoch.current;
    // Incidents must not depend on unrelated Finance/dossier requests succeeding.
    // Scheduling has a separate lifecycle in useSlotSync.
    await Promise.allSettled([
      getSystemErrors().then(feed => { if (criticalJob === criticalEpoch.current) cbRef.current.onLogsUpdate(feed.items.map(backendSystemErrorToStoreItem)); }),
    ]);
    try {
      // 1. Fetch doctors with dossier
      let backendDocs;
      try {
        backendDocs = await getAdminDoctors();
      } catch {
        // Finance has no credential-management permission. Its dashboard is
        // built from the safe financial read model, without broadening PHI access.
        const routing = await getPayoutRouting(statementMonth);
        if (activeMonthRef.current !== statementMonth) return;
        const financeDoctors: Record<string, DoctorHistoryItem> = {};
        routing.forEach((row, idx) => {
          financeDoctors[row.doctor_id] = backendDocToStoreItem({
            id: row.doctor_id, full_name: row.doctor, license_number: row.license,
            verified_phone: 'Not available in Finance view',
            primary_specialty: 'Financial record', current_hospital: '',
            payout_routing: row,
          }, idx);
        });
        cbRef.current.onDoctorsUpdate(financeDoctors);
        cbRef.current.onFinanceSummaryUpdate(await getFinanceSummary(statementMonth));
        cbRef.current.onTransactionsUpdate((await getAdminTransactions(statementMonth)).map(backendTransactionToStoreItem));
        try {
          cbRef.current.onRefundRequestsUpdate(await getRefundRequests());
        } catch { /* FinanceAdmin should always have this permission; degrade quietly if not */ }
        return;
      }
      const docStore: Record<string, DoctorHistoryItem> = {};
      let payoutRouting: any[] = [];
      try {
        payoutRouting = await getPayoutRouting(statementMonth);
      } catch { /* the doctor dossier remains usable if Finance routing is unavailable */ }
      const routingByDoctor = new Map(payoutRouting.map((r: any) => [r.doctor_id, r]));
      let videoRevenue = 0;
      let chatRevenue = 0;
      let videoCount = 0;
      let chatCount = 0;

      for (let i = 0; i < backendDocs.length; i++) {
        const doc = backendDocs[i];
        try {
          const dossier = await getDoctorDossier(doc.id);
          for (const enc of dossier.encounters || []) {
            const fee = Number((enc as any).consultation_fee || 0);
            if (enc.modality === 'VIDEO') {
              videoRevenue += fee;
              videoCount += 1;
            } else if (enc.modality === 'CHAT') {
              chatRevenue += fee;
              chatCount += 1;
            }
          }
          docStore[doc.id] = backendDocToStoreItem({ ...doc, ...dossier, payout_routing: routingByDoctor.get(doc.id) }, i);
        } catch {
          docStore[doc.id] = backendDocToStoreItem({ ...doc, payout_routing: routingByDoctor.get(doc.id) }, i);
        }
      }
      // Replace (not merge) — once the backend answers, its data is the truth,
      // including a correctly-empty result, instead of layering onto mock seed data.
      if (activeMonthRef.current !== statementMonth) return;
      cbRef.current.onDoctorsUpdate(docStore);
      cbRef.current.onModalitySplitUpdate({ videoRevenue, chatRevenue, videoCount, chatCount });

      // 2. Fetch patients
      const backendPts = await getAdminPatients();
      const ptStore: Record<string, PatientRecordItem> = {};
      for (let i = 0; i < backendPts.length; i++) {
        const pt = backendPts[i];
        try {
          const dossier = await getPatientDossier(pt.id);
          const merged = { ...pt, ...(dossier.patient || {}), encounters: dossier.encounters || [], vault_records: dossier.vault_records || [] };
          ptStore[pt.id] = backendPatientToStoreItem(merged, i);
        } catch {
          ptStore[pt.id] = backendPatientToStoreItem(pt, i);
        }
      }
      cbRef.current.onPatientsUpdate(ptStore);

      // 3. Fetch system-error incidents (the dedicated error/crash log, not
      // the audit trail — see backendSystemErrorToStoreItem's doc comment).
      // The independent critical reads above already refreshed incidents.

      // 4. Fetch grievances
      const backendGrvs = await getAdminGrievances();
      cbRef.current.onGrievancesUpdate(backendGrvs.map(backendGrievanceToStoreItem));

      // 5. Fetch transactions (month-filtered, matching the Finance page's
      // own statement-month picker — this previously ignored the picker
      // entirely and always showed all-time data).
      const backendTxs = await getAdminTransactions(statementMonth);
      cbRef.current.onTransactionsUpdate(backendTxs.map(backendTransactionToStoreItem));

      // 6. Fetch platform-wide aggregates
      cbRef.current.onOverviewUpdate(await getAdminOverview());
      cbRef.current.onFinanceSummaryUpdate(await getFinanceSummary(statementMonth));
      cbRef.current.onCommandTelemetryUpdate(await getCommandTelemetry());
      cbRef.current.onVerificationQueueUpdate(await getPendingDoctorVerifications());
      cbRef.current.onComplianceAlertsUpdate(await getComplianceAlerts());

      // 7. Fetch Finance refund requests (post-grievance-approval lifecycle —
      // not month-scoped; a refund stays actionable until resolved).
      cbRef.current.onRefundRequestsUpdate(await getRefundRequests());
    } catch (err) {
      // Keep the last successful backend snapshot when another dashboard read fails.
      console.warn('[useBackendSync] Backend unreachable, keeping last known data:', err);
    }
  }, [statementMonth]);

  useEffect(() => {
    fetchAll();
    const interval = setInterval(fetchAll, 15000);
    return () => { criticalEpoch.current++; clearInterval(interval); };
  }, [fetchAll]);
  return fetchAll;
}
