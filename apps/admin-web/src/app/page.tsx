'use client';
import { tr, LanguageSwitch, useT } from '@/lib/i18n';
import { apiErrorMessage } from '@/lib/i18n/apiErrors';
import { dataRightsNotice } from '../lib/dataRightsOutcome';

import { GrievanceInvestigation } from '@/components/GrievanceInvestigation';
import { CampaignManager } from '@/components/campaigns/CampaignManager';
import { BlogManager } from '@/components/blogs/BlogManager';
import { TopicsPage } from '@/components/blogs/CategoryManager';
import { AdminTeamPage } from '@/components/team/AdminTeamPage';
import { ActivityLogPage } from '@/components/team/ActivityLogPage';
import { NotificationCenter } from '@/components/shell/NotificationCenter';
import { StepUpDialog } from '@/components/shell/StepUpDialog';
import { PatientsDirectory } from '@/components/people/PatientsDirectory';
import { PatientRecord } from '@/components/people/PatientRecord';
import { PeerDirectory } from '@/components/people/PeerDirectory';
import { DoctorVerification } from '@/components/people/DoctorVerification';
import { PeerNavigators } from '@/components/people/PeerNavigators';
import { CommandCenter } from '@/components/operations/CommandCenter';
import { SlotMatrix } from '@/components/operations/SlotMatrix';
import { IncidentsPage } from '@/components/operations/IncidentsPage';
import { AuditPage } from '@/components/operations/AuditPage';
import { GrievancesPage } from '@/components/operations/GrievancesPage';
import { PaymentsDeferred, SettlementsDeferred, SettingsDeferred } from '@/components/finance/FinanceDeferred';
import { FacilityManager } from '@/components/facilities/FacilityManager';
import { QuizManager } from '@/components/blogs/QuizManager';
import { VideoManager } from '@/components/blogs/VideoManager';
import {NavigationGuard,useAdminNavigation} from '@/components/navigation/NavigationGuard';
import { AdminSidebar, pageTrail, type AdminPageId } from '@/components/shell/AdminSidebar';
import { ChevronRight, LogOut, Search } from 'lucide-react';

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useBackendSync, ModalitySplit } from '@/lib/useBackendSync';
import { useSlotSync } from '@/lib/useSlotSync';
import { scheduleDay, scheduleLabel } from '@/lib/schedule';
import {
  clearAdminRuntime,
  initiateDisbursement,
  approveDisbursementItem,
  finalizeFinancialPeriod,
  verifyDoctorProfessionalRegistration,
  rejectDoctorProfessionalRegistration,
  acknowledgeSystemError,
  resolveSystemError,
  adjudicateGrievanceRefund,
  adjudicateGrievanceWarn,
  adjudicateGrievanceDismiss,
  transitionRefundRequest,
  getPlatformPaymentMethods,
  savePlatformPaymentMethod,
  setPrimaryPlatformPaymentMethod,
  deactivatePlatformPaymentMethod,
  PlatformPaymentMethod,
  getPatientDossier,
  getDoctorDossier,
  executePatientDataRights,
  executeDoctorDataRights,
  AdminOverview,
  FinanceSummary,
  CommandTelemetry,
  API_BASE,
  resolveConsultationRecovery,
  getCommandTelemetry,
  PendingDoctorVerification,
  AdminSlot,
  ComplianceAlert,
  FinancialRefundRequest,
  toggleSlot,
} from '@/lib/api';
import { paymentRequiredFromPolicy } from '@/lib/bookingPolicy';
import {
  DoctorHistoryItem,
  PatientRecordItem,
  AppErrorLogItem,
  GrievanceItem,
  TransactionItem,
} from '@/data/adminStore';


interface ToastNotice {
  id: string;
  title: string;
  message: string;
  icon: string;
  type: 'success' | 'urgent' | 'warning' | 'info';
}

export default function AdminPortalPage(){return <NavigationGuard><AdminPortalContent/></NavigationGuard>;}

function AdminPortalContent() {
  useT(); // re-render this screen when the language changes
  // Navigation State
  const [activeAdminPage, setActiveAdminPage] = useState<AdminPageId>('command');
  const {requestNavigation}=useAdminNavigation();
  function navigateAdmin(page:AdminPageId){if(page!==activeAdminPage)requestNavigation(()=>setActiveAdminPage(page));}

  // Interactive Data Stores
  const [doctorStore, setDoctorStore] = useState<Record<string, DoctorHistoryItem>>({});
  const [patientStore, setPatientStore] = useState<Record<string, PatientRecordItem>>({});
  const [errorLogs, setErrorLogs] = useState<AppErrorLogItem[]>([]);
  const [grievances, setGrievances] = useState<GrievanceItem[]>([]);
  const [transactions, setTransactions] = useState<TransactionItem[]>([]);
  const [overview, setOverview] = useState<AdminOverview | null>(null);
  const [financeSummary, setFinanceSummary] = useState<FinanceSummary | null>(null);
  const [commandTelemetry, setCommandTelemetry] = useState<CommandTelemetry | null>(null);
  // Server-authoritative `FINANCIAL_MODULE_MODE` (`GET /config/booking-policy`,
  // via useBackendSync's unconditional job below). Defaults to `true` (the
  // safe default) until the server has explicitly confirmed deferred mode.
  const [paymentRequired, setPaymentRequired] = useState(true);
  const [commandNeedsReviewOnly, setCommandNeedsReviewOnly] = useState(false);
  const [recoveryResolutionBusy, setRecoveryResolutionBusy] = useState<string | null>(null);

  async function handleRecoveryResolution(sessionId: string) {
    const reason = window.prompt(tr('ui.requiredResolutionReasonAtLeast8'))?.trim();
    if (!reason || reason.length < 8) return;
    const pendingFinance = window.confirm(tr('ui.chooseOkToRecordPendingFinance'));
    setRecoveryResolutionBusy(sessionId);
    try {
      await resolveConsultationRecovery(sessionId, pendingFinance ? 'PENDING_FINANCE_ACTION' : 'RESOLVED_NO_SETTLEMENT', reason);
      showToast(tr('ui.recoveryRecorded'), tr('ui.theAuditedDecisionWasSavedAppointment'), '✓', 'success');
      const refreshed = await getCommandTelemetry();
      setCommandTelemetry(refreshed);
    } catch (error) {
      showToast(tr('ui.recoveryNotRecorded'), apiErrorMessage(error, tr('ui.theSessionMayAlreadyHaveA')), '⚠️', 'warning');
    } finally {
      setRecoveryResolutionBusy(null);
    }
  }
  const [verificationQueue, setVerificationQueue] = useState<PendingDoctorVerification[]>([]);
  const [modalitySplit, setModalitySplit] = useState<ModalitySplit>({ videoRevenue: 0, chatRevenue: 0, videoCount: 0, chatCount: 0 });
  const [adminSlots, setAdminSlots] = useState<AdminSlot[]>([]);
  const slotSync = useSlotSync(setAdminSlots,activeAdminPage==='slots');
  const [slotDate, setSlotDate] = useState(() => scheduleDay(new Date().toISOString()));
  const visibleSlots = adminSlots.filter(slot => !slotDate || scheduleDay(slot.start_time) === slotDate);
  const [complianceAlerts, setComplianceAlerts] = useState<ComplianceAlert[]>([]);
  const [refundRequests, setRefundRequests] = useState<FinancialRefundRequest[]>([]);
  const [refundBusy, setRefundBusy] = useState<string | null>(null);
  const [verificationBusy, setVerificationBusy] = useState(false);
  const [platformPaymentMethods, setPlatformPaymentMethods] = useState<PlatformPaymentMethod[]>([]);
  const [paymentMethodsBusy, setPaymentMethodsBusy] = useState(false);
  const [addPaymentMethodModalOpen, setAddPaymentMethodModalOpen] = useState(false);
  const [newProviderCode, setNewProviderCode] = useState('MPESA_PAYBILL');
  const [newLabel, setNewLabel] = useState('');
  const [newMaskedDisplay, setNewMaskedDisplay] = useState('');
  const [newCredentialsJson, setNewCredentialsJson] = useState('{\n  "paybill": "",\n  "consumer_key": "",\n  "consumer_secret": ""\n}');

  const refreshPlatformPaymentMethods = useCallback(() => {
    getPlatformPaymentMethods().then(setPlatformPaymentMethods).catch(() => {});
  }, []);
  useEffect(() => {
    if (activeAdminPage === 'settings') refreshPlatformPaymentMethods();
  }, [activeAdminPage, refreshPlatformPaymentMethods]);

  async function handleSavePlatformPaymentMethod() {
    if (paymentMethodsBusy) return;
    if (!newLabel.trim() || !newMaskedDisplay.trim()) {
      showToast(tr('ui.missingFields'), tr('ui.labelAndDisplayTextAreBoth'), 'ℹ️', 'info');
      return;
    }
    let credentials: Record<string, string>;
    try {
      credentials = JSON.parse(newCredentialsJson);
    } catch {
      showToast(tr('ui.invalidCredentialsJson'), tr('ui.fixTheJsonSyntaxBeforeSaving'), '⚠️', 'warning');
      return;
    }
    setPaymentMethodsBusy(true);
    try {
      await savePlatformPaymentMethod({
        provider_code: newProviderCode,
        label: newLabel.trim(),
        masked_display: newMaskedDisplay.trim(),
        credentials,
      });
      showToast(tr('ui.paymentMethodSaved'), `${newLabel} is now configured. This is the body — plug in the real driver when ready.`, '💳', 'success');
      setAddPaymentMethodModalOpen(false);
      setNewLabel('');
      setNewMaskedDisplay('');
      refreshPlatformPaymentMethods();
    } catch (e) {
      showToast(tr('ui.saveFailed'), tr('ui.theBackendDidNotSaveThis'), '⚠️', 'warning');
      console.warn('Platform payment method save notice:', e);
    } finally {
      setPaymentMethodsBusy(false);
    }
  }

  async function handleSetPrimaryPlatformPaymentMethod(id: string) {
    if (paymentMethodsBusy) return;
    setPaymentMethodsBusy(true);
    try {
      await setPrimaryPlatformPaymentMethod(id);
      showToast(tr('ui.primaryUpdated'), tr('ui.thisMethodIsNowPrimaryFor'), '✓', 'info');
      refreshPlatformPaymentMethods();
    } catch {
      showToast(tr('ui.updateFailed'), tr('ui.theBackendDidNotChangeThe'), '⚠️', 'warning');
    } finally {
      setPaymentMethodsBusy(false);
    }
  }

  async function handleDeactivatePlatformPaymentMethod(id: string) {
    if (paymentMethodsBusy) return;
    setPaymentMethodsBusy(true);
    try {
      await deactivatePlatformPaymentMethod(id);
      showToast(tr('ui.paymentMethodRemoved'), tr('ui.itNoLongerAppearsInThe'), '✓', 'info');
      refreshPlatformPaymentMethods();
    } catch {
      showToast(tr('ui.removalFailed'), tr('ui.theBackendDidNotRemoveThis'), '⚠️', 'warning');
    } finally {
      setPaymentMethodsBusy(false);
    }
  }
  const [slotBusy, setSlotBusy] = useState<string | null>(null);
  const [logActionBusy, setLogActionBusy] = useState<string | null>(null);
  const [dataRightsBusy, setDataRightsBusy] = useState<string | null>(null);
  const [selectedVerificationId, setSelectedVerificationId] = useState<string | null>(null);
  const [financeMonth, setFinanceMonth] = useState(new Date().toISOString().slice(0, 7));
  const [financeBusy, setFinanceBusy] = useState(false);

  // Modals Active State
  const [selectedDoctorKey, setSelectedDoctorKey] = useState<string | null>(null);
  const [selectedPatientKey, setSelectedPatientKey] = useState<string | null>(null);
  const [selectedLogId, setSelectedLogId] = useState<string | null>(null);
  const [selectedGrievanceId, setSelectedGrievanceId] = useState<string | null>(null);
  const [selectedTransactionId, setSelectedTransactionId] = useState<string | null>(null);

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
    onBookingPolicyUpdate: (policy) => setPaymentRequired(paymentRequiredFromPolicy(policy)),
  }, financeMonth, activeAdminPage, selectedDoctorKey, selectedPatientKey);

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


  // Toasts
  const [toasts, setToasts] = useState<ToastNotice[]>([]);
  const toastSequence = useRef(0);

  function showToast(title: string, message: string, icon = '✓', type: 'success' | 'urgent' | 'warning' | 'info' = 'success') {
    const id = 'toast_' + ++toastSequence.current;
    setToasts((prev) => [...prev, { id, title, message, icon, type }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }

  // Page Header Titles & Breadcrumbs
  const PAGE_METADATA: Record<AdminPageId, { title: string; breadcrumb: string }> = {
    command: { title: tr('ui.operationsLiveCommandCenter'), breadcrumb: tr('ui.liveConsultationsSpecialistAvailabilityA') },
    patients: { title: tr('ui.patientsDirectory'), breadcrumb: tr('ui.peopleCareFindPatientsAndReview') },
    verification: { title: tr('ui.doctorVerification'), breadcrumb: tr('ui.peopleCareReviewKmpdcProfessionalRegistr') },
    peers: { title: tr('ui.peerNavigators'), breadcrumb: tr('ui.peopleCareApprovePeerNavigatorsFrom') },
    peerDirectory: { title: tr('ui.peerDirectory'), breadcrumb: tr('ui.peopleCarePeerNavigatorsAndThe') },
    slots: { title: tr('ui.slotMatrixLocks'), breadcrumb: tr('ui.operationsSafetyDoctorScheduleSlotsBlock') },
    logs: { title: 'App Errors & Incidents', breadcrumb: 'Operations & Safety · Track, triage and resolve platform incidents' },
    grievances: { title: tr('ui.consultationGrievances'), breadcrumb: tr('ui.operationsSafetyComplaintsAdjudicationAn') },
    compliance: { title: tr('ui.rxQAAudit'), breadcrumb: tr('ui.operationsSafetyPrescriptionAndDoctorAns') },
    blogs: { title: tr('ui.articles'), breadcrumb: tr('ui.contentHealthArticlesByTopic') },
    videos: { title: tr('ui.videos'), breadcrumb: tr('ui.contentYoutubeVideosForThePatient') },
    quizzes: { title: tr('ui.quizzes'), breadcrumb: tr('ui.contentStandaloneQuizzesForThePatient') },
    topics: { title: tr('ui.topicsCategories'), breadcrumb: tr('ui.contentLinkArticleCategoriesToPatient') },
    team: { title: tr('ui.adminsInvitations'), breadcrumb: tr('ui.adminTeamInviteAndManageAdmins') },
    activity: { title: tr('ui.activityLog'), breadcrumb: tr('ui.adminTeamWhoDidWhatAnd') },
    facilities: { title: tr('ui.facilities'), breadcrumb: tr('ui.contentHealthFacilitiesServicesAndOpenin') },
    campaigns: { title: tr('ui.campaigns'), breadcrumb: tr('ui.pushNotificationCampaignsForPatientsAnd') },
    finance: { title: tr('ui.allPaymentsEscrow'), breadcrumb: tr('ui.financePaymentsEscrowAndThe20') },
    doctors: { title: tr('ui.settlementsPayouts'), breadcrumb: tr('ui.financePhysicianEarnings20PlatformCharge') },
    settings: { title: tr('ui.paymentSettings'), breadcrumb: tr('ui.financeCollectionMethodsAndSettlementCon') },
  };

  // Real client-side export: errorLogs is already the full currently-loaded
  // incident feed (kept accurate on every page since the sync job that
  // populates it is no longer gated to the Logs/Command tab), so this needs
  // no backend round trip — just an honest download instead of the toast
  // that previously claimed a file was produced without ever creating one.
  function handleExportLedgerCsv() {
    if (transactions.length === 0) {
      showToast(tr('ui.nothingToExport'), tr('ui.thereAreNoTransactionsCurrentlyLoaded'), 'ℹ️', 'info');
      return;
    }
    const escapeCsv = (value: unknown) => {
      const s = String(value ?? '');
      return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
    };
    const columns: [string, (t: TransactionItem) => unknown][] = [
      ['Transaction ID', (t) => t.txId],
      ['Gateway Reference', (t) => t.gatewayRef],
      ['Timestamp', (t) => t.timestamp],
      ['Type', (t) => t.typeLabel],
      ['Gateway', (t) => t.gatewayLabel],
      ['Patient', (t) => t.patient?.name],
      ['Doctor', (t) => t.doctor?.name],
      ['Gross Amount', (t) => t.grossAmount],
      ['Platform Fee', (t) => t.platformFeeAmount],
      ['Net Amount', (t) => t.netAmount],
      ['Escrow Status', (t) => t.escrowStatusLabel],
    ];
    const rows = [
      columns.map(([header]) => escapeCsv(header)).join(','),
      ...transactions.map((t) => columns.map(([, get]) => escapeCsv(get(t))).join(',')),
    ];
    const blob = new Blob([rows.join('\n')], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `hellodoctor-ledger-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.csv`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    showToast(tr('ui.ledgerCsvExported'), `${transactions.length} transactions exported as CSV file.`, '📄', 'success');
  }

  function handleExportLogs() {
    if (errorLogs.length === 0) {
      showToast(tr('ui.nothingToExport'), 'There are no incidents currently loaded to export.', 'ℹ️', 'info');
      return;
    }
    const blob = new Blob([JSON.stringify(errorLogs, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `hellodoctor-incident-logs-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    showToast('Logs Exported 📥', `${errorLogs.length} incidents exported as JSON file.`, '📥', 'success');
  }

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
          tr('tpl.batchPrepared', { amount: money(batch.total_net_disbursed) }),
          tr(batch.total_doctors === 1 ? 'tpl.batchRoutedOne' : 'tpl.batchRoutedMany', { count: batch.total_doctors }),
          '🧾',
          'info'
        );
      } else {
        showToast(tr('ui.noEligibleUnassignedStatements'), tr('ui.reviewPayoutSetupAndExistingAllocations'), 'ℹ️', 'info');
      }
      await refreshBackend();
    } catch (e) {
      showToast(tr('ui.batchNotPrepared'), tr('ui.theMonthMustBeFinalizedAnd'), '⚠️', 'warning');
      console.warn('Disbursement API notice:', e);
    } finally { setFinanceBusy(false); }
  }

  async function handleFinalizePeriod() {
    if (financeBusy) return;
    setFinanceBusy(true);
    try {
      await finalizeFinancialPeriod(financeMonth);
      showToast(tr('ui.accountingMonthFinalized'), tr('tpl.monthFrozen', { month: financeMonth }), '🧾', 'info');
      await refreshBackend();
    } catch {
      showToast(tr('ui.monthNotFinalized'), tr('ui.checkWhetherTheMonthIsAlready'), '⚠️', 'warning');
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
      showToast(tr('ui.disbursementApproved'), tr('ui.financeApprovalRecordedNoProviderPayment'), '✓', 'info');
      await refreshBackend();
    } catch (e) {
      showToast(tr('ui.approvalFailed'), tr('ui.theBackendDidNotApproveThis'), '⚠️', 'warning');
      console.warn('Disbursement approval API notice:', e);
    } finally { setFinanceBusy(false); }
  }

  // 2. Professional registration Approval — operates on whichever real pending doctor is selected, not a fixed id.
  async function handleVerificationAction(action: 'approve' | 'reject') {
    if (verificationBusy) return;
    const candidate = verificationQueue.find((d) => d.id === selectedVerificationId);
    if (!candidate) {
      showToast(tr('ui.noApplicationSelected'), tr('ui.selectADoctorFromTheVetting'), 'ℹ️', 'info');
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
          showToast(tr('ui.approvalFailed'), tr('ui.theBackendDidNotVerifyThis'), '⚠️', 'warning');
          console.warn('Professional registration verify API notice:', e);
        }
      } else {
        try {
          await rejectDoctorProfessionalRegistration(candidate.id, 'Application declined by admin');
          showToast(tr('ui.applicationDeclined'), `${candidate.full_name}'s application was rejected.`, '✕', 'warning');
          await refreshBackend();
        } catch (e) {
          showToast(tr('ui.rejectionFailed'), tr('ui.theBackendDidNotRejectThis'), '⚠️', 'warning');
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
      showToast(tr('ui.slotUpdateFailed'), tr('ui.theBackendDidNotToggleThis'), '⚠️', 'warning');
      console.warn('Slot toggle API notice:', e);
    } finally {
      setSlotBusy(null);
    }
  }

  // 4. Grievance Adjudication
  const [adjudicationNotes, setAdjudicationNotes] = useState('');
  async function handleAdjudicateGrievance(grvId: string, action: 'REFUND' | 'WARN' | 'RESOLVE') {
    if (financeBusy || adjudicationNotes.trim().length < 8) return;
    setFinanceBusy(true);
    try {
      if (action === 'REFUND') {
        await adjudicateGrievanceRefund(grvId, adjudicationNotes.trim());
      } else if (action === 'WARN') {
        await adjudicateGrievanceWarn(grvId, adjudicationNotes.trim());
      } else {
        await adjudicateGrievanceDismiss(grvId, adjudicationNotes.trim());
      }
      setAdjudicationNotes('');
      await refreshBackend();
      showToast(action === 'REFUND' ? tr('ui2.refundApproved') : tr('ui2.decisionRecorded'),
        action === 'REFUND' ? tr('ui2.providerExecutionIsPendingNoMoney') : tr('ui2.theBackendConfirmedTheDecision'), '?', 'success');
    } catch (error) {
      showToast(tr('ui.decisionNotConfirmed'), apiErrorMessage(error, tr('ui.retryAfterCheckingTheServerState')), '!', 'warning');
    } finally {
      setFinanceBusy(false);
    }
  }

  // Data Protection Act 2019 (Kenya) rights: export always works (the
  // dossier already fetched to render this modal is the full record);
  // erase requires a typed reason and confirmation since the backend may
  // perform a true, irreversible delete when there is no retained history.
  function downloadJson(filenamePrefix: string, data: unknown) {
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${filenamePrefix}-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }

  async function handleExportPatientData(patientId: string) {
    if (dataRightsBusy) return;
    setDataRightsBusy(patientId);
    try {
      const dossier = await getPatientDossier(patientId);
      downloadJson(`hellodoctor-patient-${patientId.slice(0, 8)}-export`, dossier);
      showToast(tr('ui.patientDataExported'), tr('ui.fullRecordDownloadedAsJson'), '📥', 'success');
    } catch (error) {
      showToast(tr('ui.exportFailed'), apiErrorMessage(error, tr('ui.theBackendDidNotReturnThis')), '⚠️', 'warning');
    } finally {
      setDataRightsBusy(null);
    }
  }

  async function handleExportDoctorData(doctorId: string) {
    if (dataRightsBusy) return;
    setDataRightsBusy(doctorId);
    try {
      const dossier = await getDoctorDossier(doctorId);
      downloadJson(`hellodoctor-doctor-${doctorId.slice(0, 8)}-export`, dossier);
      showToast(tr('ui.doctorDataExported'), tr('ui.fullRecordDownloadedAsJson'), '📥', 'success');
    } catch (error) {
      showToast(tr('ui.exportFailed'), apiErrorMessage(error, tr('ui.theBackendDidNotReturnThis')), '⚠️', 'warning');
    } finally {
      setDataRightsBusy(null);
    }
  }

  async function handleErasePatientData(patientId: string, displayName: string) {
    if (dataRightsBusy) return;
    const reason = window.prompt(
      tr('tpl.erasePatientPrompt', { name: displayName })
    );
    if (!reason || !reason.trim()) return;
    const trimmed = reason.trim();
    if (trimmed.length < 8 || trimmed.length > 300) {
      showToast(tr('ui.invalidReason'), tr('ui.justificationMustBe8300Characters'), '⚠️', 'warning');
      return;
    }
    if (!window.confirm(tr('tpl.eraseConfirm', { name: displayName }))) return;
    setDataRightsBusy(patientId);
    try {
      const result = await executePatientDataRights(patientId, trimmed);
      let refreshed = true;
      try { await refreshBackend(); } catch { refreshed = false; }
      setSelectedPatientKey(null);
      const notice = dataRightsNotice(result, 'Patient', refreshed);
      showToast(notice.title, notice.message, notice.icon, notice.level);
    } catch (error) {
      showToast(tr('ui.requestFailed'), apiErrorMessage(error, tr('ui.theBackendRejectedThisRequest')), '⚠️', 'warning');
    } finally {
      setDataRightsBusy(null);
    }
  }

  async function handleEraseDoctorData(doctorId: string, displayName: string) {
    if (dataRightsBusy) return;
    const reason = window.prompt(
      tr('tpl.eraseDoctorPrompt', { name: displayName })
    );
    if (!reason || !reason.trim()) return;
    const trimmed = reason.trim();
    if (trimmed.length < 8 || trimmed.length > 300) {
      showToast(tr('ui.invalidReason'), tr('ui.justificationMustBe8300Characters'), '⚠️', 'warning');
      return;
    }
    if (!window.confirm(tr('tpl.eraseConfirm', { name: displayName }))) return;
    setDataRightsBusy(doctorId);
    try {
      const result = await executeDoctorDataRights(doctorId, trimmed);
      let refreshed = true;
      try { await refreshBackend(); } catch { refreshed = false; }
      setSelectedDoctorKey(null);
      const notice = dataRightsNotice(result, 'Doctor', refreshed);
      showToast(notice.title, notice.message, notice.icon, notice.level);
    } catch (error) {
      showToast(tr('ui.requestFailed'), apiErrorMessage(error, tr('ui.theBackendRejectedThisRequest')), '⚠️', 'warning');
    } finally {
      setDataRightsBusy(null);
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
      const entered = window.prompt(tr('ui.providerReferenceForThisRefundRequired'));
      if (!entered || !entered.trim()) return;
      const trimmed = entered.trim();
      if (trimmed.length > 300) {
        showToast(tr('ui.referenceTooLong'), tr('ui.providerReferenceMustBe300Characters'), '⚠️', 'warning');
        return;
      }
      providerReference = trimmed;
    }
    setRefundBusy(requestId);
    try {
      await transitionRefundRequest(requestId, status, providerReference);
      await refreshBackend();
      showToast(tr('ui.refundRequestUpdated'), tr('tpl.statusSet', { status: status.replace(/_/g, ' ') }), '✓', 'success');
    } catch (error) {
      showToast(tr('ui.updateFailed'), apiErrorMessage(error, tr('ui.theBackendRejectedThisTransition')), '⚠️', 'warning');
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
      showToast(tr('ui.updateFailed'), e instanceof Error ? e.message : 'The backend did not accept this change.', '⚠️', 'warning');
    } finally {
      setLogActionBusy(null);
    }
  }

  async function handleResolveLog(logId: string) {
    if (logActionBusy) return;
    setLogActionBusy(logId);
    try {
      await resolveSystemError(logId, 'Administrative resolution requested by the authenticated Admin account.');
      await refreshBackend();
      showToast('Incident Resolved', 'Telemetry exception marked resolved.', '✓', 'success');
    } catch (e) {
      showToast(tr('ui.updateFailed'), e instanceof Error ? e.message : 'The backend did not accept this change.', '⚠️', 'warning');
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
              background: 'var(--afya-sidebar-top)',
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
              <div style={{ fontWeight: 800, fontSize: '13px', color: '#9BEBC8' }}>{t.title}</div>
              <div style={{ fontSize: '12px', color: '#D7EAE3', marginTop: '2px' }}>{t.message}</div>
            </div>
          </div>
        ))}
      </div>

      <StepUpDialog />
      <AdminSidebar
        active={activeAdminPage}
        onNavigate={navigateAdmin}
        badges={{
          verification: verificationQueue.length > 0 ? { label: String(verificationQueue.length), tone: 'blue' } : undefined,
          logs: unresolvedLogsCount > 0 ? { label: tr('ui4.newCount', { count: unresolvedLogsCount }), tone: 'red' } : undefined,
          grievances: pendingGrievancesCount > 0 ? { label: `${pendingGrievancesCount} open`, tone: 'red' } : undefined,
          ...(!paymentRequired ? {
            finance: { label: tr('ui.deferred'), tone: 'muted', title: tr('ui.financialModuleModeDeferredReadOnly') },
            doctors: { label: tr('ui.deferred'), tone: 'muted', title: tr('ui.financialModuleModeDeferredReadOnly') },
            settings: { label: tr('ui.deferred'), tone: 'muted', title: tr('ui.financialModuleModeDeferredReadOnly') },
          } : doctorsAwaitingCount > 0 ? { doctors: { label: `${doctorsAwaitingCount} pending`, tone: 'amber' } } : {}),
        }}
        groupBadges={{
          people: verificationQueue.length > 0 ? { label: String(verificationQueue.length), tone: 'blue' } : undefined,
          operations: unresolvedLogsCount + pendingGrievancesCount > 0 ? { label: String(unresolvedLogsCount + pendingGrievancesCount), tone: 'red' } : undefined,
          finance: !paymentRequired ? { label: tr('ui.deferred'), tone: 'muted' } : undefined,
        }}
      />

      {/* Main Wrapper (Topbar + Content Area) */}
      <div className="admin-main-wrapper">
        <header className="admin-topbar">
          <nav className="admin-topbar-left admin-trail" aria-label={tr('ui.breadcrumb')}>
            {pageTrail(activeAdminPage).slice(0, -1).map(part => (
              <span key={part} className="admin-trail-part">{part}<ChevronRight size={14} aria-hidden /></span>
            ))}
            <h1 className="admin-page-title" id="adminPageHeaderTitle" title={PAGE_METADATA[activeAdminPage].breadcrumb}>
              {pageTrail(activeAdminPage).at(-1) ?? PAGE_METADATA[activeAdminPage].title}
            </h1>
          </nav>

          <div className="admin-topbar-right">
            <div className="admin-search-box">
              <Search size={16} color="var(--afya-faint)" aria-hidden />
              <input
                type="text"
                id="adminGlobalSearchInput"
                aria-label={tr('ui.searchDoctorsPatientsIncidentsAndPayment')}
                maxLength={150}
                placeholder={tr('ui.searchDoctorsPatientsIncidents')}
                onChange={(e) => {
                  const val = e.target.value;
                  setDoctorSearchQuery(val);
                  setPatientSearchQuery(val);
                  setLogSearchQuery(val);
                  setFinanceSearchQuery(val);
                }}
              />
            </div>

            <LanguageSwitch />

            <NotificationCenter pendingApplications={verificationQueue.length} onOpen={(page) => navigateAdmin(page)} />

            <div className="admin-user-pill">
              <div className="admin-avatar">AD</div>
              <div style={{ display: 'flex', flexDirection: 'column' }}>
                <span className="admin-user-name">{tr('ui.adminAccount')}</span>
                <span className="admin-user-role">{tr('ui.administrator')}</span>
              </div>
            </div>

            <button
              type="button"
              className="admin-top-icon-btn"
              title={tr('ui.signOut')}
              aria-label={tr('ui.signOut')}
              onClick={() => requestNavigation(async () => {
                try {
                  clearAdminRuntime();
                  const result = await fetch('/api/logout', { method: 'POST' });
                  if (!result.ok) window.alert(tr('ui.signedOutLocallyServerRevocationCould'));
                } catch { window.alert(tr('ui.unableToReachTheServerServer')); }
                // A full document navigation discards the authenticated runtime.
                window.location.assign(new URL('/login', window.location.origin).href);
              })}
            >
              <LogOut size={18} aria-hidden />
            </button>
          </div>
        </header>

        {/* Scrollable Canvas Area */}
        <main className="admin-content-area">

          {/* PAGE 1: SETTLEMENTS & DOCTOR WALLET PAYOUTS */}
          {activeAdminPage === 'doctors' && (
            <div className="admin-page-view active" id="adminPage_doctors">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div className="afya-page-head" style={{ flex: 1 }}>
                  <div>
                    <h2>{tr('ui.settlementsPayouts2')}</h2>
                    <p>{tr('ui.physicianEarningsThe20PlatformCharge')}</p>
                  </div>
                </div>

                {paymentRequired && (
                <div style={{ display: 'flex', alignItems: 'center' }}>
                  <button
                    className="admin-action-btn"
                    onClick={() => navigateAdmin('finance')}
                    style={{ background: '#F1F5F9', border: '1px solid #CBD5E1', color: '#1E293B', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '9px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer', marginRight: '8px' }}
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
                    <span>{tr('ui.masterPaymentLedger')}</span>
                  </button>
                  <label style={{ fontSize: '12px', marginRight: '8px' }}>{tr('ui.statementMonth')} <input aria-label={tr('ui.statementMonth')} type="month" value={financeMonth} max={new Date().toISOString().slice(0, 7)} onChange={(e) => {
                    if (!e.target.value || financeBusy) return;
                    setFinanceMonth(e.target.value);
                    setDoctorStore((previous) => Object.fromEntries(Object.entries(previous).map(([key, doc]) => [key, { ...doc, payoutRouting: undefined }])));
                  }} /></label>
                  <button className="admin-btn-sec" disabled={financeBusy} onClick={handleFinalizePeriod}>{tr('ui.finalizeMonth')}</button>
                  <button className="admin-batch-payout-btn" disabled={financeBusy} onClick={handleExecuteBatchPayout}>
                    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                    <span>{tr('ui.prepareBatch')}</span>
                  </button>
                </div>
                )}
              </div>

              {!paymentRequired && <SettlementsDeferred consultations={overview?.modality_split ? overview.modality_split.videoCount + overview.modality_split.chatCount : null} />}
              {paymentRequired && <>
              {/* 4 Metric Cards — all computed from live backend data */}
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.grossTelehealthRevenue')}</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(overview?.gross_volume)}</div>
                  <span className="admin-kpi-delta neutral">{tr(transactions.length === 1 ? 'mix.acrossTxOne' : 'mix.acrossTxMany', { count: transactions.length })}</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.n24hChatRevenue')}</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(modalitySplit.chatRevenue)}</div>
                  <div className="kpi-micro-chart">
                    <div className="kpi-donut-legend">
                      <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#2563EB' }}></span> {tr(modalitySplit.videoCount === 1 ? 'mix.videoConsultsOne' : 'mix.videoConsultsMany', { count: modalitySplit.videoCount })}</div>
                      <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#E2E8F0' }}></span> {tr(modalitySplit.chatCount === 1 ? 'mix.chatSessionsOne' : 'mix.chatSessionsMany', { count: modalitySplit.chatCount })}</div>
                    </div>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.platformFee20')}</span>
                    <div className="admin-kpi-icon-circle teal" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(overview?.platform_fees)}</div>
                  <span className="admin-kpi-delta neutral">{tr('ui.automatedCommissionCut')}</span>
                  {(modalitySplit.videoRevenue + modalitySplit.chatRevenue) > 0 && (
                    <div className="kpi-micro-chart kpi-stacked-bar">
                      <div className="kpi-bar-row">
                        <span className="kpi-bar-label">{tr('ui.video')}</span>
                        <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: `${Math.round((modalitySplit.videoRevenue / (modalitySplit.videoRevenue + modalitySplit.chatRevenue)) * 100)}%`, background: 'linear-gradient(90deg,#0D9488,#5EEAD4)' }}></div></div>
                        <span className="kpi-bar-value">{money(modalitySplit.videoRevenue)}</span>
                      </div>
                      <div className="kpi-bar-row">
                        <span className="kpi-bar-label">{tr('ui.chat')}</span>
                        <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: `${Math.round((modalitySplit.chatRevenue / (modalitySplit.videoRevenue + modalitySplit.chatRevenue)) * 100)}%`, background: 'linear-gradient(90deg,#14B8A6,#99F6E4)' }}></div></div>
                        <span className="kpi-bar-value">{money(modalitySplit.chatRevenue)}</span>
                      </div>
                    </div>
                  )}
                </div>

                <div className="admin-kpi-card" style={{ borderColor: '#FCD34D' }}>
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.unpaidDoctorEarnings')}</span>
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
                          ? tr('tpl.noPendingEarning')
                          : tr(doctorsAwaitingCount === 1 ? 'tpl.doctorsAwaitingOne' : 'tpl.doctorsAwaitingMany', { count: doctorsAwaitingCount })}
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '11px', color: '#64748B', margin: '-8px 0 16px' }}>
                <span>ℹ️</span>
                <span>{tr('ui.totalCalculationIsBasedIncludingThe')}</span>
              </div>
              </>}

              {/* Pending Doctor Payout Disbursements Table */}
              <div className="admin-card">
                <div className="admin-card-header" style={{ flexWrap: 'wrap', gap: '12px', alignItems: 'center' }}>
                  <div>
                    <div className="admin-card-title">{tr('ui.physicianPayouts')}</div>
                    <div className="admin-card-subtitle">{tr('ui.grossEarnings20PlatformChargeFinal')}</div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                    <div style={{ position: 'relative', minWidth: '240px', maxWidth: '320px' }}>
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', pointerEvents: 'none' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
                        maxLength={150}
                        placeholder={tr('ui.searchDoctorProfessionalRegistrationSpec')}
                        value={doctorSearchQuery}
                        onChange={(e) => setDoctorSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 12px 7px 30px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', outline: 'none' }}
                      />
                    </div>

                    <div style={{ display: 'flex', gap: '6px' }}>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'all' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('all')}>{tr('mix.allCount', { count: Object.keys(doctorStore).length })}</button>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'pending' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('pending')}>{tr('mix.pendingCount', { count: doctorsAwaitingCount })}</button>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'settled' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('settled')}>{tr('mix.settledCount', { count: doctorsSettledCount })}</button>
                    </div>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>{tr('ui.doctor')}</th>
                        <th>{tr('ui.kmpdcReg')}</th>
                        <th>{tr('ui.videoVisits')}</th>
                        <th>{tr('ui.chatSessions')}</th>
                        <th>{tr('ui.monthlyGross')}</th>
                        <th>{tr('ui.platformCharge20')}</th>
                        <th>{tr('ui.finalNetTakeHome')}</th>
                        <th>{tr('ui.payoutMethod')}</th>
                        <th>{tr('ui.status')}</th>
                        <th>{tr('ui.actions')}</th>
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
                                  <div style={{ fontSize: '11px', color: 'var(--afya-muted)', marginTop: '1px' }}>{doc.phone}</div>
                                </div>
                              </div>
                            </td>
                            <td><span className="badge badge-slate" style={{ fontWeight: 750 }}>{doc.registrationNumber}</span><div>{doc.professionalVerificationStatus}</div></td>
                            <td><b>{doc.kpiVisits}</b></td>
                            <td>{doc.kpiChats}</td>
                            <td>{!paymentRequired ? tr('ui2.deferred') : doc.payoutRouting ? `${doc.payoutRouting.currency} ${Number(doc.payoutRouting.gross || 0).toLocaleString()}` : tr('ui2.unavailable')}</td>
                            <td style={{ color: '#64748B' }}>{!paymentRequired ? tr('ui2.deferred') : doc.payoutRouting ? `${doc.payoutRouting.currency} ${Number(doc.payoutRouting.platformCharge || 0).toLocaleString()}` : tr('ui2.unavailable')}</td>
                            <td style={{ fontWeight: 850, color: '#059669' }}>{!paymentRequired ? tr('ui2.deferred') : doc.payoutRouting ? `${doc.payoutRouting.currency} ${Number(doc.payoutRouting.finalPayable).toLocaleString()}` : tr('ui2.unavailable')}</td>
                            <td>
                              {!paymentRequired ? (
                                <span className="payout-method-badge bank">{tr('ui.financialModuleDeferred')}</span>
                              ) : (
                              <span className="payout-method-badge bank">
                                {doc.payoutRouting?.provider
                                  ? `${doc.payoutRouting.provider} ${doc.payoutRouting.maskedDestination || ''}`
                                  : tr('ui2.payoutSetupRequired')}
                              </span>
                              )}
                            </td>
                            <td>
                              {!paymentRequired ? (
                                <span className="badge badge-slate doc-payout-status">{tr('ui.deferred')}</span>
                              ) : (
                              <>
                              {/* The backend's `readiness` field is never literally the
                                  string 'ACTIVE' — it's 'DEV_SIMULATED', 'ASSIGNED', the
                                  destination's real verification_status, or
                                  'PAYOUT_SETUP_REQUIRED' when nothing is on file at all
                                  (see admin_handlers::list_payout_routing's CASE
                                  expression). Checking for the literal 'ACTIVE' string
                                  meant a doctor who had genuinely saved a working payout
                                  method could never show as ready. Green now means
                                  "something usable is on file", not "matches one
                                  specific string". */}
                              <span className={`badge ${doc.payoutRouting?.readiness && doc.payoutRouting.readiness !== 'PAYOUT_SETUP_REQUIRED' ? 'badge-green' : 'badge-amber'} doc-payout-status`}>
                                {doc.payoutRouting?.disbursementStatus || doc.payoutRouting?.readiness || tr('ui2.payoutSetupRequired')}
                              </span>
                              <div style={{ fontSize: '10px' }}>{doc.payoutRouting?.statementStatus || tr('ui2.notLoaded')} · {doc.payoutRouting?.readiness || tr('ui2.unknown')}</div>
                              </>
                              )}
                            </td>
                            <td>
                              <button
                                className="admin-btn-sec"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setSelectedDoctorKey(doc.key);
                                }}
                              >
                                {tr('ui.viewHistory')}
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

          {/* PAGE 2: OPERATIONS & LIVE COMMAND CENTER — Operations board, panel 1 */}
          {activeAdminPage === 'command' && (
            <div className="admin-page-view active" id="adminPage_command">
              <CommandCenter telemetry={commandTelemetry} overview={overview} needsReviewOnly={commandNeedsReviewOnly} setNeedsReviewOnly={setCommandNeedsReviewOnly} busyId={recoveryResolutionBusy} onRecordDecision={(id) => void handleRecoveryResolution(id)} apiBase={API_BASE} />
            </div>
          )}

          {/* PAGE 3: SLOT MATRIX & LOCKS — Operations board, panel 2 */}
          {activeAdminPage === 'slots' && (
            <div className="admin-page-view active" id="adminPage_slots">
              <SlotMatrix slots={visibleSlots} date={slotDate} setDate={setSlotDate} loading={slotSync.loading} error={slotSync.error ?? null} updatedAt={slotSync.updatedAt ?? null} busyId={slotBusy} onRefresh={() => void slotSync.refresh()} onToggle={(id, action) => void handleToggleSlot(id, action)} />
            </div>
          )}

          {/* PAGE 4: PATIENTS DIRECTORY — People & Care board, panels 1 and 2 */}
          {activeAdminPage === 'patients' && (
            <div className="admin-page-view active" id="adminPage_patients">
              {selectedPatientKey && selectedPatObj ? (
                <PatientRecord
                  patient={selectedPatObj}
                  onBack={() => setSelectedPatientKey(null)}
                  onExport={() => handleExportPatientData(selectedPatientKey)}
                  onErase={() => handleErasePatientData(selectedPatientKey, selectedPatObj.name || 'this patient')}
                  busy={dataRightsBusy === selectedPatientKey}
                />
              ) : (
                <PatientsDirectory patients={Object.values(patientStore)} onOpen={setSelectedPatientKey} />
              )}
            </div>
          )}

          {/* PAGE 5: DOCTOR VERIFICATION — People & Care board, panels 3 to 5 */}
          {activeAdminPage === 'verification' && (
            <div className="admin-page-view active" id="adminPage_verification">
              <DoctorVerification queue={verificationQueue} busy={verificationBusy} onSelect={setSelectedVerificationId} onDecide={handleVerificationAction} />
            </div>
          )}

          {/* PAGE 6: RX & Q&A AUDIT — Operations board, panel 5 */}
          {activeAdminPage === 'compliance' && (
            <div className="admin-page-view active" id="adminPage_compliance">
              <AuditPage alerts={complianceAlerts} />
            </div>
          )}

          {/* PAGE 7: APP ERRORS & INCIDENTS — Operations board, panel 3 */}
          {activeAdminPage === 'logs' && (
            <div className="admin-page-view active" id="adminPage_logs">
              <IncidentsPage
                logs={errorLogs}
                filtered={filteredLogs}
                search={logSearchQuery} setSearch={setLogSearchQuery}
                severity={logSeverityFilter} setSeverity={setLogSeverityFilter}
                subsystem={logSubsystemFilter} setSubsystem={setLogSubsystemFilter}
                status={logStatusFilter} setStatus={(v) => { setLogQuickFilter('all'); setLogStatusFilter(v); }}
                busyId={logActionBusy}
                onInspect={setSelectedLogId}
                onResolve={(id) => void handleResolveLog(id)}
                onExport={handleExportLogs}
                onRefresh={() => { refreshBackend(); showToast('Incidents refreshed', 'Loaded the latest errors from the backend.', '🔄', 'info'); }}
              />
            </div>
          )}

          {/* PAGE 8: CONSULTATION GRIEVANCES — Operations board, panel 4 (with appeals) */}
          {activeAdminPage === 'grievances' && (
            <div className="admin-page-view active" id="adminPage_grievances">
              <GrievancesPage grievances={grievances} onOpen={setSelectedGrievanceId} onChanged={() => refreshBackend()} />
            </div>
          )}

          {/* PAGE: ADMIN CAMPAIGN MANAGER */}
          {activeAdminPage === 'blogs' && <div className="admin-page-view active" id="adminPage_blogs"><BlogManager /></div>}
          {activeAdminPage === 'facilities' && <div className="admin-page-view active" id="adminPage_facilities"><FacilityManager /></div>}
          {activeAdminPage === 'quizzes' && <div className="admin-page-view active" id="adminPage_quizzes"><QuizManager /></div>}
          {activeAdminPage === 'videos' && <div className="admin-page-view active" id="adminPage_videos"><VideoManager /></div>}
          {activeAdminPage === 'topics' && <div className="admin-page-view active" id="adminPage_topics"><TopicsPage /></div>}
          {activeAdminPage === 'team' && <div className="admin-page-view active" id="adminPage_team"><AdminTeamPage /></div>}
          {activeAdminPage === 'peers' && <div className="admin-page-view active" id="adminPage_peers"><PeerNavigators /></div>}
          {activeAdminPage === 'peerDirectory' && <div className="admin-page-view active" id="adminPage_peerDirectory"><PeerDirectory onOpenPatient={(id) => requestNavigation(() => { setSelectedPatientKey(id); setActiveAdminPage('patients'); })} /></div>}
          {activeAdminPage === 'activity' && <div className="admin-page-view active" id="adminPage_activity"><ActivityLogPage /></div>}
          {activeAdminPage === 'campaigns' && (
            <div className="admin-page-view active" id="adminPage_campaigns">
              <CampaignManager />
            </div>
          )}

          {/* PLATFORM PAYMENT SETTINGS — the platform's own merchant
              configuration, distinct from a patient's saved payment methods
              and a doctor's payout destinations: this is the platform's own
              identity with its payment provider(s). More than one may be
              active at once. */}
          {activeAdminPage === 'settings' && (
            <div className="admin-page-view active" id="adminPage_settings" style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '12px' }}>
                <div className="afya-page-head" style={{ flex: 1 }}>
                  <div>
                    <h2>{tr('ui.paymentSettings')}</h2>
                    <p>{tr('ui.howThePlatformCollectsPaymentM')}</p>
                  </div>
                </div>
                {paymentRequired && (
                <button className="admin-btn-pri" onClick={() => setAddPaymentMethodModalOpen(true)} style={{ padding: '7px 14px', fontSize: '11.5px' }}>
                  {tr('ui.addPaymentMethod')}
                </button>
                )}
              </div>

              {!paymentRequired && <SettingsDeferred />}
              {paymentRequired && <div className="admin-table-card">
                <table className="admin-table">
                  <thead>
                    <tr>
                      <th>{tr('ui.provider')}</th>
                      <th>{tr('ui.label')}</th>
                      <th>{tr('ui.display')}</th>
                      <th>{tr('ui.status')}</th>
                      <th>{tr('ui.actions')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {platformPaymentMethods.length === 0 ? (
                      <tr><td colSpan={5} style={{ textAlign: 'center', color: '#94A3B8', padding: '24px' }}>{tr('ui.noPlatformPaymentMethodsConfiguredYet')}</td></tr>
                    ) : (
                      platformPaymentMethods.map((m) => (
                        <tr key={m.id}>
                          <td style={{ fontFamily: 'monospace', fontWeight: 700 }}>{m.provider_code}</td>
                          <td>{m.label}</td>
                          <td style={{ fontFamily: 'monospace' }}>{m.masked_display}</td>
                          <td>
                            <span className={`badge ${m.is_primary ? 'badge-green' : 'badge-amber'}`}>
                              {m.is_primary ? tr('ui2.primary') : tr('ui2.active')}
                            </span>
                          </td>
                          <td style={{ display: 'flex', gap: '8px' }}>
                            {!m.is_primary && (
                              <button className="admin-btn-sec" disabled={paymentMethodsBusy} onClick={() => handleSetPrimaryPlatformPaymentMethod(m.id)} style={{ fontSize: '11px', padding: '5px 10px' }}>
                                {tr('ui.makePrimary')}
                              </button>
                            )}
                            <button className="admin-btn-sec" disabled={paymentMethodsBusy} onClick={() => handleDeactivatePlatformPaymentMethod(m.id)} style={{ fontSize: '11px', padding: '5px 10px', color: '#DC2626' }}>
                              {tr('ui.remove')}
                            </button>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>}
            </div>
          )}

          {/* PAGE 9: OMNICHANNEL PAYMENTS & ESCROW MASTER LEDGER */}
          {activeAdminPage === 'finance' && (
            <div className="admin-page-view active" id="adminPage_finance" style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '12px' }}>
                <div className="afya-page-head" style={{ flex: 1 }}>
                  <div>
                    <h2>{tr('ui.allPaymentsEscrow2')}</h2>
                    <p>{tr('ui.patientPaymentsEscrowThe20Platform')}</p>
                  </div>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <button type="button" className="afya-btn-secondary" onClick={() => navigateAdmin('doctors')}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v12M15 9.5a3.5 3.5 0 0 0-7 0c0 2 1.5 3 3.5 3s3.5 1 3.5 3a3.5 3.5 0 0 1-7 0"/></svg>
                    <span>{tr('ui.settlementsPayouts3')}</span>
                  </button>
                  {paymentRequired && (<>
                  <button className="admin-action-btn" onClick={() => showToast(tr('ui.notYetAvailable'), tr('ui.automatedMfsWebhookReconciliationIsNot'), 'ℹ️', 'info')} style={{ background: '#F1F5F9', border: '1px solid #CBD5E1', color: '#64748B', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '8px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                    <span>{tr('ui.reconcileMfsWebhooks')}</span>
                  </button>
                  <button className="admin-action-btn" onClick={handleExportLedgerCsv} style={{ background: 'var(--afya-green)', border: '1px solid var(--afya-green)', color: '#FFFFFF', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '10px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                    <span>{tr('ui.exportLedgerCsv')}</span>
                  </button>
                  </>)}
                </div>
              </div>

              {!paymentRequired && <PaymentsDeferred />}
              {paymentRequired && <>
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.totalPlatformGmvInflow')}</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(financeSummary?.total_gross)}</div>
                  <span className="admin-kpi-delta neutral">{transactions.length} transaction{transactions.length === 1 ? '' : 's'}</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.activeEscrowLocked')}</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#EFF6FF', color: '#2563EB' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#2563EB' }}>{money(transactions.filter((t) => t.escrowStatus === 'ESCROW_LOCKED').reduce((s, t) => s + t.grossAmount, 0))}</div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>{tr('ui.heldSecurelyPendingDoctorSignOff')}</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.helodoc20Commission')}</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#ECFDF5', color: '#059669' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#059669' }}>{money(financeSummary?.total_platform_fee)}</div>
                  <span className="admin-kpi-delta up">{tr('ui.netPlatformOperatingRevenue')}</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">{tr('ui.totalNetPayableToDoctors')}</span>
                    <div className="admin-kpi-icon-circle purple" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#F5F3FF', color: '#7C3AED' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="13 17 18 12 13 7"/><polyline points="6 17 11 12 6 7"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">{money(financeSummary?.total_net)}</div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>{tr('ui.settledPendingAcrossAllDoctors')}</span>
                </div>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '11px', color: '#64748B', margin: '-8px 0 16px' }}>
                <span>ℹ️</span>
                <span>{tr('ui.totalCalculationIsBasedIncludingThe')}</span>
              </div>

              <div className="admin-card">
                <div className="admin-card-header" style={{ flexWrap: 'wrap', gap: '12px' }}>
                  <div>
                    <div className="admin-card-title">{tr('ui.omnichannelFinancialAuditTable')}</div>
                    <div className="admin-card-subtitle">{tr('ui.completeLedgerOfMfsGatewayCheckouts')}</div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                    <div style={{ position: 'relative', width: '260px' }}>
                      <input
                        type="text"
                        maxLength={150}
                        placeholder={tr('ui.searchTxidPatientDoctorRef')}
                        value={financeSearchQuery}
                        onChange={(e) => setFinanceSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 10px 7px 32px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', outline: 'none', background: '#FFFFFF' }}
                      />
                    </div>

                    <select value={financeGatewayFilter} onChange={(e) => setFinanceGatewayFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#FFFFFF', color: '#334155', outline: 'none' }}>
                      <option value="ALL">{tr('ui.allGateways')}</option>
                      <option value="MPESA">M-Pesa</option>
                      <option value="BANK_TRANSFER">{tr('ui.kenyanBankTransfer')}</option>
                      <option value="CARD">{tr('ui.cardIfPayoutCapable')}</option>
                      <option value="ALL">{tr('ui.allConfiguredProviders')}</option>
                    </select>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '4px', background: '#F1F5F9', padding: '3px', borderRadius: '8px' }}>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'ALL' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('ALL')}>{tr('ui.all')}</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'INFLOW' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('INFLOW')}>{tr('ui.inflow')}</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'ESCROW' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('ESCROW')}>{tr('ui.escrow')}</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'PAYOUT' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('PAYOUT')}>{tr('ui.payout')}</button>
                      <button className={`admin-filter-pill ${financeTypeFilter === 'REFUND' ? 'active' : ''}`} onClick={() => setFinanceTypeFilter('REFUND')}>{tr('ui.refund')}</button>
                    </div>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>{tr('ui.txidMfsRef')}</th>
                        <th>{tr('ui.timestamp')}</th>
                        <th>{tr('ui.channelType')}</th>
                        <th>{tr('ui.consultationCase')}</th>
                        <th>{tr('ui.partiesPatientDoctor')}</th>
                        <th style={{ textAlign: 'right' }}>{tr('ui.grossKes')}</th>
                        <th style={{ textAlign: 'right' }}>{tr('ui.helodocCut')}</th>
                        <th style={{ textAlign: 'right' }}>{tr('ui.netKes')}</th>
                        <th>{tr('ui.gatewayEscrowState')}</th>
                        <th style={{ textAlign: 'center' }}>{tr('ui.action')}</th>
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
                              {tr('ui.receipt')}
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
                    <div className="admin-card-title">{tr('ui.refundRequestResolution')}</div>
                    <div className="admin-card-subtitle">{tr('ui.boardApprovedRefundsAwaitingFinanceExecu')}</div>
                  </div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>{tr('mix.pendingOf', { pending: refundRequests.filter((r) => r.status !== 'REFUNDED').length, total: refundRequests.length })}</span>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>{tr('ui.approved')}</th>
                        <th>{tr('ui.grievanceTransaction')}</th>
                        <th style={{ textAlign: 'right' }}>{tr('ui.amount')}</th>
                        <th>{tr('ui.previousPaymentStatus')}</th>
                        <th>{tr('ui.providerReference')}</th>
                        <th>{tr('ui.status')}</th>
                        <th style={{ textAlign: 'right' }}>{tr('ui.actions')}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {refundRequests.length === 0 && (
                        <tr><td colSpan={7} style={{ textAlign: 'center', padding: '20px', color: '#94A3B8', fontSize: '12px' }}>{tr('ui.noRefundRequestsTheseAppearOnce')}</td></tr>
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
                                <span style={{ fontSize: '11px', color: '#94A3B8' }}>{tr('ui.finalized')}</span>
                              ) : (
                                <div style={{ display: 'flex', gap: '4px', justifyContent: 'flex-end', flexWrap: 'wrap' }}>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'PROCESSING')} style={{ padding: '4px 8px', border: '1px solid #BFDBFE', background: '#EFF6FF', color: '#1D4ED8', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>{tr('ui.processing')}</button>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'REFUNDED')} style={{ padding: '4px 8px', border: '1px solid #86EFAC', background: '#F0FDF4', color: '#166534', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>{tr('ui.refunded')}</button>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'FAILED')} style={{ padding: '4px 8px', border: '1px solid #FECACA', background: '#FEF2F2', color: '#DC2626', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>{tr('ui.failed')}</button>
                                  <button disabled={busy} onClick={() => handleTransitionRefund(r.id, 'RECONCILIATION_REQUIRED')} style={{ padding: '4px 8px', border: '1px solid #FDE68A', background: '#FFFBEB', color: '#B45309', borderRadius: '6px', fontSize: '10.5px', fontWeight: 700, cursor: 'pointer' }}>{tr('ui.needsReconciliation')}</button>
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
              </>}
            </div>
          )}

        </main>
      </div>

      {/* ── MODAL 1: DOCTOR HISTORY & FINANCIAL DOSSIER ── */}
      {selectedDocObj && (
        <div className="admin-modal-backdrop" id="adminDoctorHistoryModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSelectedDoctorKey(null); }}>
          <div className="admin-modal-card">
            <div style={{ background: 'linear-gradient(90deg, var(--afya-sidebar-top), #0b6b55)', color: '#fff', padding: '18px 22px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <div style={{ width: '48px', height: '48px', borderRadius: '50%', background: selectedDocObj.bgColor, color: '#fff', display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '16px', flexShrink: 0 }}>
                  {selectedDocObj.avatar}
                </div>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                    <span style={{ fontSize: '16.5px', fontWeight: 800, color: '#FFFFFF' }}>{selectedDocObj.name}</span>
                    <span className="badge badge-green">{selectedDocObj.registrationNumber}</span>
                    <span className="badge badge-blue">{tr('ui.verifiedSpecialist')}</span>
                    <span className="badge" style={{ background: 'rgba(255,255,255,0.12)', color: '#E2E8F0', fontSize: '10px' }}>{tr('ui.activePractitioner')}</span>
                  </div>
                  <div style={{ fontSize: '11.5px', color: '#94A3B8', marginTop: '2px' }}>{selectedDocObj.meta}</div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '14px', marginTop: '6px', fontSize: '11px', flexWrap: 'wrap' }}>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px', color: '#94A3B8' }}>
                      <span>{tr('ui.phone')}</span> <b style={{ color: '#FFFFFF', fontFamily: 'monospace' }}>{selectedDocObj.phone}</b>
                    </span>
                    <span style={{ color: '#475569' }}>•</span>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px', color: '#94A3B8' }}>
                      <span>{tr('ui.residence')}</span> <b style={{ color: '#FFFFFF' }}>{selectedDocObj.residence}</b>
                    </span>
                    <span style={{ color: '#475569' }}>•</span>
                    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px', color: '#94A3B8' }}>
                      <span>{tr('ui.email')}</span> <b style={{ color: '#E2E8F0' }}>{selectedDocObj.email}</b>
                    </span>
                  </div>
                </div>
              </div>
              <button onClick={() => setSelectedDoctorKey(null)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '20px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '1px', background: '#E2E8F0', borderBottom: '1px solid #E2E8F0' }}>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>{tr('ui.lifetimeConsultations')}</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#0F172A', marginTop: '2px' }}>{selectedDocObj.kpiVisits}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>{tr('ui.n24hChatSessions')}</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#2563EB', marginTop: '2px' }}>{selectedDocObj.kpiChats}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>{tr('ui.totalGrossRevenue')}</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#059669', marginTop: '2px' }}>{paymentRequired ? selectedDocObj.kpiGross : tr('ui2.deferred')}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>{tr('ui.netPayoutsDisbursed')}</div>
                <div style={{ fontSize: '18px', fontWeight: 800, color: '#047857', marginTop: '2px' }}>{paymentRequired ? selectedDocObj.kpiDisbursed : tr('ui2.deferred')}</div>
              </div>
            </div>
            {paymentRequired && <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '10.5px', color: '#64748B', padding: '8px 16px 0' }}>
              <span>ℹ️</span>
              <span>{tr('ui.totalCalculationIsBasedIncludingThe')}</span>
            </div>}

            <div style={{ padding: '18px 22px', overflowY: 'auto', flex: 1 }}>
              {paymentRequired && selectedDocObj.payoutRouting?.disbursementItemId && selectedDocObj.payoutRouting.disbursementStatus === 'PENDING_APPROVAL' && (
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '12px', padding: '12px 14px', marginBottom: '16px', border: '1px solid #FCD34D', borderRadius: '10px', background: '#FFFBEB' }}>
                  <div>
                    <strong style={{ fontSize: '12px', color: '#92400E' }}>{tr('ui.kesDisbursementAwaitingFinanceApproval')}</strong>
                    <div style={{ fontSize: '10.5px', color: '#78350F', marginTop: '2px' }}>{tr('ui.approvalRecordsAuthorizationOnlyPaymentR')}</div>
                  </div>
                  <button className="admin-btn-pri" disabled={financeBusy} onClick={() => handleApproveDoctorDisbursement(selectedDocObj.payoutRouting!.disbursementItemId!)}>{tr('ui.approve')}</button>
                </div>
              )}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                <strong style={{ fontSize: '13.5px', color: '#0F172A' }}>{tr('ui.recentConsultationEncountersIssuedPrescr')}</strong>
                <span style={{ fontSize: '11px', color: '#64748B' }}>{tr('ui.archivedAcrossAllPatientEncounters')}</span>
              </div>

              <table className="admin-table">
                <thead>
                  <tr>
                    <th>{tr('ui.dateTime')}</th>
                    <th>{tr('ui.patientName')}</th>
                    <th>{tr('ui.consultationMode')}</th>
                    <th>{tr('ui.feeKes')}</th>
                    <th>{tr('ui.clinicalDiagnosis')}</th>
                    <th>{tr('ui.rxIssued')}</th>
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
                <strong style={{ fontSize: '13.5px', color: '#0F172A' }}>{tr('ui.recentFinancialWalletDisbursements20Plat')}</strong>
                {!paymentRequired && <div style={{ fontSize: '11.5px', color: '#64748B', marginTop: '8px' }}>{tr('ui.financialModuleNotYetActiveNo')}</div>}
                {paymentRequired && <div style={{ marginTop: '8px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  {selectedDocObj.disbursements.map((dsb, idx) => (
                    <div key={idx} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 14px', background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '10px', fontSize: '11.5px' }}>
                      <div>
                        <b>{dsb.date}: {dsb.amount}</b> {tr('ui.assignedDestination')} <span className="badge" style={{ background: '#FCE7F3', color: '#DB2777', fontWeight: 800 }}>{dsb.method}</span>
                        <div style={{ fontSize: '10px', color: '#64748B' }}>{tr('mix.txCommission', { tx: dsb.txId, commission: dsb.commission })}</div>
                      </div>
                      <span className="badge badge-green">{dsb.status}</span>
                    </div>
                  ))}
                </div>}
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: '11px', color: '#64748B' }}>{tr('ui.professionalRegistrationVerifiedCredenti')}</span>
              <div style={{ display: 'flex', gap: '8px' }}>
                <button
                  className="admin-btn-sec"
                  disabled={dataRightsBusy === selectedDoctorKey}
                  onClick={() => selectedDoctorKey && handleExportDoctorData(selectedDoctorKey)}
                >
                  {dataRightsBusy === selectedDoctorKey ? tr('ui2.working') : tr('ui2.exportDataJson')}
                </button>
                <button
                  className="admin-btn-sec"
                  style={{ color: '#DC2626', borderColor: '#FCA5A5' }}
                  disabled={dataRightsBusy === selectedDoctorKey}
                  onClick={() => selectedDoctorKey && handleEraseDoctorData(selectedDoctorKey, selectedDocObj?.name || 'this doctor')}
                >
                  {tr('ui.eraseAnonymizeData')}
                </button>
                <button className="admin-btn-sec" onClick={() => setSelectedDoctorKey(null)}>{tr('ui.closeDossier')}</button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── MODAL 3: LOG DETAIL MODAL ── */}
      {selectedLogObj && (
        <div className="admin-modal-backdrop" id="adminLogDetailModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setSelectedLogId(null); }}>
          <div className="admin-modal-card" style={{ maxWidth: '920px' }}>
            <div style={{ background: 'linear-gradient(90deg, var(--afya-sidebar-top), #0b6b55)', color: '#fff', padding: '18px 22px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
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
              <button className="admin-btn-sec" onClick={() => setSelectedLogId(null)}>{tr('ui.closeDossier')}</button>
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

      {/* ── MODAL: ADD PLATFORM PAYMENT METHOD ── */}
      {addPaymentMethodModalOpen && (
        <div className="admin-modal-backdrop" id="adminAddPaymentMethodModal" style={{ display: 'flex' }} onClick={(e) => { if (e.target === e.currentTarget) setAddPaymentMethodModalOpen(false); }}>
          <div className="admin-modal-card" style={{ maxWidth: '620px' }}>
            <div style={{ background: 'linear-gradient(90deg, var(--afya-sidebar-top), #0b6b55)', color: '#fff', padding: '16px 20px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                <span style={{ fontSize: '20px' }}>💳</span>
                <div>
                  <strong style={{ fontSize: '15px' }}>{tr('ui.addPlatformPaymentMethod')}</strong>
                  <div style={{ fontSize: '11px', color: '#94A3B8' }}>{tr('ui.thePlatformSOwnMerchantConfiguration')}</div>
                </div>
              </div>
              <button onClick={() => setAddPaymentMethodModalOpen(false)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '18px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', gap: '14px' }}>
              <div style={{ fontSize: '11px', color: '#64748B', background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '8px', padding: '8px 10px' }}>
                {tr('ui.credentialsAreEncryptedAtRestAnd')}
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
                <div>
                  <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>{tr('ui.providerCode')}</label>
                  <select value={newProviderCode} onChange={(e) => setNewProviderCode(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#fff', outline: 'none' }}>
                    <option value="MPESA_PAYBILL">M-Pesa Paybill</option>
                    <option value="MPESA_TILL">M-Pesa Till</option>
                    <option value="CARD_AGGREGATOR">{tr('ui.cardAggregator')}</option>
                    <option value="OTHER">{tr('ui.other')}</option>
                  </select>
                </div>
                <div>
                  <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>{tr('ui.label')}</label>
                  <input type="text" maxLength={80} placeholder={tr('ui.eGPrimaryMPesaPaybill')} value={newLabel} onChange={(e) => setNewLabel(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', outline: 'none' }} />
                </div>
              </div>

              <div>
                <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>{tr('ui.displayTextShownInListsE')}</label>
                <input type="text" maxLength={80} placeholder="Paybill ****1234" value={newMaskedDisplay} onChange={(e) => setNewMaskedDisplay(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', outline: 'none' }} />
              </div>

              <div>
                <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>{tr('ui.credentialsJsonWhateverFieldsTheReal')}</label>
                <textarea rows={5} value={newCredentialsJson} onChange={(e) => setNewCredentialsJson(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', fontFamily: 'monospace', outline: 'none' }} />
              </div>
            </div>

            <div style={{ padding: '14px 20px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button disabled={paymentMethodsBusy} className="admin-btn-sec" onClick={() => setAddPaymentMethodModalOpen(false)}>{tr('ui.cancel')}</button>
              <button disabled={paymentMethodsBusy} className="admin-btn-pri" onClick={handleSavePlatformPaymentMethod}>
                <span>{tr('ui.savePaymentMethod')}</span>
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
                    <span className={`badge ${selectedGrvObj.target === 'DOCTOR' ? 'badge-rose' : 'badge-blue'}`}>{selectedGrvObj.target === 'DOCTOR' ? tr('ui2.againstDoctor') : tr('ui2.againstSystem')}</span>
                    <span className="badge badge-amber">{selectedGrvObj.status.replace(/_/g, ' ')}</span>
                  </div>
                  <div style={{ fontSize: '11px', color: '#64748B', marginTop: '2px' }}>{tr('mix.filedRef', { time: selectedGrvObj.timeFormatted, ref: selectedGrvObj.consultationId })}</div>
                </div>
              </div>
              <button onClick={() => setSelectedGrievanceId(null)} style={{ background: 'transparent', border: 'none', color: '#94A3B8', fontSize: '20px', cursor: 'pointer' }}>✕</button>
            </div>

            <div style={{ padding: '20px', maxHeight: 'calc(85vh - 130px)', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px' }}>
                <div style={{ border: '1px solid #E2E8F0', borderRadius: '10px', padding: '12px 14px', background: '#F8FAFC' }}>
                  <div style={{ fontSize: '10.5px', fontWeight: 750, color: '#475569', textTransform: 'uppercase', marginBottom: '6px' }}>{tr('ui.complainantPatient')}</div>
                  <div style={{ fontSize: '14px', fontWeight: 800, color: '#0F172A' }}>{selectedGrvObj.patient.name} ({selectedGrvObj.patient.ageGender})</div>
                  <div style={{ fontSize: '11.5px', color: '#64748B', marginTop: '2px' }}>{tr('mix.phonePrefix')}{selectedGrvObj.patient.phone}</div>
                </div>

                <div style={{ border: '1px solid #E2E8F0', borderRadius: '10px', padding: '12px 14px', background: '#F8FAFC' }}>
                  <div style={{ fontSize: '10.5px', fontWeight: 750, color: '#475569', textTransform: 'uppercase', marginBottom: '6px' }}>{tr('ui.treatingDoctor')}</div>
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
                <div style={{ fontSize: '11px', fontWeight: 800, color: '#1E293B', textTransform: 'uppercase', marginBottom: '10px' }}>{tr('ui.forensicTelemetrySystemAudit')}</div>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '8px', marginBottom: '12px' }}>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>{tr('ui.callDuration')}</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#DC2626' }}>{selectedGrvObj.telemetryEvidence.callDuration}</div>
                  </div>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>{tr('ui.webrtcAudioVideo')}</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#059669' }}>{selectedGrvObj.telemetryEvidence.connectionStatus}</div>
                  </div>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>{tr('ui.ePrescription')}</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#DC2626' }}>{selectedGrvObj.telemetryEvidence.rxIssued}</div>
                  </div>
                  <div style={{ background: '#fff', border: '1px solid #E2E8F0', padding: '8px 10px', borderRadius: '8px', textAlign: 'center' }}>
                    <div style={{ fontSize: '10px', color: '#64748B' }}>{tr('ui.settlement')}</div>
                    <div style={{ fontSize: '12px', fontWeight: 800, color: '#2563EB' }}>{selectedGrvObj.telemetryEvidence.paymentStatus}</div>
                  </div>
                </div>
              </div>

              {selectedGrvObj.backendId && <GrievanceInvestigation key={selectedGrvObj.backendId} grievanceId={selectedGrvObj.backendId} />}

              <label>Investigation decision notes
                <textarea className="auth-input" aria-label={tr('ui.investigationDecisionNotes')} minLength={8} maxLength={1000} value={adjudicationNotes} onChange={e => setAdjudicationNotes(e.target.value)} placeholder={tr('ui.recordTheEvidenceAndReasonFor')} />
              </label>

              {selectedGrvObj.adjudication && (
                <div style={{ border: '1px solid #86EFAC', borderRadius: '10px', padding: '12px 14px', background: '#F0FDF4' }}>
                  <div style={{ fontSize: '11px', fontWeight: 750, color: '#166534', textTransform: 'uppercase', marginBottom: '4px' }}>{tr('ui.adjudicationDecisionRecorded')}</div>
                  <div style={{ fontSize: '12px', color: '#14532D' }}>{selectedGrvObj.adjudication.notes}</div>
                </div>
              )}
            </div>

            <div style={{ padding: '14px 20px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <button className="admin-btn-sec" onClick={() => setSelectedGrievanceId(null)}>{tr('ui.close')}</button>
              <div style={{ display: 'flex', gap: '8px' }}>
                <button disabled={financeBusy || !selectedGrvObj.backendId || adjudicationNotes.trim().length < 8} className="admin-btn-pri" onClick={() => handleAdjudicateGrievance(selectedGrvObj.backendId!, 'REFUND')} style={{ background: '#059669', borderColor: '#059669' }}>
                  <span>{tr('ui.recordRefundDecision')}</span>
                </button>
                <button disabled={financeBusy || !selectedGrvObj.backendId || adjudicationNotes.trim().length < 8} className="admin-btn-pri" onClick={() => handleAdjudicateGrievance(selectedGrvObj.backendId!, 'WARN')} style={{ background: '#DC2626', borderColor: '#DC2626' }}>
                  <span>{tr('ui.recordDoctorWarning')}</span>
                </button>
                <button disabled={financeBusy || !selectedGrvObj.backendId || adjudicationNotes.trim().length < 8} className="admin-btn-sec" onClick={() => handleAdjudicateGrievance(selectedGrvObj.backendId!, 'RESOLVE')}>
                  <span>{tr('ui.dismissGrievance')}</span>
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
            <div style={{ background: 'linear-gradient(90deg, var(--afya-sidebar-top), #0b6b55)', color: '#fff', padding: '18px 22px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
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
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>{tr('ui.grossInflowFee')}</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#0F172A', marginTop: '4px' }}>KES  {selectedTxObj.grossAmount}</div>
                <div style={{ fontSize: '10.5px', color: '#10B981', fontWeight: 700 }}>{tr('ui.debitedFromMfsWallet')}</div>
              </div>
              <div style={{ background: '#FFFFFF', padding: '14px 18px' }}>
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>{tr('ui.helodocPlatformFee20')}</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#059669', marginTop: '4px' }}>KES  {selectedTxObj.platformFeeAmount} ({selectedTxObj.platformFeePercent}%)</div>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>{tr('ui.platformCommissionCut')}</div>
              </div>
              <div style={{ background: '#FFFFFF', padding: '14px 18px' }}>
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>{tr('ui.doctorNetRefund')}</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#2563EB', marginTop: '4px' }}>KES  {selectedTxObj.netAmount}</div>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>{tr('ui.payableToPhysicianWallet')}</div>
              </div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '10.5px', color: '#64748B', padding: '8px 18px 0', background: '#FFFFFF' }}>
              <span>ℹ️</span>
              <span>{tr('ui.totalCalculationIsBasedIncludingThe')}</span>
            </div>

            <div style={{ padding: '20px 22px', display: 'flex', flexDirection: 'column', gap: '16px', maxHeight: '60vh', overflowY: 'auto', background: '#FAFAFA' }}>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
                <div style={{ background: '#FFFFFF', border: '1px solid #E2E8F0', borderRadius: '10px', padding: '14px' }}>
                  <div style={{ fontSize: '11px', fontWeight: 800, textTransform: 'uppercase', color: '#64748B', marginBottom: '8px' }}>{tr('ui.payerPatient')}</div>
                  <strong style={{ fontSize: '13px', color: '#0F172A' }}>{selectedTxObj.patient.name}</strong>
                  <div style={{ fontSize: '11px', color: '#64748B' }}>{selectedTxObj.patient.phone}</div>
                </div>

                <div style={{ background: '#FFFFFF', border: '1px solid #E2E8F0', borderRadius: '10px', padding: '14px' }}>
                  <div style={{ fontSize: '11px', fontWeight: 800, textTransform: 'uppercase', color: '#64748B', marginBottom: '8px' }}>{tr('ui.recipientDoctorSystem')}</div>
                  <strong style={{ fontSize: '13px', color: '#0F172A' }}>{selectedTxObj.doctor.name}</strong>
                  <div style={{ fontSize: '11px', color: '#64748B' }}>{selectedTxObj.doctor.registrationNumber} • {selectedTxObj.doctor.specialty}</div>
                </div>
              </div>

              <div style={{ background: '#FFFFFF', border: '1px solid #E2E8F0', borderRadius: '10px', padding: '14px' }}>
                <div style={{ fontSize: '11px', fontWeight: 800, textTransform: 'uppercase', color: '#64748B', marginBottom: '8px' }}>{tr('ui.mfsWebhookPayload')}</div>
                <pre style={{ background: '#0F172A', color: '#E2E8F0', padding: '12px', borderRadius: '8px', fontFamily: 'monospace', fontSize: '11px', overflowX: 'auto', margin: 0 }}>
                  {JSON.stringify(selectedTxObj.gatewayPayload, null, 2)}
                </pre>
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#FFFFFF', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <button className="sheet-btn" onClick={() => showToast(tr('ui.notYetAvailable'), tr('ui.taxVoucherReceiptGenerationIsNot'), 'ℹ️', 'info')} style={{ padding: '8px 16px', fontSize: '12px', background: '#F1F5F9', border: '1px solid #CBD5E1', color: '#334155', borderRadius: '8px', cursor: 'pointer' }}>
                {tr('ui.printSaveTaxVoucher')}
              </button>
              <button className="sheet-btn primary" onClick={() => setSelectedTransactionId(null)} style={{ padding: '8px 20px', fontSize: '12px', background: 'var(--afya-green)', color: '#fff', borderRadius: '10px', cursor: 'pointer' }}>
                {tr('ui.closeAuditWindow')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
