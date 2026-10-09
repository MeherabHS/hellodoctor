'use client';

import React, { useState, useEffect } from 'react';
import { useBackendSync, ModalitySplit } from '@/lib/useBackendSync';
import { useSlotSync } from '@/lib/useSlotSync';
import { scheduleDay, scheduleLabel } from '@/lib/schedule';
import {
  initiateDisbursement,
  approveDisbursementItem,
  finalizeFinancialPeriod,
  verifyDoctorProfessionalRegistration,
  rejectDoctorProfessionalRegistration,
  simulateSystemError,
  acknowledgeSystemError,
  resolveSystemError,
  adjudicateGrievanceRefund,
  adjudicateGrievanceWarn,
  adjudicateGrievanceDismiss,
  transitionRefundRequest,
  AdminOverview,
  FinanceSummary,
  CommandTelemetry,
  PendingDoctorVerification,
  AdminSlot,
  ComplianceAlert,
  FinancialRefundRequest,
  toggleSlot,
} from '@/lib/api';
import {
  DoctorHistoryItem,
  PatientRecordItem,
  AppErrorLogItem,
  GrievanceItem,
  TransactionItem,
} from '@/data/adminStore';

type AdminPageId =
  | 'doctors'
  | 'finance'
  | 'command'
  | 'slots'
  | 'patients'
  | 'verification'
  | 'compliance'
  | 'logs'
  | 'grievances';

interface ToastNotice {
  id: string;
  title: string;
  message: string;
  icon: string;
  type: 'success' | 'urgent' | 'warning' | 'info';
}

export default function AdminPortalPage() {
  const [simulationsAllowed, setSimulationsAllowed] = useState(false);
  useEffect(() => {
    let active = true;
    fetch('/api/runtime', { cache: 'no-store' }).then(res => res.ok ? res.json() : null)
      .then(data => { if (active) setSimulationsAllowed(data?.simulations === true); }).catch(() => {});
    return () => { active = false; };
  }, []);
  // Navigation State
  const [activeAdminPage, setActiveAdminPage] = useState<AdminPageId>('doctors');

  // Interactive Data Stores
  const [doctorStore, setDoctorStore] = useState<Record<string, DoctorHistoryItem>>({});
  const [patientStore, setPatientStore] = useState<Record<string, PatientRecordItem>>({});
  const [errorLogs, setErrorLogs] = useState<AppErrorLogItem[]>([]);
  const [grievances, setGrievances] = useState<GrievanceItem[]>([]);
  const [transactions, setTransactions] = useState<TransactionItem[]>([]);
  const [overview, setOverview] = useState<AdminOverview | null>(null);
  const [financeSummary, setFinanceSummary] = useState<FinanceSummary | null>(null);
  const [commandTelemetry, setCommandTelemetry] = useState<CommandTelemetry | null>(null);
  const [verificationQueue, setVerificationQueue] = useState<PendingDoctorVerification[]>([]);
  const [modalitySplit, setModalitySplit] = useState<ModalitySplit>({ videoRevenue: 0, chatRevenue: 0, videoCount: 0, chatCount: 0 });
  const [adminSlots, setAdminSlots] = useState<AdminSlot[]>([]);
  const slotSync = useSlotSync(setAdminSlots);
  const [slotDate, setSlotDate] = useState(() => scheduleDay(new Date().toISOString()));
  const visibleSlots = adminSlots.filter(slot => !slotDate || scheduleDay(slot.start_time) === slotDate);
  const [complianceAlerts, setComplianceAlerts] = useState<ComplianceAlert[]>([]);
  const [refundRequests, setRefundRequests] = useState<FinancialRefundRequest[]>([]);
  const [refundBusy, setRefundBusy] = useState<string | null>(null);
  const [verificationBusy, setVerificationBusy] = useState(false);
  const [slotBusy, setSlotBusy] = useState<string | null>(null);
  const [simulateBusy, setSimulateBusy] = useState(false);
  const [logActionBusy, setLogActionBusy] = useState<string | null>(null);
  const [selectedVerificationId, setSelectedVerificationId] = useState<string | null>(null);
  const [financeMonth, setFinanceMonth] = useState(new Date().toISOString().slice(0, 7));
  const [financeBusy, setFinanceBusy] = useState(false);

  // Live Backend Synchronization Hook
  // Once the backend has answered at least once, its data is the source of truth —
  // including a correctly-empty result. No frontend fixture records are loaded.
  const refreshBackend = useBackendSync({
    onDoctorsUpdate: (backendDocs) => {
      setDoctorStore(backendDocs);
    },
    onPatientsUpdate: (backendPatients) => {
      setPatientStore(backendPatients);
    },
    onLogsUpdate: (backendLogs) => {
      setErrorLogs(backendLogs);
    },
    onGrievancesUpdate: (backendGrvs) => {
      setGrievances(backendGrvs);
    },
    onTransactionsUpdate: (backendTxs) => {
      setTransactions(backendTxs);
    },
    onOverviewUpdate: setOverview,
    onFinanceSummaryUpdate: setFinanceSummary,
    onCommandTelemetryUpdate: setCommandTelemetry,
    onVerificationQueueUpdate: (doctors) => {
      setVerificationQueue(doctors);
      setSelectedVerificationId((prev) => (prev && doctors.some((d) => d.id === prev) ? prev : doctors[0]?.id ?? null));
    },
    onModalitySplitUpdate: setModalitySplit,
    onComplianceAlertsUpdate: setComplianceAlerts,
    onRefundRequestsUpdate: setRefundRequests,
  }, financeMonth);

  // Formats a numeric-string amount from the backend as "KES  12,345".
  const money = (value: string | number | null | undefined) => `KES  ${Number(value || 0).toLocaleString()}`;

  // Filter States
  const [doctorStatusFilter, setDoctorStatusFilter] = useState<'all' | 'pending' | 'settled'>('all');
  const [doctorSearchQuery, setDoctorSearchQuery] = useState('');
  const [patientFilterCategory, setPatientFilterCategory] = useState<'all' | 'chronic' | 'chat' | 'pediatric'>('all');
  const [patientSearchQuery, setPatientSearchQuery] = useState('');
  const [logQuickFilter, setLogQuickFilter] = useState<'all' | 'critical' | 'payment' | 'webrtc' | 'offline' | 'unresolved'>('all');
  const [logSearchQuery, setLogSearchQuery] = useState('');
  const [logSeverityFilter, setLogSeverityFilter] = useState('ALL');
  const [logSubsystemFilter, setLogSubsystemFilter] = useState('ALL');
  const [logStatusFilter, setLogStatusFilter] = useState('ALL');
  const [grievanceQuickFilter, setGrievanceQuickFilter] = useState<'all' | 'doctor' | 'system' | 'pending'>('all');
  const [grievanceSearchQuery, setGrievanceSearchQuery] = useState('');
  const [grievanceTargetFilter, setGrievanceTargetFilter] = useState('ALL');
  const [grievanceStatusFilter, setGrievanceStatusFilter] = useState('ALL');
  const [financeTypeFilter, setFinanceTypeFilter] = useState<'ALL' | 'INFLOW' | 'ESCROW' | 'PAYOUT' | 'REFUND'>('ALL');
  const [financeGatewayFilter, setFinanceGatewayFilter] = useState('ALL');
  const [financeSearchQuery, setFinanceSearchQuery] = useState('');

  // Modals Active State
  const [selectedDoctorKey, setSelectedDoctorKey] = useState<string | null>(null);
  const [selectedPatientKey, setSelectedPatientKey] = useState<string | null>(null);
  const [selectedLogId, setSelectedLogId] = useState<string | null>(null);
  const [selectedGrievanceId, setSelectedGrievanceId] = useState<string | null>(null);
  const [selectedTransactionId, setSelectedTransactionId] = useState<string | null>(null);
  const [simulateModalOpen, setSimulateModalOpen] = useState(false);

  // Simulation Form State
  const [simScenario, setSimScenario] = useState('mpesa_timeout');
  const [simSeverity, setSimSeverity] = useState('CRITICAL');
  const [simCustomNote, setSimCustomNote] = useState('');

  // Toasts
  const [toasts, setToasts] = useState<ToastNotice[]>([]);

  function showToast(title: string, message: string, icon = '✓', type: 'success' | 'urgent' | 'warning' | 'info' = 'success') {
    const id = 'toast_' + Date.now();
    setToasts((prev) => [...prev, { id, title, message, icon, type }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }

  // Page Header Titles & Breadcrumbs
  const PAGE_METADATA: Record<AdminPageId, { title: string; breadcrumb: string }> = {
    doctors: {
      title: 'Telehealth Financial Settlement',
      breadcrumb: 'Doctor Wallet Payout Dashboard • Multi-Channel Payout & Automated Commission',
    },
    finance: {
      title: 'Omnichannel Payment & Escrow Master Ledger',
      breadcrumb: 'Real-time M-Pesa, Kenyan bank & card Inflows • 20% Platform Commission Splits • Escrow Release Audit',
    },
    command: {
      title: 'Operations & Live Command Center',
      breadcrumb: 'Real-time Platform Telemetry • Hourly Shift Load & WebRTC Stream Monitor',
    },
    slots: {
      title: 'Slot Matrix & 1-Hour Lock Governance',
      breadcrumb: '10-Minute Micro-Consultation Timetable & Cancellation Freeze',
    },
    patients: {
      title: 'Patients Directory & Medical Vaults',
      breadcrumb: 'Central Electronic Health Records • Longitudinal Patient Surveillance',
    },
    verification: {
      title: 'Doctor Verification',
      breadcrumb: 'Review KMPDC professional registration details • Manual HelloDoctor review',
    },
    compliance: {
      title: 'Clinical e-Prescription & Q&A Audit',
      breadcrumb: 'Clinical prescribing and platform compliance review',
    },
    logs: {
      title: 'App Failure Diagnostics & User Incident Logs',
      breadcrumb: 'Real-Time Telemetry & Exception Tracking • Traceability by User, Timestamp & Subsystem',
    },
    grievances: {
      title: 'Consultation Grievance & Injustice Adjudication',
      breadcrumb: 'Patient Rights & Clinical Accountability • Doctor Conduct & System Failure Disputes',
    },
  };

  // 1. Batch Payout Action — reports the real batch result, not an optimistic guess.
  async function handleExecuteBatchPayout() {
    if (financeBusy) return;
    const [year, month] = financeMonth.split('-').map(Number);
    const start = `${financeMonth}-01`;
    const end = `${financeMonth}-${new Date(Date.UTC(year, month, 0)).getUTCDate()}`;
    setFinanceBusy(true);
    try {
      const batch = await initiateDisbursement(start, end);
      if (batch.total_doctors > 0) {
        showToast(
          `Disbursement Batch Prepared: ${money(batch.total_net_disbursed)}`,
          `${batch.total_doctors} Doctor${batch.total_doctors === 1 ? '' : 's'} routed for Finance approval. No money moved.`,
          '🧾',
          'info'
        );
      } else {
        showToast('No Eligible Unassigned Statements', 'Review payout setup and existing allocations for the selected month. No money moved.', 'ℹ️', 'info');
      }
      await refreshBackend();
    } catch (e) {
      showToast('Batch Not Prepared', 'The month must be finalized and destinations eligible. The backend did not prepare a batch.', '⚠️', 'warning');
      console.warn('Disbursement API notice:', e);
    } finally { setFinanceBusy(false); }
  }

  async function handleFinalizePeriod() {
    if (financeBusy) return;
    setFinanceBusy(true);
    try {
      await finalizeFinancialPeriod(financeMonth);
      showToast('Accounting Month Finalized', `${financeMonth} statement amounts are frozen. This does not approve or pay any payout.`, '🧾', 'info');
      await refreshBackend();
    } catch {
      showToast('Month Not Finalized', 'Check whether the month is already finalized or in the future. The backend did not close it.', '⚠️', 'warning');
    } finally { setFinanceBusy(false); }
  }

  async function handleApproveDoctorDisbursement(itemId: string) {
    if (financeBusy) return;
    setFinanceBusy(true);
    try {
      await approveDisbursementItem(itemId);
      setDoctorStore((prev) => {
        const current = selectedDoctorKey ? prev[selectedDoctorKey] : undefined;
        if (!current?.payoutRouting) return prev;
        return {
          ...prev,
          [current.key]: {
            ...current,
            payoutRouting: { ...current.payoutRouting, disbursementStatus: 'APPROVED' },
          },
        };
      });
      showToast('Disbursement Approved', 'Finance approval recorded. No provider payment has been confirmed.', '✓', 'info');
      await refreshBackend();
    } catch (e) {
      showToast('Approval Failed', 'The backend did not approve this disbursement item.', '⚠️', 'warning');
      console.warn('Disbursement approval API notice:', e);
    } finally { setFinanceBusy(false); }
  }

  // 2. Professional registration Approval — operates on whichever real pending doctor is selected, not a fixed id.
  async function handleVerificationAction(action: 'approve' | 'reject') {
    if (verificationBusy) return;
    const candidate = verificationQueue.find((d) => d.id === selectedVerificationId);
    if (!candidate) {
      showToast('No Application Selected', 'Select a doctor from the vetting queue first.', 'ℹ️', 'info');
      return;
    }

    setVerificationBusy(true);
    try {
      if (action === 'approve') {
        try {
          await verifyDoctorProfessionalRegistration(candidate.id, `${candidate.license_number} approved by admin`);
          showToast(`${candidate.full_name} Approved ✓`, `${candidate.license_number} credentialed by the backend.`, '✓', 'success');
          await refreshBackend();
        } catch (e) {
          showToast('Approval Failed', 'The backend did not verify this Doctor.', '⚠️', 'warning');
          console.warn('Professional registration verify API notice:', e);
        }
      } else {
        try {
          await rejectDoctorProfessionalRegistration(candidate.id, 'Application declined by admin');
          showToast('Application Declined', `${candidate.full_name}'s application was rejected.`, '✕', 'warning');
          await refreshBackend();
        } catch (e) {
          showToast('Rejection Failed', 'The backend did not reject this application.', '⚠️', 'warning');
          console.warn('Professional registration reject API notice:', e);
        }
      }
    } finally {
      setVerificationBusy(false);
    }
  }

  // Slot availability (Available <-> Blocked) against the real backend. Guarded
  // while a mutation is pending, so duplicate clicks cannot submit stale intent.
  async function handleToggleSlot(slotId: string, action: "BLOCK" | "UNBLOCK") {
    if (slotBusy) return;
    setSlotBusy(slotId);
    try {
      const result = await toggleSlot(slotId, action);
      setAdminSlots(rows => rows.map(slot => slot.id === result.slot_id ? { ...slot, status: result.status } : slot));
      await slotSync.refresh();
    } catch (e) {
      showToast('Slot Update Failed', 'The backend did not toggle this slot.', '⚠️', 'warning');
      console.warn('Slot toggle API notice:', e);
    } finally {
      setSlotBusy(null);
    }
  }

  // 3. Log Simulation — writes a real row into the backend's dedicated
  // `system_error_logs` table (dev/staging only; the backend itself rejects
  // this in production) and refreshes from the server, rather than
  // fabricating a client-only row.
  async function handleExecuteSimulateFailure() {
    if (!simulationsAllowed) return;
    if (simulateBusy) return;
    setSimulateBusy(true);
    try {
      const created = await simulateSystemError({
        scenario: simScenario,
        severity: simSeverity,
        custom_note: simCustomNote.slice(0, 300) || undefined,
      });
      setSimulateModalOpen(false);
      setSimCustomNote('');
      await refreshBackend();
      showToast('⚡ Failure Incident Injected', `${created.component} failure logged under ${created.trace_id}.`, '⚡', 'urgent');
    } catch (e) {
      showToast('Simulation Failed', e instanceof Error ? e.message : 'The backend rejected the simulated incident.', '⚠️', 'warning');
    } finally {
      setSimulateBusy(false);
    }
  }

  // 4. Grievance Adjudication
  async function handleAdjudicateGrievance(grvId: string, action: 'REFUND' | 'WARN' | 'RESOLVE') {
    if (financeBusy) return;
    setFinanceBusy(true);
    try {
      if (action === 'REFUND') {
        await adjudicateGrievanceRefund(grvId, 'Board-approved refund; provider execution pending');
      } else if (action === 'WARN') {
        await adjudicateGrievanceWarn(grvId, 'Clinical Governance warning');
      } else {
        await adjudicateGrievanceDismiss(grvId, 'Telemetry evidence did not substantiate the claim');
      }
      await refreshBackend();
      showToast(action === 'REFUND' ? 'Refund approved' : 'Decision recorded',
        action === 'REFUND' ? 'Provider execution is pending. No money has been returned.' : 'The backend confirmed the decision.', '?', 'success');
    } catch (error) {
      showToast('Decision not confirmed', error instanceof Error ? error.message : 'Retry after checking the server state.', '!', 'warning');
    } finally {
      setFinanceBusy(false);
    }
  }

  // Finance refund-request resolution — the lifecycle step after a
  // grievance adjudication approves a refund (above). REFUNDED requires a
  // provider_reference, prompted for here since the backend 422s without one.
  async function handleTransitionRefund(
    requestId: string,
    status: 'PROCESSING' | 'REFUNDED' | 'FAILED' | 'RECONCILIATION_REQUIRED'
  ) {
    if (refundBusy) return;
    let providerReference: string | undefined;
    if (status === 'REFUNDED') {
      const entered = window.prompt('Provider reference for this refund (required, max 300 characters):');
      if (!entered || !entered.trim()) return;
      const trimmed = entered.trim();
      if (trimmed.length > 300) {
        showToast('Reference Too Long', 'Provider reference must be 300 characters or fewer.', '⚠️', 'warning');
        return;
      }
      providerReference = trimmed;
    }
    setRefundBusy(requestId);
    try {
      await transitionRefundRequest(requestId, status, providerReference);
      await refreshBackend();
      showToast('Refund Request Updated', `Status set to ${status.replace(/_/g, ' ')}.`, '✓', 'success');
    } catch (error) {
      showToast('Update Failed', error instanceof Error ? error.message : 'The backend rejected this transition.', '⚠️', 'warning');
    } finally {
      setRefundBusy(null);
    }
  }

  // System-error lifecycle actions — real backend calls against
  // `/admin/system-errors/:id/{acknowledge,resolve}`, then refetch, instead
  // of a client-only status flip. There is no "reopen" capability on the
  // backend, so a RESOLVED incident has no corresponding undo here either.
  async function handleAcknowledgeLog(logId: string) {
    if (logActionBusy) return;
    setLogActionBusy(logId);
    try {
      await acknowledgeSystemError(logId);
      await refreshBackend();
      showToast('Incident Acknowledged', 'Marked as under investigation.', '🔍', 'info');
    } catch (e) {
      showToast('Update Failed', e instanceof Error ? e.message : 'The backend did not accept this change.', '⚠️', 'warning');
    } finally {
      setLogActionBusy(null);
    }
  }

  async function handleResolveLog(logId: string) {
    if (logActionBusy) return;
    setLogActionBusy(logId);
    try {
      await resolveSystemError(logId, 'Administrative resolution signed off by Clinical Ops Director.');
      await refreshBackend();
      showToast('Incident Resolved', 'Telemetry exception marked resolved.', '✓', 'success');
    } catch (e) {
      showToast('Update Failed', e instanceof Error ? e.message : 'The backend did not accept this change.', '⚠️', 'warning');
    } finally {
      setLogActionBusy(null);
    }
  }

  // Counters
  const unresolvedLogsCount = errorLogs.filter((l) => l.status !== 'RESOLVED').length;
  const pendingGrievancesCount = grievances.filter((g) => g.status === 'PENDING_REVIEW' || g.status === 'UNDER_INVESTIGATION').length;

  // Filtered lists
  const filteredDoctors = Object.values(doctorStore).filter((doc) => {
    const text = (doc.name + ' ' + doc.registrationNumber + ' ' + doc.meta).toLowerCase();
    const matchesSearch = !doctorSearchQuery || text.includes(doctorSearchQuery.toLowerCase());
    const isSettled = (doc.pendingDisbursement ?? 0) === 0;
    const matchesStatus =
      doctorStatusFilter === 'all' ? true : doctorStatusFilter === 'settled' ? isSettled : !isSettled;
    return matchesSearch && matchesStatus;
  });
  const doctorsAwaitingCount = Object.values(doctorStore).filter((d) => (d.pendingDisbursement ?? 0) > 0).length;
  const doctorsSettledCount = Object.values(doctorStore).length - doctorsAwaitingCount;

  const filteredPatients = Object.values(patientStore).filter((pt) => {
    const text = (pt.name + ' ' + pt.id + ' ' + pt.meta + ' ' + pt.cohort).toLowerCase();
    const matchesSearch = !patientSearchQuery || text.includes(patientSearchQuery.toLowerCase());
    const matchesCat =
      patientFilterCategory === 'all'
        ? true
        : patientFilterCategory === 'chronic'
        ? pt.category.includes('chronic')
        : patientFilterCategory === 'chat'
        ? pt.category.includes('chat')
        : pt.category.includes('pediatric');
    return matchesSearch && matchesCat;
  });

  const filteredLogs = errorLogs.filter((log) => {
    if (logQuickFilter === 'critical' && log.severity !== 'CRITICAL') return false;
    if (logQuickFilter === 'payment' && log.subsystem !== 'PAYMENT_GATEWAY') return false;
    if (logQuickFilter === 'webrtc' && log.subsystem !== 'AGORA_RTC') return false;
    if (logQuickFilter === 'offline' && log.subsystem !== 'SYNC_OFFLINE_CACHE') return false;
    if (logQuickFilter === 'unresolved' && log.status === 'RESOLVED') return false;

    if (logSeverityFilter !== 'ALL' && log.severity !== logSeverityFilter) return false;
    if (logSubsystemFilter !== 'ALL' && log.subsystem !== logSubsystemFilter) return false;
    if (logStatusFilter !== 'ALL' && log.status !== logStatusFilter) return false;

    if (logSearchQuery) {
      const q = logSearchQuery.toLowerCase();
      const text = (log.user.name + ' ' + log.traceId + ' ' + log.errorMessage + ' ' + log.subsystem + ' ' + log.errorCode).toLowerCase();
      if (!text.includes(q)) return false;
    }
    return true;
  });

  const filteredGrievances = grievances.filter((grv) => {
    if (grievanceQuickFilter === 'doctor' && grv.target !== 'DOCTOR') return false;
    if (grievanceQuickFilter === 'system' && grv.target !== 'SYSTEM') return false;
    if (grievanceQuickFilter === 'pending' && !(grv.status === 'PENDING_REVIEW' || grv.status === 'UNDER_INVESTIGATION')) return false;

    if (grievanceTargetFilter !== 'ALL' && grv.target !== grievanceTargetFilter) return false;
    if (grievanceStatusFilter !== 'ALL' && grv.status !== grievanceStatusFilter) return false;

    if (grievanceSearchQuery) {
      const q = grievanceSearchQuery.toLowerCase();
      const text = (grv.id + ' ' + grv.patient.name + ' ' + grv.doctor.name + ' ' + grv.category + ' ' + grv.patientStatement).toLowerCase();
      if (!text.includes(q)) return false;
    }
    return true;
  });

  const filteredTransactions = transactions.filter((tx) => {
    if (financeTypeFilter === 'INFLOW' && tx.direction !== 'INFLOW') return false;
    if (financeTypeFilter === 'ESCROW' && tx.escrowStatus !== 'ESCROW_LOCKED') return false;
    if (financeTypeFilter === 'PAYOUT' && tx.type !== 'PAYOUT') return false;
    if (financeTypeFilter === 'REFUND' && tx.direction !== 'REFUND') return false;

    if (financeGatewayFilter !== 'ALL' && tx.gateway !== financeGatewayFilter) return false;

    if (financeSearchQuery) {
      const q = financeSearchQuery.toLowerCase();
      const text = (tx.txId + ' ' + tx.gatewayRef + ' ' + tx.patient.name + ' ' + tx.doctor.name + ' ' + tx.consultationId).toLowerCase();
      if (!text.includes(q)) return false;
    }
    return true;
  });

  // Selected Modal Objects
  const selectedDocObj = selectedDoctorKey ? doctorStore[selectedDoctorKey] || doctorStore.sabrina : null;
  const selectedPatObj = selectedPatientKey ? patientStore[selectedPatientKey] || patientStore.sarah : null;
  const selectedLogObj = selectedLogId ? errorLogs.find((l) => l.id === selectedLogId) : null;
  const selectedGrvObj = selectedGrievanceId ? grievances.find((g) => g.id === selectedGrievanceId) : null;
  const selectedTxObj = selectedTransactionId ? transactions.find((t) => t.txId === selectedTransactionId) : null;

  return (
    <div id="adminPortalShell" className="admin-portal-shell">
      {/* Toast Overlay */}
      <div id="adminToastContainer" className="app-toast-container" style={{ position: 'fixed', top: '70px', right: '24px', left: 'auto', width: '360px', zIndex: 9999 }}>
        {toasts.map((t) => (
          <div
            key={t.id}
            style={{
              background: '#0F172A',
              color: '#FFFFFF',
              borderRadius: '12px',
              padding: '12px 16px',
              marginBottom: '10px',
              boxShadow: '0 10px 25px -5px rgba(0,0,0,0.3)',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              animation: 'fadeIn 0.2s ease',
            }}
          >
            <div style={{ fontSize: '18px' }}>{t.icon}</div>
            <div style={{ flex: 1 }}>
              <div style={{ fontWeight: 800, fontSize: '12.5px', color: '#38BDF8' }}>{t.title}</div>
              <div style={{ fontSize: '11px', color: '#CBD5E1', marginTop: '2px' }}>{t.message}</div>
            </div>
          </div>
        ))}
      </div>

      {/* ── Left Sidebar (Full Height, Pure White) ── */}
      <aside className="admin-sidebar">
        <div className="admin-sidebar-header">
          <div className="admin-brand-icon">H</div>
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            <span className="admin-brand-name">HelloDoctor</span>
            <span style={{ fontSize: '9.5px', color: '#64748B', fontWeight: 700, letterSpacing: '0.5px', textTransform: 'uppercase' }}>Central Admin</span>
          </div>
        </div>

        <nav className="admin-sidebar-nav">
          <div
            className={`admin-nav-item ${activeAdminPage === 'doctors' ? 'active' : ''}`}
            id="adminNav-doctors"
            onClick={() => setActiveAdminPage('doctors')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v12M15 9.5a3.5 3.5 0 0 0-7 0c0 2 1.5 3 3.5 3s3.5 1 3.5 3a3.5 3.5 0 0 1-7 0"/></svg>
            <span>Settlements &amp; Payouts</span>
            {doctorsAwaitingCount > 0 ? (
              <span className="admin-nav-badge amber">{doctorsAwaitingCount} Pending</span>
            ) : (
              <span className="admin-nav-badge green" style={{ background: '#DCFCE7', color: '#166534' }}>Settled</span>
            )}
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'finance' ? 'active' : ''}`}
            id="adminNav-finance"
            onClick={() => setActiveAdminPage('finance')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
            <span>All Payments &amp; Escrow</span>
            <span className="admin-nav-badge green" style={{ background: '#DCFCE7', color: '#166534', fontWeight: 800 }}>Ledger</span>
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'command' ? 'active' : ''}`}
            id="adminNav-command"
            onClick={() => setActiveAdminPage('command')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
            <span>Command Center</span>
            <span style={{ width: '7px', height: '7px', borderRadius: '50%', background: '#10B981', marginLeft: 'auto' }}></span>
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'slots' ? 'active' : ''}`}
            id="adminNav-slots"
            onClick={() => setActiveAdminPage('slots')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>
            <span>Slot Matrix &amp; Locks</span>
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'patients' ? 'active' : ''}`}
            id="adminNav-patients"
            onClick={() => setActiveAdminPage('patients')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>
            <span>Patients Directory</span>
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'verification' ? 'active' : ''}`}
            id="adminNav-verification"
            onClick={() => setActiveAdminPage('verification')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><path d="m9 15 2 2 4-4"/></svg>
            <span>Doctor Verification</span>
            {verificationQueue.length > 0 && <span className="admin-nav-badge blue">{verificationQueue.length}</span>}
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'compliance' ? 'active' : ''}`}
            id="adminNav-compliance"
            onClick={() => setActiveAdminPage('compliance')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
            <span>Rx &amp; Q&amp;A Audit</span>
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'logs' ? 'active' : ''}`}
            id="adminNav-logs"
            onClick={() => setActiveAdminPage('logs')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>
            <span>App Error &amp; Incident Logs</span>
            <span className="admin-nav-badge red" id="adminLogsNavBadge" style={{ background: '#FEE2E2', color: '#DC2626', fontWeight: 800 }}>
              {unresolvedLogsCount > 0 ? `${unresolvedLogsCount} New` : 'All Clear'}
            </span>
          </div>

          <div
            className={`admin-nav-item ${activeAdminPage === 'grievances' ? 'active' : ''}`}
            id="adminNav-grievances"
            onClick={() => setActiveAdminPage('grievances')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
            <span>Consultation Grievances</span>
            <span className="admin-nav-badge red" id="adminGrievanceNavBadge" style={{ background: '#FEE2E2', color: '#DC2626', fontWeight: 800 }}>
              {pendingGrievancesCount > 0 ? `${pendingGrievancesCount} Open` : '0'}
            </span>
          </div>
        </nav>

        <div className="admin-sidebar-footer">
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <span style={{ width: '7px', height: '7px', borderRadius: '50%', background: '#10B981' }}></span>
            <span style={{ fontWeight: 700, color: '#475569' }}>Core Gateway v2.4</span>
          </div>
          <span style={{ color: '#94A3B8' }}>DGHS Uplink</span>
        </div>
      </aside>

      {/* Main Wrapper (Topbar + Content Area) */}
      <div className="admin-main-wrapper">
        <header className="admin-topbar">
          <div className="admin-topbar-left">
            <h1 className="admin-page-title" id="adminPageHeaderTitle">
              {PAGE_METADATA[activeAdminPage].title}
            </h1>
            <span className="admin-breadcrumb" id="adminBreadcrumb">
              {PAGE_METADATA[activeAdminPage].breadcrumb}
            </span>
          </div>

          <div className="admin-topbar-right">
            <div className="admin-search-box">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
              <input
                type="text"
                id="adminGlobalSearchInput"
                maxLength={150}
                placeholder="Search doctor, Professional registration #, patient ID, TxID..."
                onChange={(e) => {
                  const val = e.target.value;
                  setDoctorSearchQuery(val);
                  setPatientSearchQuery(val);
                  setLogSearchQuery(val);
                  setFinanceSearchQuery(val);
                }}
              />
            </div>

            <button
              className="admin-16263-btn"
              onClick={() => showToast('Emergency Bridge', 'The Kenya emergency contact is configured by deployment.', '🚨', 'info')}
            >
              <span style={{ width: '7px', height: '7px', borderRadius: '50%', background: '#DC2626' }}></span>
              <span>🚨 Emergency Helpline Bridge</span>
            </button>

            <div
              className="admin-top-icon-btn"
              title="Pending Notifications"
              onClick={() => showToast('Central Notifications', '3 Professional registration physician applications pending review.', '🔔', 'info')}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
              <span className="badge-dot"></span>
            </div>

            <div className="admin-user-pill">
              <div className="admin-avatar">AR</div>
              <div style={{ display: 'flex', flexDirection: 'column' }}>
                <span className="admin-user-name">Amina Rashid</span>
                <span className="admin-user-role">Clinical Ops Director</span>
              </div>
            </div>

            <div
              className="admin-top-icon-btn"
              title="Sign out"
              onClick={async () => {
                try {
                  const result = await fetch('/api/logout', { method: 'POST' });
                  if (!result.ok) window.alert('Signed out locally. Server revocation could not be confirmed.');
                } catch { window.alert('Unable to reach the server. Server sign-out could not be confirmed.'); }
                window.location.href = '/login';
              }}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>
            </div>
          </div>
        </header>

        {/* Scrollable Canvas Area */}
        <main className="admin-content-area">

          {/* PAGE 1: SETTLEMENTS & DOCTOR WALLET PAYOUTS */}
          {activeAdminPage === 'doctors' && (
            <div className="admin-page-view active" id="adminPage_doctors">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div>
                  <h2 style={{ fontSize: '20px', fontWeight: 850, color: '#0F172A', margin: 0, letterSpacing: '-0.4px' }}>
                    Doctor Wallet Payout Dashboard
                  </h2>
                  <div style={{ fontSize: '12px', color: '#64748B', marginTop: '2px' }}>
                    Automated 20% platform commission reconciliation and multi-channel payouts
                  </div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center' }}>
                  <button
                    className="admin-action-btn"
                    onClick={() => setActiveAdminPage('finance')}
                    style={{ background: '#F1F5F9', border: '1px solid #CBD5E1', color: '#1E293B', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '9px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer', marginRight: '8px' }}
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
                    <span>Master Payment Ledger →</span>
                  </button>
                  <label style={{ fontSize: '12px', marginRight: '8px' }}>Statement month <input aria-label="Statement month" type="month" value={financeMonth} max={new Date().toISOString().slice(0, 7)} onChange={(e) => {
                    if (!e.target.value || financeBusy) return;
                    setFinanceMonth(e.target.value);
                    setDoctorStore((previous) => Object.fromEntries(Object.entries(previous).map(([key, doc]) => [key, { ...doc, payoutRouting: undefined }])));
                  }} /></label>
                  <button className="admin-btn-sec" disabled={financeBusy} onClick={handleFinalizePeriod}>Finalize Month</button>
                  <button className="admin-batch-payout-btn" disabled={financeBusy} onClick={handleExecuteBatchPayout}>
                    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                    <span>Prepare Batch</span>
                  </button>
                </div>
              </div>

              {/* 4 Metric Cards — all computed from live backend data */}
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Gross Telehealth Revenue</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(overview?.gross_volume)}</div>
                  <span className="admin-kpi-delta neutral">Across {transactions.length} transaction{transactions.length === 1 ? '' : 's'}</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">24h Chat Revenue</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(modalitySplit.chatRevenue)}</div>
                  <div className="kpi-micro-chart">
                    <div className="kpi-donut-legend">
                      <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#2563EB' }}></span> {modalitySplit.videoCount} video consultation{modalitySplit.videoCount === 1 ? '' : 's'}</div>
                      <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#E2E8F0' }}></span> {modalitySplit.chatCount} chat session{modalitySplit.chatCount === 1 ? '' : 's'}</div>
                    </div>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Platform Fee (20%)</span>
                    <div className="admin-kpi-icon-circle teal" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(overview?.platform_fees)}</div>
                  <span className="admin-kpi-delta neutral">Automated commission cut</span>
                  {(modalitySplit.videoRevenue + modalitySplit.chatRevenue) > 0 && (
                    <div className="kpi-micro-chart kpi-stacked-bar">
                      <div className="kpi-bar-row">
                        <span className="kpi-bar-label">Video</span>
                        <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: `${Math.round((modalitySplit.videoRevenue / (modalitySplit.videoRevenue + modalitySplit.chatRevenue)) * 100)}%`, background: 'linear-gradient(90deg,#0D9488,#5EEAD4)' }}></div></div>
                        <span className="kpi-bar-value">{money(modalitySplit.videoRevenue)}</span>
                      </div>
                      <div className="kpi-bar-row">
                        <span className="kpi-bar-label">Chat</span>
                        <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: `${Math.round((modalitySplit.chatRevenue / (modalitySplit.videoRevenue + modalitySplit.chatRevenue)) * 100)}%`, background: 'linear-gradient(90deg,#14B8A6,#99F6E4)' }}></div></div>
                        <span className="kpi-bar-value">{money(modalitySplit.chatRevenue)}</span>
                      </div>
                    </div>
                  )}
                </div>

                <div className="admin-kpi-card" style={{ borderColor: '#FCD34D' }}>
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Unpaid Doctor Earnings</span>
                    <div className="admin-kpi-icon-circle amber" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" id="adminEscrowDisplay">
                    {doctorsAwaitingCount === 0 ? (
                      <span style={{ color: '#059669', fontSize: '20px' }}>{money(overview?.pending_disbursement)}</span>
                    ) : (
                      money(overview?.pending_disbursement)
                    )}
                  </div>
                  <div className="kpi-micro-chart">
                    <div className="kpi-pulse-indicator" style={{ marginBottom: '4px' }}>
                      <div className="kpi-pulse-dot" style={{ background: doctorsAwaitingCount === 0 ? '#059669' : '#D97706' }}></div>
                      <span className="kpi-pulse-text" style={{ color: doctorsAwaitingCount === 0 ? '#059669' : '#D97706' }}>
                        {doctorsAwaitingCount === 0
                          ? 'No pending earning balance'
                          : `${doctorsAwaitingCount} doctor${doctorsAwaitingCount === 1 ? '' : 's'} awaiting disbursement`}
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '11px', color: '#64748B', margin: '-8px 0 16px' }}>
                <span>ℹ️</span>
                <span>Total calculation is based including the platform charge 20%.</span>
              </div>

              {/* Pending Doctor Payout Disbursements Table */}
              <div className="admin-card">
                <div className="admin-card-header" style={{ flexWrap: 'wrap', gap: '12px', alignItems: 'center' }}>
                  <div>
                    <div className="admin-card-title">Physician Directory &amp; Wallet Payout Database</div>
                    <div className="admin-card-subtitle">Search, filter &amp; audit Professional registration specialists, consultation loads, and payment settlements</div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                    <div style={{ position: 'relative', minWidth: '240px', maxWidth: '320px' }}>
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', pointerEvents: 'none' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
                        maxLength={150}
                        placeholder="Search doctor, Professional registration #, specialty, method..."
                        value={doctorSearchQuery}
                        onChange={(e) => setDoctorSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 12px 7px 30px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', outline: 'none' }}
                      />
                    </div>

                    <div style={{ display: 'flex', gap: '6px' }}>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'all' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('all')}>All ({Object.keys(doctorStore).length})</button>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'pending' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('pending')}>Pending ({doctorsAwaitingCount})</button>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'settled' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('settled')}>Settled ({doctorsSettledCount})</button>
                    </div>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Doctor Name</th>
                        <th>Professional registration Reg</th>
                        <th>Completed Visits</th>
                        <th>24h Chat Sessions</th>
                        <th>Monthly Gross</th>
                        <th>Monthly Platform Charge</th>
                        <th>Monthly Payable / Estimate</th>
                        <th>Payout Method</th>
                        <th>Status</th>
                        <th>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredDoctors.map((doc) => {
                        const isSettled = (doc.pendingDisbursement ?? 0) === 0;
                        return (
                          <tr
                            key={doc.key}
                            className="admin-clickable-row"
                            onClick={() => setSelectedDoctorKey(doc.key)}
                          >
                            <td>
                              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                                <div style={{ width: '34px', height: '34px', borderRadius: '50%', background: doc.bgColor + '20', color: doc.bgColor, display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '12px' }}>
                                  {doc.avatar}
                                </div>
                                <div>
                                  <strong style={{ color: '#0F172A', fontSize: '13px' }}>{doc.name}</strong>
                                  <div style={{ fontSize: '10.5px', color: '#64748B' }}>{doc.meta.split('•')[0]} • {doc.residence.split('(')[0]}</div>
                                  <div style={{ fontSize: '10px', color: '#2563EB', fontFamily: 'monospace', marginTop: '1px' }}>📞 {doc.phone}</div>
                                </div>
                              </div>
                            </td>
                            <td><span className="badge badge-slate" style={{ fontWeight: 750 }}>{doc.registrationNumber}</span><div>{doc.professionalVerificationStatus}</div></td>
                            <td><b>{doc.kpiVisits}</b></td>
                            <td>{doc.kpiChats}</td>
                            <td>{doc.payoutRouting ? `${doc.payoutRouting.currency} ${Number(doc.payoutRouting.gross || 0).toLocaleString()}` : 'Unavailable'}</td>
                            <td style={{ color: '#64748B' }}>{doc.payoutRouting ? `${doc.payoutRouting.currency} ${Number(doc.payoutRouting.platformCharge || 0).toLocaleString()}` : 'Unavailable'}</td>
                            <td style={{ fontWeight: 850, color: '#059669' }}>{doc.payoutRouting ? `${doc.payoutRouting.currency} ${Number(doc.payoutRouting.finalPayable).toLocaleString()}` : 'Unavailable'}</td>
                            <td>
                              <span className="payout-method-badge bank">
                                {doc.payoutRouting?.provider
                                  ? `${doc.payoutRouting.provider} ${doc.payoutRouting.maskedDestination || ''}`
                                  : 'Payout setup required'}
                              </span>
                            </td>
                            <td>
                              <span className={`badge ${doc.payoutRouting?.readiness === 'ACTIVE' ? 'badge-green' : 'badge-amber'} doc-payout-status`}>
                                {doc.payoutRouting?.disbursementStatus || doc.payoutRouting?.readiness || 'Payout setup required'}
                              </span>
                              <div style={{ fontSize: '10px' }}>{doc.payoutRouting?.statementStatus || 'Not loaded'} · {doc.payoutRouting?.readiness || 'Unknown'}</div>
                            </td>
                            <td>
                              <button
                                className="admin-btn-sec"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setSelectedDoctorKey(doc.key);
                                }}
                              >
                                View History ↗
                              </button>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* PAGE 2: EXECUTIVE COMMAND CENTER — all values are live from /admin/command/telemetry */}
          {activeAdminPage === 'command' && (
            <div className="admin-page-view active" id="adminPage_command">
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Consultations Completed Today</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{commandTelemetry?.completed_today ?? 0}</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Active Specialists Online</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="8.5" cy="7" r="4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{commandTelemetry?.active_specialists ?? 0}</div>
                  <span className="admin-kpi-delta neutral">of {Object.keys(doctorStore).length} total registered</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Live Consultation Sessions</span>
                    <div className="admin-kpi-icon-circle purple" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{commandTelemetry?.active_sessions_count ?? 0}</div>
                  <span className="admin-kpi-delta neutral">Video + 24h chat combined</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Emergency Helpline</span>
                    <div className="admin-kpi-icon-circle red" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#DC2626' }}>Configured</div>
                  <span className="admin-kpi-delta neutral">Kenya emergency contact is configured per deployment.</span>
                </div>
              </div>

              {/* Live WebRTC Stream Monitor — real consultation_sessions + telemetry */}
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <div className="admin-card-title">Live WebRTC Stream Monitor</div>
                    <div className="admin-card-subtitle">Real-time Agora RTC sessions currently in progress</div>
                  </div>
                </div>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  {(commandTelemetry?.active_streams ?? []).length === 0 ? (
                    <div style={{ padding: '24px', textAlign: 'center', color: '#64748B', fontSize: '12.5px' }}>No consultations are currently in progress.</div>
                  ) : (
                    commandTelemetry!.active_streams.map((s) => (
                      <div className="webrtc-stream-card" key={s.session_id}>
                        <div>
                          <strong style={{ fontSize: '12px', color: '#0F172A' }}>{s.channel_name} • {s.doctor_name}</strong>
                          <div style={{ fontSize: '10.5px', color: '#64748B', marginTop: '2px' }}>
                            Patient: {s.patient_name} • Started: <b>{s.started_at ? new Date(s.started_at).toLocaleTimeString() : 'Unknown'}</b>
                          </div>
                        </div>
                        <span className={`webrtc-ping-badge ${s.rtt_ms != null && s.rtt_ms < 60 ? 'good' : 'moderate'}`}>{s.rtt_ms == null ? 'Unmeasured RTT' : `${s.rtt_ms}ms`} • {s.connection_state}</span>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}

          {/* PAGE 3: SLOT MATRIX & LOCKS — real schedule_slots data, grouped by doctor */}
          {activeAdminPage === 'slots' && (
            <div className="admin-page-view active" id="adminPage_slots">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <div className="admin-card-title">Doctor Schedule Slots</div>
                    <div className="admin-card-subtitle">Click an available slot to block it, or a blocked slot to make it available. Booked, payment-locked and cancelled slots cannot be changed here.</div>
                    <div className="admin-card-subtitle">All schedule times use East Africa Time (UTC+3), matching Doctor Web. Available means open for booking; it is not a patient booking.</div>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span className="slot-chip available" style={{ padding: '4px 8px', fontSize: '10px' }}>Available</span>
                    <span className="slot-chip blocked" style={{ padding: '4px 8px', fontSize: '10px' }}>Blocked</span>
                    <span className="slot-chip booked" style={{ padding: '4px 8px', fontSize: '10px' }}>Booked</span>
                    <span className="slot-chip locked" style={{ padding: '4px 8px', fontSize: '10px' }}>🔒 Locked in Payment</span>
                  </div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: '12px', padding: '12px 20px' }}>
                  <label>Schedule date <input type="date" aria-label="Schedule date" value={slotDate} onChange={event => setSlotDate(event.target.value)} /></label>
                  <button onClick={() => setSlotDate('')}>All dates</button>
                  <button disabled={slotSync.loading || !!slotBusy} onClick={() => void slotSync.refresh()}>Refresh schedule</button>
                  <span role="status" aria-live="polite">{slotSync.loading ? 'Refreshing schedule…' : slotSync.updatedAt ? `Last synced ${scheduleLabel(slotSync.updatedAt)} EAT` : 'Schedule not loaded'}</span>
                </div>
                {slotSync.error && <div role="alert" style={{ padding: '12px 20px', color: '#B45309' }}>{slotSync.error}</div>}
                {visibleSlots.length === 0 ? (
                  <div style={{ padding: '24px', textAlign: 'center', color: '#64748B', fontSize: '12.5px' }}>{slotSync.loading ? 'Loading schedule…' : slotSync.error ? 'Schedule unavailable.' : slotDate ? 'No schedule slots for this date.' : 'No schedule slots exist yet.'}</div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
                    {Object.entries(
                      visibleSlots.reduce((groups: Record<string, typeof adminSlots>, slot) => {
                        (groups[slot.doctor_id] ??= []).push(slot);
                        return groups;
                      }, {})
                    ).map(([doctorId, slots]) => (
                      <div key={doctorId}>
                        <strong style={{ fontSize: '12.5px', color: '#0F172A' }}>{slots[0].doctor_name}</strong>
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '8px' }}>
                          {slots
                            .sort((a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime())
                            .map((slot) => {
                              const chipCls = slot.status === 'AVAILABLE' ? 'available' : slot.status === 'BLOCKED' ? 'blocked' : slot.status === 'BOOKED' ? 'booked' : slot.status === 'LOCKED_IN_PAYMENT' ? 'locked' : 'cancelled';
                              const canToggle = (slot.status === 'AVAILABLE' || slot.status === 'BLOCKED') && !slotBusy && !slotSync.error;
                              return (
                                <div
                                  key={slot.id}
                                  data-slot-id={slot.id}
                                  data-slot-status={slot.status}
                                  className={`slot-chip ${chipCls}`}
                                  title={`${slot.status === 'AVAILABLE' ? 'Block slot' : slot.status === 'BLOCKED' ? 'Make slot available' : slot.status}. Created ${scheduleLabel(slot.created_at)} EAT. Slot ${slot.id}`}
                                  style={{ cursor: canToggle ? 'pointer' : 'default', opacity: slot.status === 'CANCELLED' ? 0.5 : slotBusy === slot.id ? 0.6 : 1 }}
                                  onClick={() => canToggle && handleToggleSlot(slot.id, slot.status === 'AVAILABLE' ? 'BLOCK' : 'UNBLOCK')}
                                >
                                  <span>{scheduleLabel(slot.start_time)} – {new Date(slot.end_time).toLocaleTimeString('en-KE', { timeZone: 'Africa/Nairobi', hour: '2-digit', minute: '2-digit' })} EAT</span>
                                  <span style={{ fontSize: '9.5px', opacity: 0.85 }}>
                                    {slot.status === 'LOCKED_IN_PAYMENT' ? '🔒 Locked' : slot.status.replace(/_/g, ' ')}
                                  </span>
                                </div>
                              );
                            })}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* PAGE 4: PATIENTS DIRECTORY */}
          {activeAdminPage === 'patients' && (
            <div className="admin-page-view active" id="adminPage_patients">
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div>
                    <span className="admin-kpi-sub">Total Registered Patients</span>
                    <div className="admin-kpi-val">{Object.keys(patientStore).length}</div>
                  </div>
                  <div className="admin-kpi-icon-circle green">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/></svg>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div>
                    <span className="admin-kpi-sub">Patients With 24h Chat History</span>
                    <div className="admin-kpi-val">{Object.values(patientStore).filter((p) => (p.chatSessions ?? 0) > 0).length}</div>
                  </div>
                  <div className="admin-kpi-icon-circle blue">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div>
                    <span className="admin-kpi-sub">Total Consultations Logged</span>
                    <div className="admin-kpi-val">{Object.values(patientStore).reduce((s, p) => s + p.encounters.length, 0)}</div>
                  </div>
                  <div className="admin-kpi-icon-circle amber">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div>
                    <span className="admin-kpi-sub">Total Patient Health Spend</span>
                    <div className="admin-kpi-val">{money(overview?.gross_volume)}</div>
                  </div>
                  <div className="admin-kpi-icon-circle teal">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v12M15 9.5a3.5 3.5 0 0 0-7 0c0 2 1.5 3 3.5 3s3.5 1 3.5 3a3.5 3.5 0 0 1-7 0"/></svg>
                  </div>
                </div>
              </div>

              <div className="admin-card">
                <div className="admin-card-header" style={{ flexWrap: 'wrap', gap: '12px', alignItems: 'center' }}>
                  <div>
                    <div className="admin-card-title">Patients Directory &amp; Longitudinal Archive</div>
                    <div className="admin-card-subtitle">Search, filter &amp; open patient clinical dossiers, EMR vaults, and consult history</div>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                    <div style={{ position: 'relative', minWidth: '240px', maxWidth: '320px' }}>
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', pointerEvents: 'none' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
                        maxLength={150}
                        placeholder="Search patient name, #PT ID, phone, dx..."
                        value={patientSearchQuery}
                        onChange={(e) => setPatientSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 12px 7px 30px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', outline: 'none' }}
                      />
                    </div>

                    <div style={{ display: 'flex', gap: '6px' }}>
                      <button className={`admin-filter-pill ${patientFilterCategory === 'all' ? 'active' : ''}`} onClick={() => setPatientFilterCategory('all')}>All ({Object.keys(patientStore).length})</button>
                      <button className={`admin-filter-pill ${patientFilterCategory === 'chronic' ? 'active' : ''}`} onClick={() => setPatientFilterCategory('chronic')}>Chronic Care</button>
                      <button className={`admin-filter-pill ${patientFilterCategory === 'chat' ? 'active' : ''}`} onClick={() => setPatientFilterCategory('chat')}>Active 24h Chat</button>
                      <button className={`admin-filter-pill ${patientFilterCategory === 'pediatric' ? 'active' : ''}`} onClick={() => setPatientFilterCategory('pediatric')}>Pediatric</button>
                    </div>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Patient Name</th>
                        <th>Patient ID</th>
                        <th>Demographics</th>
                        <th>Completed Video Visits</th>
                        <th>24h Chat Sessions</th>
                        <th>Total Spend (KES )</th>
                        <th>Blood Group / Cohort</th>
                        <th>Last Consultation</th>
                        <th>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredPatients.map((pt) => (
                        <tr
                          key={pt.key}
                          className="admin-clickable-row"
                          onClick={() => setSelectedPatientKey(pt.key)}
                        >
                          <td>
                            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                              <div style={{ width: '32px', height: '32px', borderRadius: '50%', background: pt.bgColor + '20', color: pt.bgColor, display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '11px' }}>
                                {pt.avatar}
                              </div>
                              <div>
                                <strong style={{ color: '#0F172A', fontSize: '13px' }}>{pt.name}</strong>
                                <div style={{ fontSize: '10px', color: '#64748B' }}>{pt.meta.split('•')[1]}</div>
                              </div>
                            </div>
                          </td>
                          <td><span className="badge badge-slate" style={{ fontWeight: 750 }}>{pt.id}</span></td>
                          <td>{pt.meta.split('•')[0]}</td>
                          <td><b>{pt.videoVisits ?? 0}</b></td>
                          <td>{pt.chatSessions ?? 0}</td>
                          <td><b style={{ color: '#0F172A', fontSize: '13px' }}>{money(pt.totalSpend)}</b></td>
                          <td>
                            <span className="badge" style={{ background: '#FEE2E2', color: '#DC2626', fontSize: '9.5px', fontWeight: 750 }}>{pt.cohort}</span>
                          </td>
                          <td>{pt.encounters[0]?.date || 'No encounters yet'}{pt.encounters[0] && <div style={{ fontSize: '10px', color: '#64748B' }}>by {pt.encounters[0].doctor}</div>}</td>
                          <td>
                            <button
                              className="admin-btn-sec"
                              onClick={(e) => {
                                e.stopPropagation();
                                setSelectedPatientKey(pt.key);
                              }}
                            >
                              View Dossier ↗
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* PAGE 5: Professional registration CREDENTIALING — genuinely PENDING Doctors from /admin/doctors/pending-verification */}
          {activeAdminPage === 'verification' && (
            <div className="admin-page-view active" id="adminPage_verification">
              <div style={{ display: 'grid', gridTemplateColumns: '320px 1fr', gap: '18px' }}>
                <div className="admin-card" style={{ padding: '16px' }}>
                  <div style={{ fontSize: '14px', fontWeight: 800, color: '#0F172A', marginBottom: '12px' }}>Doctor Verification Queue ({verificationQueue.length} Pending)</div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                    {verificationQueue.length === 0 ? (
                      <div style={{ padding: '16px', textAlign: 'center', color: '#64748B', fontSize: '12.5px' }}>No Doctors are awaiting professional registration review.</div>
                    ) : (
                      verificationQueue.map((d) => (
                        <div
                          key={d.id}
                          onClick={() => setSelectedVerificationId(d.id)}
                          style={{
                            padding: '12px',
                            background: d.id === selectedVerificationId ? '#ECFDF5' : '#F8FAFC',
                            border: `1px solid ${d.id === selectedVerificationId ? '#10B981' : '#E2E8F0'}`,
                            borderRadius: '12px',
                            cursor: 'pointer',
                          }}
                        >
                          <strong style={{ fontSize: '12.5px', color: d.id === selectedVerificationId ? '#065F46' : '#0F172A' }}>{d.full_name}</strong>
                          <div style={{ fontSize: '11px', color: d.id === selectedVerificationId ? '#047857' : '#475569', marginTop: '2px' }}>{d.license_number} • {d.primary_specialty}</div>
                          <div style={{ fontSize: '10px', color: '#64748B', marginTop: '4px' }}>Applied {new Date(d.created_at).toLocaleDateString()}</div>
                        </div>
                      ))
                    )}
                  </div>
                </div>

                <div className="admin-card">
                  {(() => {
                    const candidate = verificationQueue.find((d) => d.id === selectedVerificationId);
                    if (!candidate) {
                      return <div style={{ padding: '24px', textAlign: 'center', color: '#64748B', fontSize: '12.5px' }}>Select an application from the queue to review it.</div>;
                    }
                    return (
                      <>
                        <div className="admin-card-header">
                          <div>
                            <div className="admin-card-title">In Review: {candidate.full_name} ({candidate.license_number})</div>
                          </div>
                          <span className="badge badge-amber">Pending review</span>
                        </div>

                        <div style={{ background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '12px', padding: '14px', marginBottom: '18px' }}>
                          <div style={{ fontSize: '11.5px', fontWeight: 750, color: '#475569', marginBottom: '10px' }}>APPLICATION DETAILS</div>
                          <div style={{ fontSize: '12px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                            <div><b>Full Name:</b> {candidate.full_name}</div>
                            <div><b>KMPDC Registration Number:</b> {candidate.license_number}</div>
                            <div><b>Regulatory Authority:</b> Kenya Medical Practitioners and Dentists Council ({candidate.license_authority})</div>
                            <div><b>Country:</b> {candidate.license_country}</div>
                            <div><b>Status:</b> Pending review</div>
                            <div><b>Qualifications:</b> {candidate.qualifications.join(', ') || 'Not provided'}</div>
                            <div><b>Specialty:</b> {candidate.primary_specialty}</div>
                            <div><b>Experience:</b> {candidate.experience_years} years</div>
                            <div><b>Current Hospital:</b> {candidate.current_hospital}</div>
                            <div><b>Verified Phone:</b> {candidate.verified_phone}</div>
                          </div>
                        </div>

                        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', borderTop: '1px solid #E2E8F0', paddingTop: '16px' }}>
                          <button disabled={verificationBusy} className="slot-subtle-btn" style={{ background: '#FEE2E2', color: '#DC2626', border: 'none', padding: '8px 16px', borderRadius: '8px', fontSize: '12px', fontWeight: 750, cursor: 'pointer' }} onClick={() => handleVerificationAction('reject')}>
                            ✕ Reject Application
                          </button>
                          <button disabled={verificationBusy} className="admin-btn-pri" style={{ padding: '8px 16px', fontSize: '12px' }} onClick={() => handleVerificationAction('approve')}>
                            ✓ Verify Registration
                          </button>
                        </div>
                      </>
                    );
                  })()}
                </div>
              </div>
            </div>
          )}

          {/* PAGE 6: CLINICAL PRESCRIPTION AUDIT — real alerts from /admin/compliance/alerts */}
          {activeAdminPage === 'compliance' && (
            <div className="admin-page-view active" id="adminPage_compliance">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <div className="admin-card-title">Clinical e-Prescription Audit</div>
                    <div className="admin-card-subtitle">Prescriptions automatically flagged for controlled-substance keywords requiring counter-signature</div>
                  </div>
                  <span className={`badge ${complianceAlerts.length > 0 ? 'badge-amber' : 'badge-green'}`} style={{ fontWeight: 800 }}>
                    {complianceAlerts.length > 0 ? `${complianceAlerts.length} Flagged for Review` : 'All Clear'}
                  </span>
                </div>

                {complianceAlerts.length === 0 ? (
                  <div style={{ padding: '24px', textAlign: 'center', color: '#64748B', fontSize: '12.5px' }}>No prescriptions are currently flagged.</div>
                ) : (
                  complianceAlerts.map((alert) => (
                    <div key={alert.id} style={{ background: '#FEF2F2', border: '1px solid #FECACA', borderRadius: '14px', padding: '18px', marginBottom: '14px' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                          <span style={{ fontSize: '20px' }}>⚠️</span>
                          <div>
                            <strong style={{ color: '#991B1B', fontSize: '13.5px' }}>{alert.category} — {alert.rx_number}</strong>
                            <div style={{ fontSize: '11px', color: '#B91C1C', marginTop: '2px' }}>Issued by {alert.doctor_name} • Patient: {alert.patient_name}</div>
                          </div>
                        </div>
                        <span className="badge" style={{ background: '#FEE2E2', color: '#991B1B', fontWeight: 800 }}>{alert.status}</span>
                      </div>

                      <div style={{ marginTop: '14px', background: '#fff', borderRadius: '10px', padding: '14px', border: '1px solid #FCA5A5' }}>
                        <div style={{ fontSize: '11px', color: '#64748B', fontWeight: 700 }}>DOCTOR NOTES</div>
                        <div style={{ fontSize: '12px', color: '#0F172A', marginTop: '4px' }}>{alert.doctor_notes}</div>
                        <div style={{ fontSize: '11px', color: '#7F1D1D', marginTop: '10px' }}>{alert.flag_reason}</div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          )}

          {/* PAGE 7: APP ERROR LOGS */}
          {activeAdminPage === 'logs' && (
            <div className="admin-page-view active" id="adminPage_logs">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '18px', flexWrap: 'wrap', gap: '12px' }}>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <h2 style={{ fontSize: '20px', fontWeight: 850, color: '#0F172A', margin: 0, letterSpacing: '-0.4px' }}>App Incident &amp; Failure Diagnostics</h2>
                    <span className="badge" style={{ background: '#FEE2E2', color: '#991B1B', fontWeight: 800, fontSize: '11px' }}>Live Telemetry Stream</span>
                  </div>
                  <div style={{ fontSize: '12px', color: '#64748B', marginTop: '3px' }}>
                    Tracks runtime client/server exceptions, failed handshakes &amp; network errors against user, timestamp &amp; exact failed subsystem.
                  </div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  {simulationsAllowed && <button className="slot-subtle-btn" style={{ background: '#FEF2F2', color: '#DC2626', border: '1px solid #FECACA', padding: '7px 12px', borderRadius: '9px', fontSize: '11.5px', fontWeight: 750, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '5px' }} onClick={() => setSimulateModalOpen(true)}>
                    <span>⚡ Simulate App Failure</span>
                  </button>}
                  <button className="slot-subtle-btn" style={{ background: '#F8FAFC', color: '#334155', border: '1px solid #E2E8F0', padding: '7px 12px', borderRadius: '9px', fontSize: '11.5px', fontWeight: 700, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '5px' }} onClick={() => showToast('Logs Exported 📥', `${errorLogs.length} incidents exported as JSON file.`, '📥', 'success')}>
                    <span>📥 Export Logs (.JSON)</span>
                  </button>
                  <button className="admin-btn-pri" onClick={() => { refreshBackend(); showToast('Telemetry Synced 🔄', 'Refreshed latest error streams from the backend.', '🔄', 'info'); }} style={{ padding: '7px 14px', fontSize: '11.5px' }}>
                    <span>🔄 Refresh Feed</span>
                  </button>
                </div>
              </div>

              <div className="admin-kpi-grid" style={{ marginBottom: '18px' }}>
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Total Incidents Logged</span>
                    <div className="admin-kpi-icon-circle amber" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#FEF3C7', color: '#D97706', display: 'grid', placeItems: 'center' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{errorLogs.length} Incidents</div>
                  <span className="admin-kpi-delta" style={{ color: '#DC2626', fontSize: '11px', fontWeight: 700 }}>● {unresolvedLogsCount} Unresolved</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Primary Failure Hotspot</span>
                    <div className="admin-kpi-icon-circle rose" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#FFE4E6', color: '#E11D48', display: 'grid', placeItems: 'center' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><rect x="1" y="4" width="22" height="16" rx="2" ry="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg>
                    </div>
                  </div>
                  {(() => {
                    const counts: Record<string, number> = {};
                    errorLogs.forEach((l) => { counts[l.subsystem] = (counts[l.subsystem] ?? 0) + 1; });
                    const top = Object.entries(counts).sort((a, b) => b[1] - a[1])[0];
                    return (
                      <>
                        <div className="admin-kpi-val" style={{ fontSize: '19px', color: '#BE123C' }}>{top ? top[0] : 'No Incidents'}</div>
                        <span className="admin-kpi-delta" style={{ color: '#B45309', fontSize: '11px', fontWeight: 700 }}>{top ? `${top[1]} of ${errorLogs.length} incidents` : 'Nothing logged yet'}</span>
                      </>
                    );
                  })()}
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Impacted User Accounts</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#EFF6FF', color: '#2563EB', display: 'grid', placeItems: 'center' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{new Set(errorLogs.map((l) => l.user.id)).size} Users</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Critical Incidents</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#ECFDF5', color: '#059669', display: 'grid', placeItems: 'center' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#047857' }}>{errorLogs.filter((l) => l.severity === 'CRITICAL').length}</div>
                </div>
              </div>

              <div className="admin-card">
                <div style={{ padding: '16px 20px', borderBottom: '1px solid #E2E8F0', background: '#FAFAFA' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '12px', marginBottom: '12px' }}>
                    <div style={{ position: 'relative', flex: 1, minWidth: '280px', maxWidth: '480px' }}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
                        maxLength={150}
                        placeholder="Filter by User, Trace ID, Subsystem, Error Code..."
                        value={logSearchQuery}
                        onChange={(e) => setLogSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '8px 12px 8px 34px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#fff', outline: 'none' }}
                      />
                    </div>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                      <select value={logSeverityFilter} onChange={(e) => setLogSeverityFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', color: '#334155', fontWeight: 600 }}>
                        <option value="ALL">All Severities</option>
                        <option value="CRITICAL">🔴 Critical (Fatal)</option>
                        <option value="ERROR">🟠 Error</option>
                        <option value="WARNING">🟡 Warning</option>
                      </select>

                      <select value={logSubsystemFilter} onChange={(e) => setLogSubsystemFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', color: '#334155', fontWeight: 600 }}>
                        <option value="ALL">All Subsystems</option>
                        <option value="PAYMENT_GATEWAY">💳 Payment Gateway</option>
                        <option value="AGORA_RTC">📹 Agora WebRTC</option>
                        <option value="DATABASE">🗄️ Database</option>
                        <option value="DOCTOR_FLUTTER">Doctor Flutter</option>
                        <option value="DOCTOR_WEB">Doctor Web</option>
                        <option value="INTERNAL">⚙️ Internal</option>
                        <option value="DEPENDENCY">🔌 Dependency</option>
                        <option value="CLINICAL_RX_ENGINE">💊 Clinical Rx Engine</option>
                        <option value="SYNC_OFFLINE_CACHE">📶 Sync &amp; Offline</option>
                        <option value="PROFESSIONAL_KYC_HUB">🏛️ Professional KYC Hub</option>
                        <option value="AUTH_OTP_GATEWAY">📱 Auth / OTP Gateway</option>
                      </select>

                      <select value={logStatusFilter} onChange={(e) => setLogStatusFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', color: '#334155', fontWeight: 600 }}>
                        <option value="ALL">All Statuses</option>
                        <option value="UNRESOLVED">⚠️ Unresolved</option>
                        <option value="INVESTIGATING">🔍 Investigating</option>
                        <option value="RESOLVED">✓ Resolved</option>
                      </select>
                    </div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap', fontSize: '11px' }}>
                    <span style={{ color: '#64748B', fontWeight: 700, marginRight: '4px' }}>Quick Filters:</span>
                    <button className={`log-quick-pill ${logQuickFilter === 'all' ? 'active' : ''}`} onClick={() => setLogQuickFilter('all')}>All Logs ({errorLogs.length})</button>
                    <button className={`log-quick-pill ${logQuickFilter === 'critical' ? 'active' : ''}`} onClick={() => setLogQuickFilter('critical')}>🔴 Critical</button>
                    <button className={`log-quick-pill ${logQuickFilter === 'payment' ? 'active' : ''}`} onClick={() => setLogQuickFilter('payment')}>💳 M-Pesa provider</button>
                    <button className={`log-quick-pill ${logQuickFilter === 'webrtc' ? 'active' : ''}`} onClick={() => setLogQuickFilter('webrtc')}>📹 WebRTC</button>
                    <button className={`log-quick-pill ${logQuickFilter === 'unresolved' ? 'active' : ''}`} onClick={() => setLogQuickFilter('unresolved')}>⚠️ Unresolved</button>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th style={{ width: '130px' }}>Time &amp; Trace ID</th>
                        <th style={{ width: '180px' }}>Impacted User</th>
                        <th style={{ width: '170px' }}>Failed Subsystem</th>
                        <th>Failure Rationale &amp; Error Signature</th>
                        <th style={{ width: '90px' }}>Severity</th>
                        <th style={{ width: '110px' }}>Status</th>
                        <th style={{ width: '130px', textAlign: 'right' }}>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredLogs.map((log) => (
                        <tr key={log.id} className="admin-clickable-row" onClick={() => setSelectedLogId(log.id)}>
                          <td>
                            <div style={{ fontWeight: 750, color: '#0F172A', fontSize: '12px' }}>{log.timeFormatted}</div>
                            <div style={{ fontSize: '10px', color: '#64748B' }}>{log.relativeTime}</div>
                            <span style={{ fontFamily: 'monospace', fontSize: '9.5px', background: '#F1F5F9', color: '#475569', padding: '1px 5px', borderRadius: '4px', marginTop: '3px', display: 'inline-block' }}>{log.traceId}</span>
                          </td>
                          <td>
                            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                              <div style={{ width: '28px', height: '28px', borderRadius: '50%', background: log.user.avatarBg, color: '#fff', display: 'grid', placeItems: 'center', fontSize: '11px', fontWeight: 800 }}>{log.user.avatar}</div>
                              <div>
                                <div style={{ fontWeight: 750, color: '#0F172A', fontSize: '12px' }}>{log.user.name}</div>
                                <div style={{ fontSize: '10px', color: '#64748B' }}>{log.user.phone}</div>
                              </div>
                            </div>
                          </td>
                          <td>
                            <div style={{ fontWeight: 750, color: '#BE123C', fontSize: '11.5px' }}>{log.failedPart}</div>
                            <span className="badge" style={{ background: '#FFF1F2', color: '#9F1239', fontSize: '9.5px', padding: '1px 5px', marginTop: '2px' }}>{log.subsystem}</span>
                          </td>
                          <td>
                            <div style={{ fontSize: '11.5px', fontWeight: 700, color: '#0F172A' }}>{log.errorMessage}</div>
                            <span className="badge" style={{ background: '#0F172A', color: '#38BDF8', fontFamily: 'monospace', fontSize: '9.5px', marginTop: '4px' }}>{log.errorCode}</span>
                          </td>
                          <td>
                            <span className={`badge ${log.severity === 'CRITICAL' ? 'badge-rose' : 'badge-amber'}`}>{log.severity}</span>
                          </td>
                          <td>
                            <span className={`badge ${log.status === 'RESOLVED' ? 'badge-green' : log.status === 'INVESTIGATING' ? 'badge-blue' : 'badge-rose'}`}>
                              {log.status === 'RESOLVED' ? '✓ Resolved' : log.status === 'INVESTIGATING' ? '🔍 Investigating' : '⚠️ Unresolved'}
                            </span>
                          </td>
                          <td style={{ textAlign: 'right' }}>
                            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '6px', alignItems: 'center' }}>
                              <button className="admin-btn-sec" onClick={(e) => { e.stopPropagation(); setSelectedLogId(log.id); }} style={{ padding: '4px 8px', fontSize: '11px' }}>Inspect 🔍</button>
                              {log.status !== 'RESOLVED' && (
                                <button disabled={logActionBusy === log.id} className="slot-subtle-btn" onClick={(e) => { e.stopPropagation(); handleResolveLog(log.id); }} style={{ padding: '4px 7px', fontSize: '11px' }} title="Mark as Resolved">
                                  ✓
                                </button>
                              )}
                            </div>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* PAGE 8: CONSULTATION GRIEVANCES */}
          {activeAdminPage === 'grievances' && (
            <div className="admin-page-view active" id="adminPage_grievances">
              <div className="admin-kpi-grid" style={{ gridTemplateColumns: 'repeat(4, 1fr)', marginBottom: '20px' }}>
                <div className="admin-kpi-card">
                  <span className="admin-kpi-sub">Total Grievances Filed</span>
                  <span className="admin-kpi-val" style={{ color: '#0F172A' }}>{grievances.length}</span>
                  <span className="admin-kpi-delta down">{pendingGrievancesCount} Pending Adjudication</span>
                </div>
                <div className="admin-kpi-card">
                  <span className="admin-kpi-sub">Claims Against Doctors</span>
                  <span className="admin-kpi-val" style={{ color: '#DC2626' }}>{grievances.filter((g) => g.target === 'DOCTOR').length}</span>
                </div>
                <div className="admin-kpi-card">
                  <span className="admin-kpi-sub">Claims Against System</span>
                  <span className="admin-kpi-val" style={{ color: '#2563EB' }}>{grievances.filter((g) => g.target === 'SYSTEM').length}</span>
                </div>
                <div className="admin-kpi-card">
                  <span className="admin-kpi-sub">Adjudicated Rate</span>
                  <span className="admin-kpi-val" style={{ color: '#059669' }}>
                    {grievances.length === 0 ? '—' : `${Math.round(((grievances.length - pendingGrievancesCount) / grievances.length) * 100)}%`}
                  </span>
                  <span className="admin-kpi-delta neutral">{grievances.length - pendingGrievancesCount} of {grievances.length} resolved</span>
                </div>
              </div>

              <div className="admin-card">
                <div style={{ padding: '16px 20px', borderBottom: '1px solid #E2E8F0', background: '#FAFAFA' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '12px', marginBottom: '12px' }}>
                    <div style={{ position: 'relative', flex: 1, minWidth: '280px', maxWidth: '480px' }}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
                        maxLength={150}
                        placeholder="Filter by Patient, Doctor, Professional registration #, Dispute ID, Category..."
                        value={grievanceSearchQuery}
                        onChange={(e) => setGrievanceSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '8px 12px 8px 34px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#fff', outline: 'none' }}
                      />
                    </div>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                      <select value={grievanceTargetFilter} onChange={(e) => setGrievanceTargetFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', color: '#334155', fontWeight: 600 }}>
                        <option value="ALL">All Targets</option>
                        <option value="DOCTOR">🩺 Against Doctor Only</option>
                        <option value="SYSTEM">⚙️ Against System Only</option>
                      </select>

                      <select value={grievanceStatusFilter} onChange={(e) => setGrievanceStatusFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', color: '#334155', fontWeight: 600 }}>
                        <option value="ALL">All Statuses</option>
                        <option value="PENDING_REVIEW">⚠️ Pending Review</option>
                        <option value="UNDER_INVESTIGATION">🔍 Under Investigation</option>
                        <option value="REFUNDED">💳 Refunded</option>
                        <option value="WARNED">⚠️ Doctor Warned</option>
                        <option value="DISMISSED">✓ Dismissed</option>
                      </select>
                    </div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap', fontSize: '11px' }}>
                    <span style={{ color: '#64748B', fontWeight: 700, marginRight: '4px' }}>Quick Filters:</span>
                    <button className={`grv-quick-pill ${grievanceQuickFilter === 'all' ? 'active' : ''}`} onClick={() => setGrievanceQuickFilter('all')}>All ({grievances.length})</button>
                    <button className={`grv-quick-pill ${grievanceQuickFilter === 'doctor' ? 'active' : ''}`} onClick={() => setGrievanceQuickFilter('doctor')}>🩺 Doctor Claims</button>
                    <button className={`grv-quick-pill ${grievanceQuickFilter === 'system' ? 'active' : ''}`} onClick={() => setGrievanceQuickFilter('system')}>⚙️ System Claims</button>
                    <button className={`grv-quick-pill ${grievanceQuickFilter === 'pending' ? 'active' : ''}`} onClick={() => setGrievanceQuickFilter('pending')}>⚠️ Pending Action</button>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th style={{ width: '130px' }}>Dispute ID &amp; Time</th>
                        <th style={{ width: '180px' }}>Complainant (Patient)</th>
                        <th style={{ width: '180px' }}>Consultation Doctor</th>
                        <th style={{ width: '140px' }}>Report Target</th>
                        <th>Issue Summary &amp; Patient Claim</th>
                        <th style={{ width: '150px' }}>Board Action / Remedy</th>
                        <th style={{ width: '110px' }}>Status</th>
                        <th style={{ width: '120px', textAlign: 'right' }}>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredGrievances.map((grv) => (
                        <tr key={grv.id} style={{ cursor: 'pointer' }} onClick={() => setSelectedGrievanceId(grv.id)}>
                          <td>
                            <div style={{ fontWeight: 850, fontSize: '12px', color: '#0F172A' }}>{grv.id}</div>
                            <div style={{ fontSize: '10.5px', color: '#64748B', marginTop: '2px' }}>{grv.timeFormatted}</div>
                            <div style={{ fontSize: '10px', color: '#94A3B8' }}>Ref: #{grv.consultationId}</div>
                          </td>
                          <td>
                            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                              <div style={{ width: '28px', height: '28px', borderRadius: '50%', background: '#E0F2FE', color: '#0284C7', fontWeight: 800, fontSize: '11px', display: 'grid', placeItems: 'center' }}>
                                {grv.patient.name.split(' ').map((n) => n[0]).join('')}
                              </div>
                              <div>
                                <div style={{ fontWeight: 800, fontSize: '12px', color: '#0F172A' }}>{grv.patient.name}</div>
                                <div style={{ fontSize: '10.5px', color: '#64748B' }}>{grv.patient.phone}</div>
                              </div>
                            </div>
                          </td>
                          <td>
                            <div style={{ fontWeight: 800, fontSize: '12px', color: '#0F172A' }}>{grv.doctor.name}</div>
                            <div style={{ fontSize: '10.5px', color: '#64748B' }}>{grv.doctor.registrationNumber}</div>
                          </td>
                          <td>
                            <span className={`badge ${grv.target === 'DOCTOR' ? 'badge-rose' : 'badge-blue'}`}>
                              {grv.target === 'DOCTOR' ? '🩺 Against Doctor' : '⚙️ Against System'}
                            </span>
                          </td>
                          <td>
                            <div style={{ fontWeight: 750, fontSize: '11.5px', color: '#0F172A', marginBottom: '2px' }}>{grv.category}</div>
                            <div style={{ fontSize: '11px', color: '#475569', maxWidth: '320px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                              {grv.claimSummary}
                            </div>
                          </td>
                          <td>
                            <span className="badge" style={{ background: '#F8FAFC', border: '1px solid #CBD5E1', color: '#0F172A', fontSize: '10px', fontWeight: 700 }}>
                              {grv.boardRemedy}
                            </span>
                          </td>
                          <td>
                            <span className={`badge ${grv.status === 'REFUNDED' ? 'badge-green' : grv.status === 'WARNED' ? 'badge-rose' : 'badge-amber'}`}>
                              {grv.status.replace(/_/g, ' ')}
                            </span>
                          </td>
                          <td style={{ textAlign: 'right' }}>
                            <button className="admin-btn-sec" onClick={(e) => { e.stopPropagation(); setSelectedGrievanceId(grv.id); }} style={{ padding: '5px 9px', fontSize: '11px', fontWeight: 700 }}>
                              Adjudicate ›
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* PAGE 9: OMNICHANNEL PAYMENTS & ESCROW MASTER LEDGER */}
          {activeAdminPage === 'finance' && (
            <div className="admin-page-view active" id="adminPage_finance" style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '12px' }}>
                <div>
                  <h2 style={{ fontSize: '20px', fontWeight: 850, color: '#0F172A', margin: 0, letterSpacing: '-0.4px' }}>Omnichannel Payment &amp; Escrow Master Ledger</h2>
                  <div style={{ fontSize: '12px', color: '#64748B', marginTop: '2px' }}>Centralized financial audit tracking all patient inflows, gateway escrow locks, 20% platform commission cuts, doctor payouts &amp; refund reversals</div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <button className="admin-action-btn" onClick={() => setActiveAdminPage('doctors')} style={{ background: '#F8FAFC', border: '1px solid #E2E8F0', color: '#334155', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '8px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v12M15 9.5a3.5 3.5 0 0 0-7 0c0 2 1.5 3 3.5 3s3.5 1 3.5 3a3.5 3.5 0 0 1-7 0"/></svg>
                    <span>Doctor Payouts Hub →</span>
                  </button>
                  <button className="admin-action-btn" onClick={() => showToast('MFS Webhooks Reconciled', '0 discrepancies detected across Kenyan payment rails payment gateways.', '✓', 'success')} style={{ background: '#EFF6FF', border: '1px solid #BFDBFE', color: '#1D4ED8', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '8px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                    <span>Reconcile MFS Webhooks</span>
                  </button>
                  <button className="admin-action-btn" onClick={() => showToast('Ledger CSV Exported', 'Full financial audit export downloaded.', '📄', 'success')} style={{ background: '#0F172A', border: '1px solid #0F172A', color: '#FFFFFF', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '8px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                    <span>Export Ledger CSV</span>
                  </button>
                </div>
              </div>

              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Total Platform GMV Inflow</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(financeSummary?.total_gross)}</div>
                  <span className="admin-kpi-delta neutral">{transactions.length} transaction{transactions.length === 1 ? '' : 's'}</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Active Escrow Locked</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#EFF6FF', color: '#2563EB' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#2563EB' }}>{money(transactions.filter((t) => t.escrowStatus === 'ESCROW_LOCKED').reduce((s, t) => s + t.grossAmount, 0))}</div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>Held securely pending doctor sign-off</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">HeloDoc 20% Commission</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#ECFDF5', color: '#059669' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#059669' }}>{money(financeSummary?.total_platform_fee)}</div>
                  <span className="admin-kpi-delta up">Net Platform Operating Revenue</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Total Net Payable to Doctors</span>
                    <div className="admin-kpi-icon-circle purple" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#F5F3FF', color: '#7C3AED' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="13 17 18 12 13 7"/><polyline points="6 17 11 12 6 7"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(financeSummary?.total_net)}</div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>Settled + pending, across all doctors</span>
                </div>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '11px', color: '#64748B', margin: '-8px 0 16px' }}>
                <span>ℹ️</span>
                <span>Total calculation is based including the platform charge 20%.</span>
              </div>

              <div className="admin-card">
                <div className="admin-card-header" style={{ flexWrap: 'wrap', gap: '12px' }}>
                  <div>
                    <div className="admin-card-title">Omnichannel Financial Audit Table</div>
                    <div className="admin-card-subtitle">Complete ledger of MFS gateway checkouts, fee splits, doctor wallets, and clinical escrow holds</div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                    <div style={{ position: 'relative', width: '260px' }}>
                      <input
                        type="text"
                        maxLength={150}
                        placeholder="Search TxID, Patient, Doctor, Ref..."
                        value={financeSearchQuery}
                        onChange={(e) => setFinanceSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 10px 7px 32px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', outline: 'none', background: '#FFFFFF' }}
                      />
                    </div>

                    <select value={financeGatewayFilter} onChange={(e) => setFinanceGatewayFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#FFFFFF', color: '#334155', outline: 'none' }}>
                      <option value="ALL">All Gateways</option>
                      <option value="MPESA">M-Pesa</option>
                      <option value="BANK_TRANSFER">Kenyan bank transfer</option>
                      <option value="CARD">Card (if payout-capable)</option>
                      <option value="ALL">All configured providers</option>
                    </select>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '4px', background: '#F1F5F9', padding: '3px', borderRadius: '8px' }}>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'ALL' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('ALL')}>All</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'INFLOW' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('INFLOW')}>Inflow 📥</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'ESCROW' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('ESCROW')}>Escrow 🔒</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'PAYOUT' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('PAYOUT')}>Payout 📤</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'REFUND' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('REFUND')}>Refund ↩️</button>
                    </div>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>TxID &amp; MFS Ref</th>
                        <th>Timestamp</th>
                        <th>Channel &amp; Type</th>
                        <th>Consultation / Case</th>
                        <th>Parties (Patient / Doctor)</th>
                        <th style={{ textAlign: 'right' }}>Gross (KES )</th>
                        <th style={{ textAlign: 'right' }}>HeloDoc Cut</th>
                        <th style={{ textAlign: 'right' }}>Net (KES )</th>
                        <th>Gateway &amp; Escrow State</th>
                        <th style={{ textAlign: 'center' }}>Action</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredTransactions.map((tx) => (
                        <tr key={tx.txId} className="admin-clickable-row" onClick={() => setSelectedTransactionId(tx.txId)}>
                          <td>
                            <div style={{ fontWeight: 800, color: '#0F172A', fontSize: '12.5px' }}>{tx.txId}</div>
                            <div style={{ fontSize: '10px', color: '#64748B', fontFamily: 'monospace' }}>{tx.gatewayRef}</div>
                          </td>
                          <td style={{ fontSize: '11.5px', color: '#334155', whiteSpace: 'nowrap' }}>{tx.timestamp}</td>
                          <td>
                            <span className={`payout-method-badge ${tx.gateway === 'MFS Bulk API' ? 'mpesa' : tx.gateway === 'Card' ? 'card' : 'bank'}`}>
                              {tx.gatewayLabel}
                            </span>
                            <div style={{ fontSize: '10px', color: '#64748B', marginTop: '2px' }}>{tx.typeLabel}</div>
                          </td>
                          <td>
                            <b style={{ color: '#2563EB', fontSize: '12px' }}>{tx.consultationId}</b>
                            <div style={{ fontSize: '10px', color: '#64748B' }}>{tx.sessionType}</div>
                          </td>
                          <td>
                            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                              <span style={{ fontSize: '12px', fontWeight: 700, color: '#0F172A' }}>{tx.patient.name}</span>
                              <span style={{ color: '#94A3B8' }}>→</span>
                              <span style={{ fontSize: '12px', fontWeight: 700, color: '#0F172A' }}>{tx.doctor.name}</span>
                            </div>
                            <div style={{ fontSize: '10px', color: '#64748B' }}>{tx.patient.phone} • {tx.doctor.registrationNumber}</div>
                          </td>
                          <td style={{ textAlign: 'right', fontWeight: 800, color: '#0F172A', fontSize: '13px' }}>KES  {tx.grossAmount.toLocaleString()}</td>
                          <td style={{ textAlign: 'right', fontSize: '12px', color: '#059669', fontWeight: 700 }}>KES  {tx.platformFeeAmount.toLocaleString()} <span style={{ fontSize: '9.5px', color: '#64748B' }}>({tx.platformFeePercent}%)</span></td>
                          <td style={{ textAlign: 'right', fontWeight: 850, color: '#2563EB', fontSize: '13.5px' }}>KES  {tx.netAmount.toLocaleString()}</td>
                          <td>
                            <span className={`badge ${tx.statusBadgeCls}`}>{tx.escrowStatusLabel}</span>
                          </td>
                          <td style={{ textAlign: 'center' }}>
                            <button onClick={(e) => { e.stopPropagation(); setSelectedTransactionId(tx.txId); }} style={{ padding: '4px 8px', border: '1px solid #CBD5E1', background: '#FFFFFF', borderRadius: '6px', fontSize: '11px', fontWeight: 700, color: '#334155', cursor: 'pointer' }}>
                              Receipt 🔍
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              <div className="admin-card">
                <div className="admin-card-header" style={{ flexWrap: 'wrap', gap: '12px' }}>
                  <div>
                    <div className="admin-card-title">Refund Request Resolution</div>
                    <div className="admin-card-subtitle">Board-approved refunds awaiting Finance execution against the payment provider — distinct from grievance adjudication, which only approves the refund</div>
                  </div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>{refundRequests.filter((r) => r.status !== 'REFUNDED').length} pending of {refundRequests.length}</span>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Approved</th>
                        <th>Grievance / Transaction</th>
                        <th style={{ textAlign: 'right' }}>Amount</th>
                        <th>Previous Payment Status</th>
                        <th>Provider Reference</th>
                        <th>Status</th>
                        <th style={{ textAlign: 'right' }}>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {refundRequests.length === 0 && (
                        <tr><td colSpan={7} style={{ textAlign: 'center', padding: '20px', color: '#94A3B8', fontSize: '12px' }}>No refund requests. These appear once a grievance refund is board-approved.</td></tr>
                      )}
                      {refundRequests.map((r) => {
                        const busy = refundBusy === r.id;
                        const terminal = r.status === 'REFUNDED';
                        const statusCls = r.status === 'REFUNDED' ? 'badge-green' : r.status === 'FAILED' || r.status === 'RECONCILIATION_REQUIRED' ? 'badge-rose' : r.status === 'PROCESSING' ? 'badge-blue' : 'badge-amber';
                        return (
                          <tr key={r.id}>
                            <td style={{ fontSize: '11.5px', color: '#334155', whiteSpace: 'nowrap' }}>{new Date(r.approved_at).toLocaleString()}</td>
                            <td style={{ fontSize: '11px', fontFamily: 'monospace', color: '#64748B' }}>{r.grievance_id.slice(0, 8)} / {r.transaction_id.slice(0, 8)}</td>
                            <td style={{ textAlign: 'right', fontWeight: 800, color: '#0F172A', fontSize: '13px' }}>{r.currency} {Number(r.amount).toLocaleString()}</td>
                            <td style={{ fontSize: '11px', color: '#64748B' }}>{r.previous_payment_status}</td>
                            <td style={{ fontSize: '11px', color: '#0F172A' }}>{r.provider_reference || '—'}</td>
                            <td><span className={`badge ${statusCls}`}>{r.status.replace(/_/g, ' ')}</span></td>
                            <td style={{ textAlign: 'right' }}>
                              {terminal ? (
                                <span style={{ fontSize: '11px', color: '#94A3B8' }}>Finalized</span>
                              ) : (
                                <div style={{ display: 'flex', gap: '4px', justifyContent: 'flex-end', flexWrap: 'wrap' }}>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'PROCESSING')} style={{ padding: '4px 8px', border: '1px solid #BFDBFE', background: '#EFF6FF', color: '#1D4ED8', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>Processing</button>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'REFUNDED')} style={{ padding: '4px 8px', border: '1px solid #86EFAC', background: '#F0FDF4', color: '#166534', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>Refunded</button>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'FAILED')} style={{ padding: '4px 8px', border: '1px solid #FECACA', background: '#FEF2F2', color: '#DC2626', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>Failed</button>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'RECONCILIATION_REQUIRED')} style={{ padding: '4px 8px', border: '1px solid #FDE68A', background: '#FFFBEB', color: '#B45309', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>Needs Reconciliation</button>
                                </div>
                              )}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

        </main>
      </div>

      {/* ── MODAL 1: DOCTOR HISTORY & FINANCIAL DOSSIER ── */}
      {selectedDocObj && (
        <div className="admin-modal-backdrop" id="adminDoctorHistoryModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSelectedDoctorKey(null); }}>
          <div className="admin-modal-card">
            <div style={{ background: '#0F172A', color: '#fff', padding: '18px 22px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <div style={{ width: '48px', height: '48px', borderRadius: '50%', background: selectedDocObj.bgColor, color: '#fff', display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '16px', flexShrink: 0 }}>
                  {selectedDocObj.avatar}
                </div>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                    <span style={{ fontSize: '16.5px', fontWeight: 800, color: '#FFFFFF' }}>{selectedDocObj.name}</span>
                    <span className="badge badge-green">{selectedDocObj.registrationNumber}</span>
                    <span className="badge badge-blue">Verified Specialist</span>
                    <span className="badge" style={{ background: 'rgba(255,255,255,0.12)', color: '#E2E8F0', fontSize: '10px' }}>Active Practitioner</span>
                  </div>
                  <div style={{ fontSize: '11.5px', color: '#94A3B8', marginTop: '2px' }}>{selectedDocObj.meta}</div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginTop: '6px', fontSize: '11px', flexWrap: 'wrap' }}>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px', color: '#94A3B8' }}>
                      <span>Phone:</span> <b style={{ color: '#FFFFFF', fontFamily: 'monospace' }}>{selectedDocObj.phone}</b>
                    </span>
                    <span style={{ color: '#475569' }}>•</span>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px', color: '#94A3B8' }}>
                      <span>Residence:</span> <b style={{ color: '#FFFFFF' }}>{selectedDocObj.residence}</b>
                    </span>
                    <span style={{ color: '#475569' }}>•</span>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px', color: '#94A3B8' }}>
                      <span>Email:</span> <b style={{ color: '#E2E8F0' }}>{selectedDocObj.email}</b>
                    </span>
                  </div>
                </div>
              </div>
              <button onClick={() => setSelectedDoctorKey(null)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '20px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '1px', background: '#E2E8F0', borderBottom: '1px solid #E2E8F0' }}>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Lifetime Consultations</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#0F172A', marginTop: '2px' }}>{selectedDocObj.kpiVisits}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>24h Chat Sessions</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#2563EB', marginTop: '2px' }}>{selectedDocObj.kpiChats}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Total Gross Revenue</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#059669', marginTop: '2px' }}>{selectedDocObj.kpiGross}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Net Payouts Disbursed</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#047857', marginTop: '2px' }}>{selectedDocObj.kpiDisbursed}</div>
              </div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '10.5px', color: '#64748B', padding: '8px 16px 0' }}>
              <span>ℹ️</span>
              <span>Total calculation is based including the platform charge 20%.</span>
            </div>

            <div style={{ padding: '18px 22px', overflowY: 'auto', flex: 1 }}>
              {selectedDocObj.payoutRouting?.disbursementItemId && selectedDocObj.payoutRouting.disbursementStatus === 'PENDING_APPROVAL' && (
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '12px', padding: '12px 14px', marginBottom: '16px', border: '1px solid #FCD34D', borderRadius: '10px', background: '#FFFBEB' }}>
                  <div>
                    <strong style={{ fontSize: '12px', color: '#92400E' }}>KES disbursement awaiting Finance approval</strong>
                    <div style={{ fontSize: '10.5px', color: '#78350F', marginTop: '2px' }}>Approval records authorization only; payment remains unconfirmed until a provider integration reports success.</div>
                  </div>
                  <button className="admin-btn-pri" disabled={financeBusy} onClick={() => handleApproveDoctorDisbursement(selectedDocObj.payoutRouting!.disbursementItemId!)}>Approve</button>
                </div>
              )}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                <strong style={{ fontSize: '13.5px', color: '#0F172A' }}>Recent Consultation Encounters &amp; Issued Prescriptions</strong>
                <span style={{ fontSize: '11px', color: '#64748B' }}>Archived across all patient encounters</span>
              </div>

              <table className="admin-table">
                <thead>
                  <tr>
                    <th>Date &amp; Time</th>
                    <th>Patient Name</th>
                    <th>Consultation Mode</th>
                    <th>Fee (KES )</th>
                    <th>Clinical Diagnosis</th>
                    <th>Rx Issued</th>
                  </tr>
                </thead>
                <tbody>
                  {selectedDocObj.encounters.map((enc, idx) => (
                    <tr key={idx}>
                      <td>{enc.time}</td>
                      <td><b>{enc.ptName}</b></td>
                      <td><span className={`badge ${enc.badgeCls}`}>{enc.mode}</span></td>
                      <td>{enc.fee}</td>
                      <td>{enc.diagnosis}</td>
                      <td><span className="badge badge-blue">{enc.rx}</span></td>
                    </tr>
                  ))}
                </tbody>
              </table>

              <div style={{ marginTop: '20px' }}>
                <strong style={{ fontSize: '13.5px', color: '#0F172A' }}>Recent Financial Wallet Disbursements (20% Platform Fee Enforced)</strong>
                <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  {selectedDocObj.disbursements.map((dsb, idx) => (
                    <div key={idx} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 14px', background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '10px', fontSize: '11.5px' }}>
                      <div>
                        <b>{dsb.date}: {dsb.amount}</b> Assigned destination <span className="badge" style={{ background: '#FCE7F3', color: '#DB2777', fontWeight: 800 }}>{dsb.method}</span>
                        <div style={{ fontSize: '10px', color: '#64748B' }}>TxID: {dsb.txId} • Commission Deducted: {dsb.commission}</div>
                      </div>
                      <span className="badge badge-green">{dsb.status}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: '11px', color: '#64748B' }}>Professional registration Verified Credential Ledger • HelloDoctor Kenya</span>
              <button className="admin-btn-sec" onClick={() => setSelectedDoctorKey(null)}>Close Dossier</button>
            </div>
          </div>
        </div>
      )}

      {/* ── MODAL 2: PATIENT CLINICAL DOSSIER & VAULT ── */}
      {selectedPatObj && (
        <div className="admin-modal-backdrop" id="adminPatientDossierModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSelectedPatientKey(null); }}>
          <div className="admin-modal-card">
            <div style={{ background: '#0F172A', color: '#fff', padding: '18px 22px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <div style={{ width: '44px', height: '44px', borderRadius: '50%', background: selectedPatObj.bgColor, color: '#fff', display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '15px' }}>
                  {selectedPatObj.avatar}
                </div>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontSize: '16px', fontWeight: 800 }}>{selectedPatObj.name}</span>
                    <span className="badge badge-slate">{selectedPatObj.id}</span>
                    {selectedPatObj.chatActive && <span className="badge badge-green">Active 24h Chat</span>}
                  </div>
                  <div style={{ fontSize: '11.5px', color: '#94A3B8', marginTop: '2px' }}>{selectedPatObj.meta}</div>
                </div>
              </div>
              <button onClick={() => setSelectedPatientKey(null)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '20px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '1px', background: '#E2E8F0', borderBottom: '1px solid #E2E8F0' }}>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Video Visits</div>
                <div style={{ fontSize: '16px', fontWeight: 800, color: '#0F172A', marginTop: '2px' }}>{selectedPatObj.videoVisits ?? 0}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>24h Chat Sessions</div>
                <div style={{ fontSize: '16px', fontWeight: 800, color: '#0F172A', marginTop: '2px' }}>{selectedPatObj.chatSessions ?? 0}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Blood Group / Cohort</div>
                <div style={{ fontSize: '13px', fontWeight: 800, color: '#2563EB', marginTop: '4px' }}>{selectedPatObj.cohort}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Attached Lab Panels</div>
                <div style={{ fontSize: '13px', fontWeight: 800, color: '#059669', marginTop: '4px' }}>{selectedPatObj.labs}</div>
              </div>
            </div>

            <div style={{ padding: '18px 22px', overflowY: 'auto', flex: 1 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                <strong style={{ fontSize: '13.5px', color: '#0F172A' }}>Patient Lifetime Consultation History</strong>
                <span style={{ fontSize: '11px', color: '#64748B' }}>Chronological electronic health record</span>
              </div>

              <table className="admin-table">
                <thead>
                  <tr>
                    <th>Date</th>
                    <th>Attending Physician</th>
                    <th>Specialty</th>
                    <th>Consultation Mode</th>
                    <th>Primary Clinical Finding</th>
                    <th>Issued e-Rx</th>
                  </tr>
                </thead>
                <tbody>
                  {selectedPatObj.encounters.map((enc, idx) => (
                    <tr key={idx}>
                      <td>{enc.date}</td>
                      <td><b>{enc.doctor}</b></td>
                      <td>{enc.spec}</td>
                      <td><span className={`badge ${enc.badgeCls}`}>{enc.mode}</span></td>
                      <td>{enc.diag}</td>
                      <td><span className="badge badge-blue">{enc.rx}</span></td>
                    </tr>
                  ))}
                </tbody>
              </table>

              <div style={{ marginTop: '20px' }}>
                <strong style={{ fontSize: '13.5px', color: '#0F172A' }}>Health Vault Attached Diagnostic Tests &amp; Prescriptions</strong>
                <div style={{ marginTop: '8px', display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
                  <div style={{ padding: '10px 14px', background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '10px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div>
                      <strong style={{ fontSize: '12px', color: '#0F172A' }}>📄 Lipid_Profile_Panel.pdf</strong>
                      <div style={{ fontSize: '10px', color: '#64748B' }}>Cholesterol: 215 mg/dL • Attached 18 Sep 2026</div>
                    </div>
                    <button className="admin-btn-sec" onClick={() => showToast('Opening Report', 'Displaying diagnostic report from patient vault.', '📄', 'info')}>View PDF</button>
                  </div>
                  <div style={{ padding: '10px 14px', background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '10px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div>
                      <strong style={{ fontSize: '12px', color: '#0F172A' }}>📄 CBC_Hemogram_Report.pdf</strong>
                      <div style={{ fontSize: '10px', color: '#64748B' }}>Hb: 12.8 g/dL • Attached 12 Aug 2026</div>
                    </div>
                    <button className="admin-btn-sec" onClick={() => showToast('Opening Report', 'Displaying diagnostic report from patient vault.', '📄', 'info')}>View PDF</button>
                  </div>
                </div>
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: '11px', color: '#64748B' }}>Confidential Medical Record • HelloDoctor Health Vault</span>
              <button className="admin-btn-sec" onClick={() => setSelectedPatientKey(null)}>Close Dossier</button>
            </div>
          </div>
        </div>
      )}

      {/* ── MODAL 3: LOG DETAIL MODAL ── */}
      {selectedLogObj && (
        <div className="admin-modal-backdrop" id="adminLogDetailModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSelectedLogId(null); }}>
          <div className="admin-modal-card" style={{ maxWidth: '920px' }}>
            <div style={{ background: '#0F172A', color: '#fff', padding: '18px 22px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <div style={{ width: '42px', height: '42px', borderRadius: '12px', background: '#DC2626', color: '#fff', display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '18px' }}>
                  ⚠️
                </div>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontSize: '16px', fontWeight: 800, letterSpacing: '0.5px', fontFamily: 'monospace' }}>{selectedLogObj.traceId}</span>
                    <span className="badge" style={{ background: '#FEE2E2', color: '#991B1B', fontWeight: 800 }}>{selectedLogObj.severity}</span>
                    <span className="badge" style={{ background: selectedLogObj.status === 'RESOLVED' ? '#DCFCE7' : '#FEF3C7', color: selectedLogObj.status === 'RESOLVED' ? '#166534' : '#D97706', fontWeight: 800 }}>
                      {selectedLogObj.status}
                    </span>
                  </div>
                  <div style={{ fontSize: '11.5px', color: '#94A3B8', marginTop: '3px' }}>Logged: {selectedLogObj.timeFormatted} ({selectedLogObj.relativeTime})</div>
                </div>
              </div>
              <button onClick={() => setSelectedLogId(null)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '20px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ padding: '22px', overflowY: 'auto', flex: 1, display: 'flex', flexDirection: 'column', gap: '18px' }}>
              <div style={{ background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '14px', padding: '16px' }}>
                <div style={{ fontSize: '11px', fontWeight: 800, color: '#475569', textTransform: 'uppercase', letterSpacing: '0.5px', marginBottom: '12px', display: 'flex', justifyContent: 'space-between' }}>
                  <span>1. IMPACTED USER &amp; CLIENT ENVIRONMENT</span>
                  <span className="badge badge-green">{selectedLogObj.user.role} Account</span>
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <div style={{ width: '42px', height: '42px', borderRadius: '50%', background: selectedLogObj.user.avatarBg, color: '#fff', display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '14px' }}>
                      {selectedLogObj.user.avatar}
                    </div>
                    <div>
                      <div style={{ fontSize: '14px', fontWeight: 800, color: '#0F172A' }}>{selectedLogObj.user.name} ({selectedLogObj.user.ageGender})</div>
                      <div style={{ fontSize: '11px', color: '#64748B' }}>ID: {selectedLogObj.user.id} • {selectedLogObj.user.phone}</div>
                    </div>
                  </div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '3px', fontSize: '11.5px' }}>
                    <div><b style={{ color: '#475569' }}>Device:</b> <span style={{ color: '#0F172A' }}>{selectedLogObj.user.device}</span></div>
                    <div><b style={{ color: '#475569' }}>App build:</b> <span style={{ color: '#0F172A' }}>{selectedLogObj.user.appVersion}</span></div>
                    <div><b style={{ color: '#475569' }}>Network:</b> <span style={{ color: '#0F172A' }}>{selectedLogObj.user.network}</span></div>
                  </div>
                </div>
              </div>

              <div style={{ background: '#FFF1F2', border: '1px solid #FECDD3', borderRadius: '14px', padding: '16px' }}>
                <div style={{ fontSize: '11px', fontWeight: 800, color: '#9F1239', textTransform: 'uppercase', letterSpacing: '0.5px', marginBottom: '10px' }}>
                  2. FAILURE ROOT CAUSE &amp; FAILED SUBSYSTEM
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px', marginBottom: '12px' }}>
                  <div>
                    <div style={{ fontSize: '10.5px', color: '#881337', fontWeight: 700 }}>SUBSYSTEM CODENAME</div>
                    <div style={{ fontSize: '13px', fontWeight: 800, color: '#9F1239', marginTop: '2px', fontFamily: 'monospace' }}>{selectedLogObj.subsystem}</div>
                  </div>
                  <div>
                    <div style={{ fontSize: '10.5px', color: '#881337', fontWeight: 700 }}>COMPONENT CODENAME</div>
                    <div style={{ fontSize: '12px', fontFamily: 'monospace', fontWeight: 700, color: '#9F1239', marginTop: '2px' }}>{selectedLogObj.component}</div>
                  </div>
                  <div>
                    <div style={{ fontSize: '10.5px', color: '#881337', fontWeight: 700 }}>ERROR CODE / HTTP STATUS</div>
                    <div style={{ fontSize: '12px', fontFamily: 'monospace', fontWeight: 700, color: '#9F1239', marginTop: '2px' }}>{selectedLogObj.errorCode}{selectedLogObj.httpStatus ? ` (HTTP ${selectedLogObj.httpStatus})` : ''}</div>
                  </div>
                  <div>
                    <div style={{ fontSize: '10.5px', color: '#881337', fontWeight: 700 }}>ENDPOINT / OCCURRENCES</div>
                    <div style={{ fontSize: '12px', fontFamily: 'monospace', fontWeight: 700, color: '#9F1239', marginTop: '2px', wordBreak: 'break-all' }}>{selectedLogObj.endpoint || '—'} • seen {selectedLogObj.occurrenceCount}×</div>
                  </div>
                  <div>
                    <div style={{ fontSize: '10.5px', color: '#881337', fontWeight: 700 }}>SCREEN / OPERATION</div>
                    <div style={{ fontSize: '12px', fontFamily: 'monospace', fontWeight: 700, color: '#9F1239', marginTop: '2px' }}>{selectedLogObj.failedPart}</div>
                  </div>
                </div>
                <div style={{ background: '#fff', border: '1px solid #FDA4AF', borderRadius: '10px', padding: '12px' }}>
                  <div style={{ fontSize: '10.5px', color: '#64748B', fontWeight: 700 }}>CLINICAL &amp; SYSTEM ERROR SUMMARY</div>
                  <div style={{ fontSize: '12px', color: '#9F1239', marginTop: '2px', lineHeight: 1.45 }}>{selectedLogObj.errorMessage}</div>
                </div>
              </div>

              <div style={{ background: '#0F172A', borderRadius: '14px', padding: '16px', color: '#E2E8F0' }}>
                <div style={{ fontSize: '11px', fontWeight: 800, color: '#94A3B8', textTransform: 'uppercase', letterSpacing: '0.5px', marginBottom: '8px' }}>
                  3. SOURCE CODE SNIPPET / STACK TRACE
                </div>
                <pre style={{ margin: 0, fontFamily: 'monospace', fontSize: '11px', lineHeight: 1.55, color: '#38BDF8', whiteSpace: 'pre-wrap', wordBreak: 'break-all', maxHeight: '180px', overflowY: 'auto' }}>
                  {selectedLogObj.stackTrace}
                </pre>
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <button className="admin-btn-sec" onClick={() => setSelectedLogId(null)}>Close Dossier</button>
              <div style={{ display: 'flex', gap: '8px' }}>
                {selectedLogObj.status === 'UNRESOLVED' && (
                  <button disabled={logActionBusy === selectedLogObj.id} className="slot-subtle-btn" style={{ background: '#EFF6FF', color: '#1D4ED8', border: '1px solid #BFDBFE', padding: '8px 14px', borderRadius: '8px', fontSize: '11.5px', fontWeight: 750, cursor: 'pointer' }} onClick={() => handleAcknowledgeLog(selectedLogObj.id)}>
                    Acknowledge / Investigate 🔍
                  </button>
                )}
                {selectedLogObj.status !== 'RESOLVED' ? (
                  <button disabled={logActionBusy === selectedLogObj.id} className="admin-btn-pri" onClick={() => handleResolveLog(selectedLogObj.id)} style={{ padding: '8px 16px', fontSize: '11.5px' }}>
                    Mark as Resolved ✓
                  </button>
                ) : (
                  <span className="badge badge-green" style={{ padding: '8px 16px', fontSize: '11.5px' }}>✓ Resolved{selectedLogObj.resolutionNote ? `: ${selectedLogObj.resolutionNote}` : ''}</span>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── MODAL 4: SIMULATE FAILURE MODAL ── */}
      {simulationsAllowed && simulateModalOpen && (
        <div className="admin-modal-backdrop" id="adminSimulateFailureModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSimulateModalOpen(false); }}>
          <div className="admin-modal-card" style={{ maxWidth: '620px' }}>
            <div style={{ background: '#0F172A', color: '#fff', padding: '16px 20px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                <span style={{ fontSize: '20px' }}>⚡</span>
                <div>
                  <strong style={{ fontSize: '15px' }}>Simulate Application Failure Incident</strong>
                  <div style={{ fontSize: '11px', color: '#94A3B8' }}>Inject an artificial error event to test admin telemetry tracking</div>
                </div>
              </div>
              <button onClick={() => setSimulateModalOpen(false)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '18px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', gap: '14px' }}>
              <div style={{ fontSize: '11px', color: '#64748B', background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '8px', padding: '8px 10px' }}>
                This writes a real row into the backend&apos;s system-error log (dev/staging only — the backend refuses this in production) so the dedicated incident pipeline can be exercised end to end.
              </div>

              <div>
                <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>Failure Scenario Preset</label>
                <select value={simScenario} onChange={(e) => setSimScenario(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#fff', outline: 'none' }}>
                  <option value="mpesa_timeout">💳 M-Pesa provider: HTTP 504 Gateway Callback Timeout</option>
                  <option value="webrtc_ice">📹 Agora WebRTC: ICE Candidate Relay Failed (Code 702)</option>
                  <option value="clinical_triage">💊 Clinical Rx Engine: Controlled-substance triage violation</option>
                  <option value="offline_sync">📶 Offline Cache: Mutation sync conflict</option>
                  <option value="kyc_hub_down">🏛️ Professional Registry: KYC hub maintenance</option>
                  <option value="sms_otp">📱 Auth Gateway: SMS OTP carrier rate limit</option>
                </select>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '140px 1fr', gap: '10px' }}>
                <div>
                  <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>Severity</label>
                  <select value={simSeverity} onChange={(e) => setSimSeverity(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#fff', outline: 'none' }}>
                    <option value="CRITICAL">🔴 CRITICAL</option>
                    <option value="ERROR">🟠 ERROR</option>
                    <option value="WARNING">🟡 WARNING</option>
                  </select>
                </div>
                <div>
                  <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>Custom Note</label>
                  <input type="text" maxLength={300} placeholder="e.g. Stress test injection" value={simCustomNote} onChange={(e) => setSimCustomNote(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', outline: 'none' }} />
                </div>
              </div>
            </div>

            <div style={{ padding: '14px 20px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button disabled={simulateBusy} className="admin-btn-sec" onClick={() => setSimulateModalOpen(false)}>Cancel</button>
              <button disabled={simulateBusy} className="admin-btn-pri" onClick={handleExecuteSimulateFailure} style={{ background: '#DC2626', borderColor: '#DC2626' }}>
                <span>⚡ Inject Failure Incident</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── MODAL 5: GRIEVANCE DETAIL MODAL ── */}
      {selectedGrvObj && (
        <div className="admin-modal-backdrop" id="adminGrievanceDetailModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSelectedGrievanceId(null); }}>
          <div className="admin-modal-card" style={{ maxWidth: '840px', width: '95%' }}>
            <div style={{ padding: '16px 20px', borderBottom: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center', background: '#FAFAFA' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                <div style={{ width: '36px', height: '36px', borderRadius: '10px', background: '#FEE2E2', color: '#DC2626', display: 'grid', placeItems: 'center', fontSize: '18px' }}>⚖️</div>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontSize: '16px', fontWeight: 800, color: '#0F172A' }}>{selectedGrvObj.id}</span>
                    <span className={`badge ${selectedGrvObj.target === 'DOCTOR' ? 'badge-rose' : 'badge-blue'}`}>{selectedGrvObj.target === 'DOCTOR' ? 'Against Doctor' : 'Against System'}</span>
                    <span className="badge badge-amber">{selectedGrvObj.status.replace(/_/g, ' ')}</span>
                  </div>
                  <div style={{ fontSize: '11px', color: '#64748B', marginTop: '2px' }}>Filed {selectedGrvObj.timeFormatted} • Ref #{selectedGrvObj.consultationId}</div>
                </div>
              </div>
              <button onClick={() => setSelectedGrievanceId(null)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '20px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ padding: '20px', maxHeight: 'calc(85vh - 130px)', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px' }}>
                <div style={{ border: '1px solid #E2E8F0', borderRadius: '10px', padding: '12px 14px', background: '#F8FAFC' }}>
                  <div style={{ fontSize: '10.5px', fontWeight: 750, color: '#475569', textTransform: 'uppercase', marginBottom: '6px' }}>👤 Complainant (Patient)</div>
                  <div style={{ fontSize: '14px', fontWeight: 800, color: '#0F172A' }}>{selectedGrvObj.patient.name} ({selectedGrvObj.patient.ageGender})</div>
                  <div style={{ fontSize: '11.5px', color: '#64748B', marginTop: '2px' }}>Phone: {selectedGrvObj.patient.phone}</div>
                </div>

                <div style={{ border: '1px solid #E2E8F0', borderRadius: '10px', padding: '12px 14px', background: '#F8FAFC' }}>
                  <div style={{ fontSize: '10.5px', fontWeight: 750, color: '#475569', textTransform: 'uppercase', marginBottom: '6px' }}>🩺 Accused Physician</div>
                  <div style={{ fontSize: '14px', fontWeight: 800, color: '#0F172A' }}>{selectedGrvObj.doctor.name}</div>
                  <div style={{ fontSize: '11.5px', color: '#64748B', marginTop: '2px' }}>{selectedGrvObj.doctor.registrationNumber} • Fee: {selectedGrvObj.doctor.fee}</div>
                </div>
              </div>

              <div style={{ border: '1.5px solid #FCA5A5', borderRadius: '12px', background: '#FFF5F5', padding: '14px 16px' }}>
                <div style={{ fontSize: '12.5px', fontWeight: 800, color: '#991B1B' }}>{selectedGrvObj.category}</div>
                <div style={{ fontSize: '12px', lineHeight: 1.5, color: '#450A0A', background: '#FFFFFF', border: '1px solid #FECACA', borderRadius: '8px', padding: '10px 12px', fontStyle: 'italic', marginTop: '6px' }}>
                  &quot;{selectedGrvObj.patientStatement}&quot;
                </div>
              </div>

              <div style={{ border: '1px solid #CBD5E1', borderRadius: '10px', padding: '14px 16px', background: '#F8FAFC' }}>
                <div style={{ fontSize: '11px', fontWeight: 800, color: '#1E293B', textTransform: 'uppercase', marginBottom: '10px' }}>📊 Forensic Telemetry &amp; System Audit</div>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '8px', marginBottom: '12px' }}>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>Call Duration</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#DC2626' }}>{selectedGrvObj.telemetryEvidence.callDuration}</div>
                  </div>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>WebRTC Audio/Video</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#059669' }}>{selectedGrvObj.telemetryEvidence.connectionStatus}</div>
                  </div>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>e-Prescription</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#DC2626' }}>{selectedGrvObj.telemetryEvidence.rxIssued}</div>
                  </div>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>Settlement</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#2563EB' }}>{selectedGrvObj.telemetryEvidence.paymentStatus}</div>
                  </div>
                </div>
              </div>

              {selectedGrvObj.adjudication && (
                <div style={{ border: '1px solid #86EFAC', borderRadius: '10px', padding: '12px 14px', background: '#F0FDF4' }}>
                  <div style={{ fontSize: '11px', fontWeight: 750, color: '#166534', textTransform: 'uppercase', marginBottom: '4px' }}>✓ Adjudication Decision Recorded</div>
                  <div style={{ fontSize: '12px', color: '#14532D' }}>{selectedGrvObj.adjudication.notes}</div>
                </div>
              )}
            </div>

            <div style={{ padding: '14px 20px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <button className="admin-btn-sec" onClick={() => setSelectedGrievanceId(null)}>Close</button>
              <div style={{ display: 'flex', gap: '8px' }}>
                <button disabled={financeBusy} className="admin-btn-pri" onClick={() => handleAdjudicateGrievance(selectedGrvObj.id, 'REFUND')} style={{ background: '#059669', borderColor: '#059669' }}>
                  <span>💳 Record Finance Refund</span>
                </button>
                <button disabled={financeBusy} className="admin-btn-pri" onClick={() => handleAdjudicateGrievance(selectedGrvObj.id, 'WARN')} style={{ background: '#DC2626', borderColor: '#DC2626' }}>
                  <span>⚠️ Issue Doctor Professional registration Warning</span>
                </button>
                <button disabled={financeBusy} className="admin-btn-sec" onClick={() => handleAdjudicateGrievance(selectedGrvObj.id, 'RESOLVE')}>
                  <span>✓ Mark Resolved</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── MODAL 6: TRANSACTION DETAIL MODAL ── */}
      {selectedTxObj && (
        <div className="admin-modal-backdrop" id="adminTransactionDetailModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSelectedTransactionId(null); }}>
          <div className="admin-modal-card" style={{ maxWidth: '760px', width: '95%' }}>
            <div style={{ background: '#0F172A', color: '#fff', padding: '18px 22px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <div style={{ width: '42px', height: '42px', borderRadius: '10px', background: 'rgba(255,255,255,0.1)', display: 'grid', placeItems: 'center', color: '#38BDF8' }}>
                  <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
                </div>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontSize: '16px', fontWeight: 800 }}>{selectedTxObj.txId}</span>
                    <span className="badge badge-green">{selectedTxObj.escrowStatusLabel}</span>
                    <span className="payout-method-badge bkash">{selectedTxObj.gatewayLabel}</span>
                  </div>
                  <div style={{ fontSize: '11.5px', color: '#94A3B8', marginTop: '2px' }}>{selectedTxObj.timestamp} • TrxID: {selectedTxObj.gatewayRef}</div>
                </div>
              </div>
              <button onClick={() => setSelectedTransactionId(null)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '20px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '1px', background: '#E2E8F0', borderBottom: '1px solid #E2E8F0' }}>
              <div style={{ background: '#FFFFFF', padding: '14px 18px' }}>
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>Gross Inflow (Fee)</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#0F172A', marginTop: '4px' }}>KES  {selectedTxObj.grossAmount}</div>
                <div style={{ fontSize: '10.5px', color: '#10B981', fontWeight: 700 }}>Debited from MFS Wallet</div>
              </div>
              <div style={{ background: '#FFFFFF', padding: '14px 18px' }}>
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>HeloDoc Platform Fee (20%)</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#059669', marginTop: '4px' }}>KES  {selectedTxObj.platformFeeAmount} ({selectedTxObj.platformFeePercent}%)</div>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Platform Commission Cut</div>
              </div>
              <div style={{ background: '#FFFFFF', padding: '14px 18px' }}>
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>Doctor Net / Refund</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#2563EB', marginTop: '4px' }}>KES  {selectedTxObj.netAmount}</div>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Payable to Physician Wallet</div>
              </div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '10.5px', color: '#64748B', padding: '8px 18px 0', background: '#FFFFFF' }}>
              <span>ℹ️</span>
              <span>Total calculation is based including the platform charge 20%.</span>
            </div>

            <div style={{ padding: '20px 22px', display: 'flex', flexDirection: 'column', gap: '16px', maxHeight: '60vh', overflowY: 'auto', background: '#FAFAFA' }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
                <div style={{ background: '#FFFFFF', border: '1px solid #E2E8F0', borderRadius: '10px', padding: '14px' }}>
                  <div style={{ fontSize: '11px', fontWeight: 800, textTransform: 'uppercase', color: '#64748B', marginBottom: '8px' }}>Payer (Patient)</div>
                  <strong style={{ fontSize: '13px', color: '#0F172A' }}>{selectedTxObj.patient.name}</strong>
                  <div style={{ fontSize: '11px', color: '#64748B' }}>{selectedTxObj.patient.phone}</div>
                </div>

                <div style={{ background: '#FFFFFF', border: '1px solid #E2E8F0', borderRadius: '10px', padding: '14px' }}>
                  <div style={{ fontSize: '11px', fontWeight: 800, textTransform: 'uppercase', color: '#64748B', marginBottom: '8px' }}>Recipient (Doctor / System)</div>
                  <strong style={{ fontSize: '13px', color: '#0F172A' }}>{selectedTxObj.doctor.name}</strong>
                  <div style={{ fontSize: '11px', color: '#64748B' }}>{selectedTxObj.doctor.registrationNumber} • {selectedTxObj.doctor.specialty}</div>
                </div>
              </div>

              <div style={{ background: '#FFFFFF', border: '1px solid #E2E8F0', borderRadius: '10px', padding: '14px' }}>
                <div style={{ fontSize: '11px', fontWeight: 800, textTransform: 'uppercase', color: '#64748B', marginBottom: '8px' }}>MFS Webhook Payload</div>
                <pre style={{ background: '#0F172A', color: '#E2E8F0', padding: '12px', borderRadius: '8px', fontFamily: 'monospace', fontSize: '11px', overflowX: 'auto', margin: 0 }}>
                  {JSON.stringify(selectedTxObj.gatewayPayload, null, 2)}
                </pre>
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#FFFFFF', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <button className="sheet-btn" onClick={() => showToast('Receipt Downloaded 📄', 'Official tax voucher saved.', '📄', 'success')} style={{ padding: '8px 16px', fontSize: '12px', background: '#F1F5F9', border: '1px solid #CBD5E1', color: '#334155', borderRadius: '8px', cursor: 'pointer' }}>
                📄 Print / Save Tax Voucher
              </button>
              <button className="sheet-btn primary" onClick={() => setSelectedTransactionId(null)} style={{ padding: '8px 20px', fontSize: '12px', background: '#0F172A', color: '#fff', borderRadius: '8px', cursor: 'pointer' }}>
                Close Audit Window
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
