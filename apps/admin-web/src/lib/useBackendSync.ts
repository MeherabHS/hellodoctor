// useBackendSync.ts — Fetches live data from the Axum backend and converts
// it into the exact same shapes that page.tsx already consumes, so we can
// merge backend records into the stores WITHOUT touching any UI markup.

import { useEffect, useCallback, useRef } from 'react';
import { startScheduleSync } from './polling';
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
    kpiVisits: String(doc.video_count ?? videoCount),
    kpiChats: String(doc.chat_count ?? chatCount),
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
    chatActive: (pt.chat_count ?? chatSessions) > 0,
    avatar: initials,
    bgColor: AVATAR_COLORS[(idx + 2) % AVATAR_COLORS.length],
    meta: `${pt.date_of_birth || '30'} • ${pt.gender || 'F'} • ${pt.emergency_contact_phone || 'Kenya'}`,
    cohort: pt.blood_group ? `Blood ${pt.blood_group}` : 'General Patient',
    labs: `${(pt.vault_records || []).length} Documents in Vault`,
    vaultRecords: (pt.vault_records || []).map((v: any) => ({
      id: v.id,
      appointmentId: v.appointment_id,
      mimeType: v.mime_type,
      fileSizeBytes: v.file_size_bytes,
      securityStatus: v.security_status,
      createdAt: v.created_at,
    })),
    category: 'general chat',
    videoVisits: pt.video_count ?? videoVisits,
    chatSessions: pt.chat_count ?? chatSessions,
    totalSpend: Number(pt.consultation_fee_total ?? totalSpend),
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
    backendId: g.id,
    timestamp: g.created_at || '',
    timeFormatted: g.created_at ? new Date(g.created_at).toLocaleTimeString() : 'Not supplied',
    relativeTime: g.created_at ? new Date(g.created_at).toLocaleDateString() : 'Not supplied',
    patient: {
      id: g.patient_id,
      name: g.patient_name || 'Not supplied',
      phone: 'Not provided',
      ageGender: 'Demographics not supplied',
    },
    doctor: {
      id: g.doctor_id || 'Not supplied',
      name: g.doctor_name || 'Not supplied',
      registrationNumber: g.doctor_license || 'Not provided',
      specialty: g.doctor_specialty || 'Not supplied',
      hospital: g.doctor_hospital || 'Not supplied',
      fee: g.consultation_fee == null ? 'Not supplied' : `KES ${g.consultation_fee}`,
    },
    consultationId: g.consultation_id,
    consultationType: g.modality === 'CHAT' ? 'Chat consultation' : g.modality === 'VIDEO' ? 'Video consultation' : 'Consultation',
    target: g.target_type || 'DOCTOR',
    category: g.category || 'Not supplied',
    severity: ['CRITICAL', 'HIGH', 'MEDIUM'].includes(g.severity) ? g.severity : 'UNASSESSED',
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
      callDuration: 'Media not implemented',
      connectionStatus: 'No provider evidence',
      rxIssued: 'See prescription records in investigation timeline',
      paymentStatus: g.payment_status || 'See investigation timeline',
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
    adjudication: g.adjudication || null,
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

export function useBackendSync(callbacks:BackendSyncCallbacks,statementMonth:string,activePage='doctors',selectedDoctor:string|null=null,selectedPatient:string|null=null) {
 const cbRef=useRef(callbacks);
 useEffect(()=>{cbRef.current=callbacks;},[callbacks]);
 const epoch=useRef(0),pending=useRef<Promise<void>|null>(null),roundFailed=useRef(false);
 const rawDoctors=useRef<Record<string,any>>({}),rawPatients=useRef<Record<string,any>>({});
 const doctors=useRef<Record<string,DoctorHistoryItem>>({}),patients=useRef<Record<string,PatientRecordItem>>({});
 const fetchPage=useCallback(()=>{
  if(document.visibilityState!=='visible'||!navigator.onLine)return Promise.resolve();
  if(pending.current)return pending.current;
  const version=epoch.current;
  roundFailed.current=false;
  const apply=(job:()=>Promise<void>)=>job().catch(()=>{ roundFailed.current=true; /* Keep the last successful snapshot; protected actions still reauthorize. */ });
  const current=()=>version===epoch.current;
  const jobs:Array<()=>Promise<void>>=[];
  if(['doctors','finance','command'].includes(activePage))jobs.push(async()=>{
   const overview=await getAdminOverview();if(!current())return;
   cbRef.current.onOverviewUpdate(overview);
   if(overview.modality_split)cbRef.current.onModalitySplitUpdate({videoCount:overview.modality_split.videoCount,chatCount:overview.modality_split.chatCount,videoRevenue:Number(overview.modality_split.videoRevenue),chatRevenue:Number(overview.modality_split.chatRevenue)});
  });
  if(activePage==='doctors')jobs.push(async()=>{
   const [rows,routing]=await Promise.all([getAdminDoctors(),getPayoutRouting(statementMonth)]);if(!current())return;
   const routingByDoctor=new Map(routing.map(r=>[r.doctor_id,r]));
   rawDoctors.current=Object.fromEntries(rows.map(row=>[row.id,{...row,payout_routing:routingByDoctor.get(row.id)}]));
   doctors.current=Object.fromEntries(rows.map((row,index)=>{const mapped=backendDocToStoreItem(rawDoctors.current[row.id],index);return [row.id,{...mapped,encounters:doctors.current[row.id]?.encounters??mapped.encounters}];}));
   cbRef.current.onDoctorsUpdate(doctors.current);
  });
  if(activePage==='patients')jobs.push(async()=>{
   const rows=await getAdminPatients();if(!current())return;
   rawPatients.current=Object.fromEntries(rows.map(row=>[row.id,row]));
   patients.current=Object.fromEntries(rows.map((row,index)=>{const mapped=backendPatientToStoreItem(row,index);return [row.id,{...mapped,encounters:patients.current[row.id]?.encounters??mapped.encounters}];}));
   cbRef.current.onPatientsUpdate(patients.current);
  });
  // Unconditional, not gated by activePage: the sidebar's unresolved-error
  // badge (adminLogsNavBadge) is always rendered regardless of which page is
  // open, but was only ever populated while actively viewing the Logs or
  // Command page — on every other page it silently showed a false "All
  // Clear" default rather than the real count. This keeps that badge (and
  // any other consumer of the logs feed) accurate everywhere, matching the
  // backend's own system_error_logs state rather than whatever page the
  // admin last happened to visit.
  jobs.push(async()=>{const feed=await getSystemErrors();if(current())cbRef.current.onLogsUpdate(feed.items.map(backendSystemErrorToStoreItem));});
  if(activePage==='grievances')jobs.push(async()=>{const rows=await getAdminGrievances();if(current())cbRef.current.onGrievancesUpdate(rows.map(backendGrievanceToStoreItem));});
  if(activePage==='finance')jobs.push(async()=>{const rows=await getAdminTransactions(statementMonth);if(current())cbRef.current.onTransactionsUpdate(rows.map(backendTransactionToStoreItem));});
  if(['finance','doctors'].includes(activePage))jobs.push(async()=>{const result=await getFinanceSummary(statementMonth);if(current())cbRef.current.onFinanceSummaryUpdate(result);});
  if(activePage==='command')jobs.push(async()=>{const result=await getCommandTelemetry();if(current())cbRef.current.onCommandTelemetryUpdate(result);});
  // Unconditional, same reasoning as the system-errors job above: the
  // sidebar nav badge and the header notification bell both need the real
  // pending-verification count on every page, not only while the admin has
  // the Verification tab itself open.
  jobs.push(async()=>{const result=await getPendingDoctorVerifications();if(current())cbRef.current.onVerificationQueueUpdate(result);});
  if(activePage==='compliance')jobs.push(async()=>{const result=await getComplianceAlerts();if(current())cbRef.current.onComplianceAlertsUpdate(result);});
  if(activePage==='finance')jobs.push(async()=>{const result=await getRefundRequests();if(current())cbRef.current.onRefundRequestsUpdate(result);});
  const work=Promise.all(jobs.map(apply)).then(()=>{}).finally(()=>{if(pending.current===work)pending.current=null;});
  pending.current=work;return work;
 },[activePage,statementMonth]);
 useEffect(()=>{
  const activeEpoch=epoch;
  epoch.current++;pending.current=null;
  const stop=startScheduleSync(fetchPage,{
   window,document,online:()=>navigator.onLine,
   every:(callback,ms)=>window.setTimeout(callback,ms),cancel:timer=>window.clearTimeout(timer as number),
  },30000,{hasError:()=>roundFailed.current,initial:true});
  return()=>{activeEpoch.current++;stop();};
 },[fetchPage]);
 useEffect(()=>{
  if(!selectedDoctor)return;let active=true;
  getDoctorDossier(selectedDoctor).then(dossier=>{if(!active)return;const mapped=backendDocToStoreItem({...rawDoctors.current[selectedDoctor],...dossier},0);doctors.current={...doctors.current,[selectedDoctor]:mapped};cbRef.current.onDoctorsUpdate(doctors.current);}).catch(()=>{});
  return()=>{active=false;};
 },[selectedDoctor]);
 useEffect(()=>{
  if(!selectedPatient)return;let active=true;
  getPatientDossier(selectedPatient).then(dossier=>{if(!active)return;const mapped=backendPatientToStoreItem({...rawPatients.current[selectedPatient],...(dossier.patient||{}),encounters:dossier.encounters||[],vault_records:dossier.vault_records||[]},0);patients.current={...patients.current,[selectedPatient]:mapped};cbRef.current.onPatientsUpdate(patients.current);}).catch(()=>{});
  return()=>{active=false;};
 },[selectedPatient]);
 return fetchPage;
}
