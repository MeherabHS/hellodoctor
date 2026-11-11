import {backendTransactionToStoreItem} from './financeProjection';
﻿// useBackendSync.ts — Fetches live data from the Axum backend and converts
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
  getBookingPolicy,
  AdminOverview,
  FinanceSummary,
  CommandTelemetry,
  PendingDoctorVerification,
  ComplianceAlert,
  SystemErrorLog,
  FinancialRefundRequest,
  BookingPolicy,
} from '@/lib/api';
import { backendDocToStoreItem, backendPatientToStoreItem, backendGrievanceToStoreItem } from './adminProjection';

export {backendTransactionToStoreItem} from './financeProjection';
export { backendDocToStoreItem, backendPatientToStoreItem, backendGrievanceToStoreItem } from './adminProjection';

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
  onBookingPolicyUpdate: (policy: BookingPolicy) => void;
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
  // Unconditional, same reasoning: the financial-module mode gates nav items
  // and page content on both the 'doctors' and 'finance' pages, and must
  // never be decided client-side — always read from the server.
  jobs.push(async()=>{const result=await getBookingPolicy();if(current())cbRef.current.onBookingPolicyUpdate(result);});
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
