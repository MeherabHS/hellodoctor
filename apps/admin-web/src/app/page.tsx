'use client';

import React, { useState } from 'react';
import { useBackendSync } from '@/lib/useBackendSync';
import {
  initiateDisbursement,
  verifyBmdcDoctor,
  rejectBmdcDoctor,
  simulateAdminLog,
  adjudicateGrievanceRefund,
  adjudicateGrievanceWarn,
  adjudicateGrievanceDismiss,
} from '@/lib/api';
import {
  ADMIN_DOCTORS_STORE,
  ADMIN_PATIENT_STORE,
  INITIAL_ERROR_LOGS,
  INITIAL_GRIEVANCES,
  INITIAL_TRANSACTIONS,
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
  | 'bmdc'
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
  // Navigation State
  const [activeAdminPage, setActiveAdminPage] = useState<AdminPageId>('doctors');

  // Interactive Data Stores
  const [doctorStore, setDoctorStore] = useState<Record<string, DoctorHistoryItem>>(ADMIN_DOCTORS_STORE);
  const [patientStore, setPatientStore] = useState<Record<string, PatientRecordItem>>(ADMIN_PATIENT_STORE);
  const [errorLogs, setErrorLogs] = useState<AppErrorLogItem[]>(INITIAL_ERROR_LOGS);
  const [grievances, setGrievances] = useState<GrievanceItem[]>(INITIAL_GRIEVANCES);
  const [transactions, setTransactions] = useState<TransactionItem[]>(INITIAL_TRANSACTIONS);

  // Live Backend Synchronization Hook
  // Once the backend has answered at least once, its data is the source of truth —
  // including a correctly-empty result — and fully replaces the initial mock seed data.
  useBackendSync({
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
  });

  // Escrow / Payout State
  const [payoutsSettled, setPayoutsSettled] = useState(false);
  const [escrowTotal, setEscrowTotal] = useState('৳ 890,000');

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
  const [simUser, setSimUser] = useState('sarah');
  const [simScenario, setSimScenario] = useState('bkash_timeout');
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
      breadcrumb: 'Real-time bKash, Nagad & Card Inflows • 20% Platform Commission Splits • Escrow Release Audit',
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
    bmdc: {
      title: 'Doctor Credentialing & BMDC KYC Hub',
      breadcrumb: 'Physician Regulatory KYC • NID & Certificate OCR Match',
    },
    compliance: {
      title: 'Clinical e-Prescription & Q&A Audit',
      breadcrumb: 'DGDA Antibiotic Stewardship & Narcotic Schedule H Compliance',
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

  // 1. Batch Payout Action
  function handleExecuteBatchPayout() {
    setPayoutsSettled(true);
    setEscrowTotal('৳ 0 (Disbursed)');
    showToast('Payout Batch Disbursed! ৳ 890,000', 'Successfully disbursed settlements to 6 active physicians via bKash & Nagad APIs.', '💸', 'success');

    const now = new Date();
    const start = new Date(now.getFullYear(), now.getMonth(), 1).toISOString().split('T')[0];
    const end = now.toISOString().split('T')[0];
    initiateDisbursement(start, end).catch((e) => console.warn('Disbursement API notice:', e));
  }

  // 2. BMDC Approval
  function handleBmdcAction(action: 'approve' | 'clarify' | 'reject') {
    if (action === 'approve') {
      showToast('BMDC Reg A-48291 Approved ✓', 'Dr. Sarah Rahman credentialed & scheduled for live teleconsults.', '✓', 'success');
      verifyBmdcDoctor('da000001-0000-0000-0000-000000000001', 'BMDC Reg A-48291 Approved').catch((e) =>
        console.warn('BMDC verify API notice:', e)
      );
    } else if (action === 'clarify') {
      showToast('Clarification Sent to Physician', 'Requested higher-resolution scan of Bangladesh Medical & Dental Council certificate.', '⚠️', 'info');
    } else {
      showToast('Application Declined', 'Physician notification dispatched with rejection grounds.', '✕', 'warning');
      rejectBmdcDoctor('da000001-0000-0000-0000-000000000001', 'Application Declined').catch((e) =>
        console.warn('BMDC reject API notice:', e)
      );
    }
  }

  // 3. Log Simulation
  function handleExecuteSimulateFailure() {
    const newId = 'ERR-' + Date.now().toString().slice(-8);
    const traceNum = Math.floor(10000 + Math.random() * 90000);
    const traceId = `TRC-${traceNum}-FAIL`;

    const userMap: Record<string, any> = {
      sarah: { id: 'USR-8921', name: 'Sarah Khan', role: 'Patient', phone: '+880 1711-234567', avatar: 'SK', avatarBg: '#059669', device: 'Samsung Galaxy S24', appVersion: 'v2.4.1', network: 'GP 4G', ip: '103.114.98.24' },
      sabrina: { id: 'DOC-45821', name: 'Dr. Sabrina Akter', role: 'Doctor', phone: '+880 1819-334455', avatar: 'SA', avatarBg: '#2563EB', device: 'MacBook Pro M3', appVersion: 'Web v2.4.0', network: 'Hospital Fiber', ip: '103.205.71.18' },
      rafiq: { id: 'USR-7410', name: 'Rafiq Ahmed', role: 'Patient', phone: '+880 1819-987654', avatar: 'RA', avatarBg: '#059669', device: 'Xiaomi Redmi 13', appVersion: 'v2.4.1', network: 'Robi 3G', ip: '119.30.38.102' },
      anika: { id: 'DOC-74921', name: 'Dr. Anika Rahman', role: 'Doctor', phone: '+880 1712-445566', avatar: 'AR', avatarBg: '#D97706', device: 'iPhone 15 Pro', appVersion: 'v2.3.9', network: 'Banglalink 4G', ip: '103.88.232.14' },
    };

    const targetUser = userMap[simUser] || userMap.sarah;

    const newLog: AppErrorLogItem = {
      id: newId,
      traceId,
      timestamp: new Date().toISOString(),
      timeFormatted: 'Just now, ' + new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      relativeTime: 'Just now',
      user: { ...targetUser, ageGender: targetUser.role === 'Doctor' ? 'Specialist' : '30+' },
      severity: simSeverity as any,
      subsystem: simScenario === 'bkash_timeout' ? 'Payment Gateway' : 'Agora WebRTC',
      component: simScenario === 'bkash_timeout' ? '/api/v2/payment/bkash/execute-agreement' : "agora.joinChannel('room_hd_9104')",
      actionAttempted: 'Simulated Diagnostic Action',
      failedPart: simScenario === 'bkash_timeout' ? 'bKash Callback Handshake' : 'Agora ICE Pairing',
      errorCode: simScenario === 'bkash_timeout' ? 'HTTP 504 GATEWAY_TIMEOUT' : 'ICE_FAILED (702)',
      errorMessage: 'Artificial incident injected by QA console.' + (simCustomNote ? ` [${simCustomNote}]` : ''),
      stackTrace: 'Error: SimulatedException [500]\n    at IngestionWorker (test.rs:42:10)',
      status: 'UNRESOLVED',
      resolvedAt: null,
      resolutionNote: null,
    };

    setErrorLogs((prev) => [newLog, ...prev]);
    setSimulateModalOpen(false);
    showToast('⚡ Failure Incident Injected', `${newLog.failedPart} failure logged against ${targetUser.name}.`, '⚡', 'urgent');

    simulateAdminLog({
      action: newLog.actionAttempted,
      subsystem: newLog.subsystem,
      severity: newLog.severity,
      error_summary: newLog.errorMessage,
      stack_trace: newLog.stackTrace,
    }).catch((e) => console.warn('Log simulate API notice:', e));
  }

  // 4. Grievance Adjudication
  function handleAdjudicateGrievance(grvId: string, action: 'REFUND' | 'WARN' | 'RESOLVE') {
    setGrievances((prev) =>
      prev.map((g) => {
        if (g.id !== grvId) return g;
        if (action === 'REFUND') {
          return {
            ...g,
            status: 'REFUNDED',
            boardRemedy: 'Escrow Refund Disbursed (৳800)',
            adjudication: {
              action: 'REFUNDED',
              date: 'Today, ' + new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
              admin: 'Central Escrow Adjudication',
              notes: 'Full consultation fee refunded to patient mobile financial wallet via automated escrow reversal.',
            },
          };
        } else if (action === 'WARN') {
          return {
            ...g,
            status: 'WARNED',
            boardRemedy: 'BMDC Disciplinary Warning Logged',
            adjudication: {
              action: 'DOCTOR_WARNED',
              date: 'Today, ' + new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
              admin: 'Central Clinical Governance Board',
              notes: 'Formal ethics and conduct warning issued under BMDC Code of Ethics Sec 4.',
            },
          };
        } else {
          return {
            ...g,
            status: 'DISMISSED',
            boardRemedy: 'Claim Reviewed and Dismissed — No Compliance Action Warranted',
            adjudication: {
              action: 'DISMISS',
              date: 'Today, ' + new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
              admin: 'Central Medical Operations',
              notes: 'Dispute review concluded. Telemetry evidence did not substantiate the claim.',
            },
          };
        }
      })
    );

    if (action === 'REFUND') {
      showToast('Refund Disbursed 💳', 'Full fee refunded to patient wallet via bKash.', '💳', 'success');
      adjudicateGrievanceRefund(grvId, 'Central Escrow Adjudication refund').catch((e) =>
        console.warn('Grievance refund API notice:', e)
      );
    } else if (action === 'WARN') {
      showToast('Doctor Warned ⚠️', 'Formal clinical misconduct warning registered in compliance dossier.', '⚠️', 'warning');
      adjudicateGrievanceWarn(grvId, 'Central Clinical Governance warning').catch((e) =>
        console.warn('Grievance warn API notice:', e)
      );
    } else {
      showToast('Grievance Dismissed ✓', 'Dispute claim reviewed and dismissed — no compliance action warranted.', '✓', 'success');
      adjudicateGrievanceDismiss(grvId, 'Telemetry evidence did not substantiate the claim').catch((e) =>
        console.warn('Grievance dismiss API notice:', e)
      );
    }
  }

  // Log resolve toggle
  function handleToggleLogResolve(logId: string) {
    setErrorLogs((prev) =>
      prev.map((l) => {
        if (l.id !== logId) return l;
        const isResolved = l.status === 'RESOLVED';
        return {
          ...l,
          status: isResolved ? 'UNRESOLVED' : 'RESOLVED',
          resolvedAt: isResolved ? null : 'Today, ' + new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
          resolutionNote: isResolved ? null : 'Administrative resolution signed off by Clinical Ops Director.',
        };
      })
    );
    showToast('Log Status Updated', 'Telemetry exception status toggled.', '🔄', 'info');
  }

  // Counters
  const unresolvedLogsCount = errorLogs.filter((l) => l.status !== 'RESOLVED').length;
  const pendingGrievancesCount = grievances.filter((g) => g.status === 'PENDING_REVIEW' || g.status === 'UNDER_INVESTIGATION').length;

  // Filtered lists
  const filteredDoctors = Object.values(doctorStore).filter((doc) => {
    const text = (doc.name + ' ' + doc.bmdc + ' ' + doc.meta).toLowerCase();
    const matchesSearch = !doctorSearchQuery || text.includes(doctorSearchQuery.toLowerCase());
    const isSettled = payoutsSettled || doc.key === 'farhana' || doc.key === 'tariqul';
    const matchesStatus =
      doctorStatusFilter === 'all' ? true : doctorStatusFilter === 'settled' ? isSettled : !isSettled;
    return matchesSearch && matchesStatus;
  });

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
    if (logQuickFilter === 'payment' && log.subsystem !== 'Payment Gateway') return false;
    if (logQuickFilter === 'webrtc' && log.subsystem !== 'Agora WebRTC') return false;
    if (logQuickFilter === 'offline' && log.subsystem !== 'Sync & Offline Cache') return false;
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
            <span className="admin-nav-badge amber">{payoutsSettled ? 'Settled' : '4 Pending'}</span>
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
            className={`admin-nav-item ${activeAdminPage === 'bmdc' ? 'active' : ''}`}
            id="adminNav-bmdc"
            onClick={() => setActiveAdminPage('bmdc')}
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><path d="m9 15 2 2 4-4"/></svg>
            <span>BMDC Credentialing</span>
            <span className="admin-nav-badge blue">3</span>
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
                placeholder="Search doctor, BMDC #, patient ID, TxID..."
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
              onClick={() => showToast('16263 Emergency Bridge Active 🚨', 'Direct audio interconnect routed to DGHS National Health Call Center.', '🚨', 'urgent')}
            >
              <span style={{ width: '7px', height: '7px', borderRadius: '50%', background: '#DC2626' }}></span>
              <span>🚨 16263 Helpline Bridge</span>
            </button>

            <div
              className="admin-top-icon-btn"
              title="Pending Notifications"
              onClick={() => showToast('Central Notifications', '3 BMDC physician applications pending review.', '🔔', 'info')}
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
                  <button className="admin-batch-payout-btn" onClick={handleExecuteBatchPayout}>
                    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>
                    <span>Process Batch Payout</span>
                    <span style={{ background: 'rgba(255,255,255,0.2)', padding: '2px 6px', borderRadius: '6px', fontSize: '10.5px', marginLeft: '4px' }}>bKash + Nagad</span>
                  </button>
                </div>
              </div>

              {/* 4 Metric Cards with Micro-Charts */}
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Gross Telehealth Revenue</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">৳ 2,450,000</div>
                  <span className="admin-kpi-delta up">↗ +8.5% this month</span>
                  <div className="kpi-micro-chart kpi-sparkline">
                    <svg viewBox="0 0 200 36" preserveAspectRatio="none">
                      <defs>
                        <linearGradient id="sparkGradRev" x1="0" y1="0" x2="0" y2="1">
                          <stop offset="0%" stopColor="#059669" stopOpacity="0.2"/>
                          <stop offset="100%" stopColor="#059669" stopOpacity="0"/>
                        </linearGradient>
                      </defs>
                      <path d="M0 30 L33 26 L66 28 L100 20 L133 16 L166 12 L200 4 L200 36 L0 36 Z" fill="url(#sparkGradRev)"/>
                      <polyline points="0,30 33,26 66,28 100,20 133,16 166,12 200,4" fill="none" stroke="#059669" strokeWidth="2" strokeLinecap="round"/>
                      <circle cx="200" cy="4" r="2.5" fill="#059669" stroke="#fff" strokeWidth="1.5"/>
                    </svg>
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '9px', color: '#94A3B8', padding: '0 2px', marginTop: '1px' }}>
                      <span>Apr</span><span>May</span><span>Jun</span><span>Jul</span><span>Aug</span><span>Sep</span><span style={{ color: '#059669', fontWeight: 700 }}>Oct</span>
                    </div>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">24h Chat Revenue (৳ 300 tier)</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">৳ 380,000</div>
                  <div className="kpi-micro-chart">
                    <div className="kpi-donut-wrap">
                      <svg viewBox="0 0 42 42">
                        <circle cx="21" cy="21" r="16" fill="none" stroke="#E2E8F0" strokeWidth="5"/>
                        <circle cx="21" cy="21" r="16" fill="none" stroke="#2563EB" strokeWidth="5" strokeDasharray="62 38.5" strokeDashoffset="25" strokeLinecap="round"/>
                        <text x="21" y="23" textAnchor="middle" fontSize="9" fontWeight="800" fill="#2563EB">62%</text>
                      </svg>
                      <div className="kpi-donut-legend">
                        <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#2563EB' }}></span> 784 New Users</div>
                        <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#E2E8F0' }}></span> 483 Returning</div>
                        <div className="kpi-donut-legend-item" style={{ fontWeight: 700, color: '#0F172A', marginTop: '1px' }}>1,267 total sessions</div>
                      </div>
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
                  <div className="admin-kpi-val">৳ 490,000</div>
                  <span className="admin-kpi-delta neutral">Automated commission cut</span>
                  <div className="kpi-micro-chart kpi-stacked-bar">
                    <div className="kpi-bar-row">
                      <span className="kpi-bar-label">Video</span>
                      <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: '78%', background: 'linear-gradient(90deg,#0D9488,#5EEAD4)' }}></div></div>
                      <span className="kpi-bar-value">৳ 382k</span>
                    </div>
                    <div className="kpi-bar-row">
                      <span className="kpi-bar-label">Chat</span>
                      <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: '22%', background: 'linear-gradient(90deg,#14B8A6,#99F6E4)' }}></div></div>
                      <span className="kpi-bar-value">৳ 108k</span>
                    </div>
                  </div>
                </div>

                <div className="admin-kpi-card" style={{ borderColor: '#FCD34D' }}>
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Pending Doctor Wallet Escrow</span>
                    <div className="admin-kpi-icon-circle amber" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" id="adminEscrowDisplay">
                    {payoutsSettled ? (
                      <span style={{ color: '#059669', fontSize: '20px' }}>৳ 0 (Disbursed)</span>
                    ) : (
                      escrowTotal
                    )}
                  </div>
                  <div className="kpi-micro-chart">
                    <div className="kpi-pulse-indicator" style={{ marginBottom: '4px' }}>
                      <div className="kpi-pulse-dot" style={{ background: '#D97706' }}></div>
                      <span className="kpi-pulse-text" style={{ color: '#D97706' }}>
                        {payoutsSettled ? 'All 14 doctors settled' : '14 doctors awaiting disbursement'}
                      </span>
                    </div>
                    <div style={{ display: 'flex', gap: '6px' }}>
                      <span style={{ fontSize: '9.5px', background: '#FEF3C7', color: '#92400E', padding: '3px 8px', borderRadius: '6px', fontWeight: 700 }}>bKash × 8</span>
                      <span style={{ fontSize: '9.5px', background: '#FEF3C7', color: '#92400E', padding: '3px 8px', borderRadius: '6px', fontWeight: 700 }}>Nagad × 4</span>
                      <span style={{ fontSize: '9.5px', background: '#FEF3C7', color: '#92400E', padding: '3px 8px', borderRadius: '6px', fontWeight: 700 }}>BEFTN × 2</span>
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
                    <div className="admin-card-subtitle">Search, filter &amp; audit BMDC specialists, consultation loads, and payment settlements</div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
                    <div style={{ position: 'relative', minWidth: '240px', maxWidth: '320px' }}>
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '10px', top: '50%', transform: 'translateY(-50%)', pointerEvents: 'none' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
                        placeholder="Search doctor, BMDC #, specialty, method..."
                        value={doctorSearchQuery}
                        onChange={(e) => setDoctorSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 12px 7px 30px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', outline: 'none' }}
                      />
                    </div>

                    <div style={{ display: 'flex', gap: '6px' }}>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'all' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('all')}>All (6)</button>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'pending' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('pending')}>Pending (4)</button>
                      <button className={`admin-filter-pill ${doctorStatusFilter === 'settled' ? 'active' : ''}`} onClick={() => setDoctorStatusFilter('settled')}>Settled (2)</button>
                    </div>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Doctor Name</th>
                        <th>BMDC Reg</th>
                        <th>Completed Visits</th>
                        <th>24h Chat Sessions</th>
                        <th>Gross Earnings</th>
                        <th>Platform Cut</th>
                        <th>Net Payout (৳)</th>
                        <th>Payout Method</th>
                        <th>Status</th>
                        <th>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredDoctors.map((doc) => {
                        const isSettled = payoutsSettled || doc.key === 'farhana' || doc.key === 'tariqul';
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
                            <td><span className="badge badge-slate" style={{ fontWeight: 750 }}>{doc.bmdc}</span></td>
                            <td><b>{doc.kpiVisits}</b></td>
                            <td>{doc.kpiChats}</td>
                            <td>{doc.kpiGross}</td>
                            <td style={{ color: '#64748B' }}>৳ 15,720</td>
                            <td style={{ fontWeight: 850, color: '#059669' }}>{doc.kpiDisbursed}</td>
                            <td>
                              <span className="payout-method-badge bkash">
                                {doc.disbursements[0]?.method || 'bKash Merchant'}
                              </span>
                            </td>
                            <td>
                              <span className={`badge ${isSettled ? 'badge-green' : 'badge-amber'} doc-payout-status`}>
                                {isSettled ? 'Settled ✓' : 'Pending Settlement'}
                              </span>
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

          {/* PAGE 2: EXECUTIVE COMMAND CENTER */}
          {activeAdminPage === 'command' && (
            <div className="admin-page-view active" id="adminPage_command">
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Total Consultations Today</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">1,480</div>
                  <span className="admin-kpi-delta up">↗ +12.5% vs yesterday</span>
                  <div className="kpi-micro-chart kpi-sparkline">
                    <svg viewBox="0 0 200 40" preserveAspectRatio="none">
                      <defs>
                        <linearGradient id="sparkGrad1" x1="0" y1="0" x2="0" y2="1">
                          <stop offset="0%" stopColor="#059669" stopOpacity="0.25"/>
                          <stop offset="100%" stopColor="#059669" stopOpacity="0"/>
                        </linearGradient>
                      </defs>
                      <path d="M0 32 L28 28 L56 22 L84 25 L112 18 L140 14 L168 10 L200 6 L200 40 L0 40 Z" fill="url(#sparkGrad1)"/>
                      <polyline points="0,32 28,28 56,22 84,25 112,18 140,14 168,10 200,6" fill="none" stroke="#059669" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                      <circle cx="200" cy="6" r="3" fill="#059669" stroke="#fff" strokeWidth="1.5"/>
                    </svg>
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '9px', color: '#94A3B8', marginTop: '2px', padding: '0 2px' }}>
                      <span>Mon</span><span>Tue</span><span>Wed</span><span>Thu</span><span>Fri</span><span>Sat</span><span style={{ color: '#059669', fontWeight: 700 }}>Today</span>
                    </div>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Active Specialists Online</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="8.5" cy="7" r="4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">142</div>
                  <div className="kpi-micro-chart">
                    <div className="kpi-donut-wrap">
                      <svg viewBox="0 0 42 42">
                        <circle cx="21" cy="21" r="16" fill="none" stroke="#E2E8F0" strokeWidth="5"/>
                        <circle cx="21" cy="21" r="16" fill="none" stroke="#2563EB" strokeWidth="5" strokeDasharray="94.4 6.1" strokeDashoffset="25" strokeLinecap="round"/>
                        <text x="21" y="23" textAnchor="middle" fontSize="10" fontWeight="800" fill="#2563EB">94%</text>
                      </svg>
                      <div className="kpi-donut-legend">
                        <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#2563EB' }}></span> 134 On-Duty</div>
                        <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#F59E0B' }}></span> 22 On Break</div>
                        <div className="kpi-donut-legend-item"><span className="dot" style={{ background: '#E2E8F0' }}></span> 8 Offline</div>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">24h Paid Chats Active</span>
                    <div className="admin-kpi-icon-circle purple" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">380</div>
                  <span className="admin-kpi-delta neutral">1,500 live messages</span>
                  <div className="kpi-micro-chart kpi-stacked-bar">
                    <div className="kpi-bar-row">
                      <span className="kpi-bar-label">Video</span>
                      <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: '72%', background: 'linear-gradient(90deg,#7E22CE,#A855F7)' }}></div></div>
                      <span className="kpi-bar-value">274</span>
                    </div>
                    <div className="kpi-bar-row">
                      <span className="kpi-bar-label">Chat</span>
                      <div className="kpi-bar-track"><div className="kpi-bar-fill" style={{ width: '28%', background: 'linear-gradient(90deg,#C084FC,#E9D5FF)' }}></div></div>
                      <span className="kpi-bar-value">106</span>
                    </div>
                  </div>
                </div>

                <div className="admin-kpi-card" style={{ borderColor: '#FCA5A5' }}>
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Emergency 16263 Escalations</span>
                    <div className="admin-kpi-icon-circle red" style={{ width: '34px', height: '34px', borderRadius: '10px' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#DC2626' }}>16,263</div>
                  <div className="kpi-micro-chart">
                    <div className="kpi-pulse-indicator">
                      <div className="kpi-pulse-dot"></div>
                      <span className="kpi-pulse-text">3 red-flag alerts in last 30 min</span>
                    </div>
                    <div style={{ display: 'flex', gap: '6px', marginTop: '8px' }}>
                      <span style={{ fontSize: '9.5px', background: '#FEF2F2', color: '#DC2626', padding: '3px 8px', borderRadius: '6px', fontWeight: 700 }}>🚨 Chest Pain × 1</span>
                      <span style={{ fontSize: '9.5px', background: '#FEF2F2', color: '#DC2626', padding: '3px 8px', borderRadius: '6px', fontWeight: 700 }}>🚨 Seizure × 2</span>
                    </div>
                  </div>
                </div>
              </div>

              {/* 2-Column Command Layout */}
              <div className="command-dashboard-grid">
                <div className="admin-card">
                  <div className="admin-card-header">
                    <div>
                      <div className="admin-card-title">Hourly Consultation Volume &amp; Load</div>
                      <div className="admin-card-subtitle">Peak shifts: Morning 08:00–12:00 AM &amp; Evening 05:00–10:00 PM</div>
                    </div>
                    <div style={{ display: 'flex', gap: '6px' }}>
                      <button className="admin-filter-pill active" style={{ padding: '4px 10px', fontSize: '11px' }}>Today</button>
                      <button className="admin-filter-pill" style={{ padding: '4px 10px', fontSize: '11px' }}>7 Days</button>
                    </div>
                  </div>

                  <div style={{ background: '#F8FAFC', borderRadius: '12px', padding: '16px 12px', border: '1px solid #E2E8F0' }}>
                    <svg viewBox="0 0 540 180" style={{ width: '100%', height: '160px', overflow: 'visible' }}>
                      <defs>
                        <linearGradient id="chartGrad" x1="0%" y1="0%" x2="0%" y2="100%">
                          <stop offset="0%" stopColor="#059669" stopOpacity="0.3"/>
                          <stop offset="100%" stopColor="#059669" stopOpacity="0.0"/>
                        </linearGradient>
                      </defs>
                      <line x1="30" y1="30" x2="520" y2="30" stroke="#E2E8F0" strokeDasharray="3,3"/>
                      <line x1="30" y1="75" x2="520" y2="75" stroke="#E2E8F0" strokeDasharray="3,3"/>
                      <line x1="30" y1="120" x2="520" y2="120" stroke="#E2E8F0" strokeDasharray="3,3"/>
                      <path d="M 40 150 C 90 145, 120 70, 160 85 C 200 100, 240 135, 280 115 C 320 95, 360 40, 400 50 C 440 60, 480 130, 510 150 L 510 150 L 40 150 Z" fill="url(#chartGrad)"/>
                      <path d="M 40 150 C 90 145, 120 70, 160 85 C 200 100, 240 135, 280 115 C 320 95, 360 40, 400 50 C 440 60, 480 130, 510 150" fill="none" stroke="#059669" strokeWidth="3"/>
                    </svg>
                  </div>
                </div>

                <div className="admin-card">
                  <div className="admin-card-header">
                    <div>
                      <div className="admin-card-title">Live WebRTC Stream Monitor</div>
                      <div className="admin-card-subtitle">Real-time Agora RTC bitrate &amp; packet latency monitoring</div>
                    </div>
                  </div>

                  <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                    <div className="webrtc-stream-card">
                      <div>
                        <strong style={{ fontSize: '12px', color: '#0F172A' }}>Call #897103 • Dr. Anisul Haque (BMDC 74902)</strong>
                        <div style={{ fontSize: '10.5px', color: '#64748B', marginTop: '2px' }}>Patient: P-ANON-A03 (Dhaka) • Elapsed: <b>08:45 / 10:00</b></div>
                      </div>
                      <span className="webrtc-ping-badge good">34ms • HD Audio/Video</span>
                    </div>

                    <div className="webrtc-stream-card">
                      <div>
                        <strong style={{ fontSize: '12px', color: '#0F172A' }}>Call #897104 • Dr. Sabrina Akter (BMDC 45821)</strong>
                        <div style={{ fontSize: '10.5px', color: '#64748B', marginTop: '2px' }}>Patient: P-ANON-D91 (Dhanmondi) • Elapsed: <b>05:12 / 10:00</b></div>
                      </div>
                      <span className="webrtc-ping-badge good">42ms • HD Audio/Video</span>
                    </div>

                    <div className="webrtc-stream-card">
                      <div>
                        <strong style={{ fontSize: '12px', color: '#0F172A' }}>Call #897105 • Dr. Karim Hossain (BMDC 81551)</strong>
                        <div style={{ fontSize: '10.5px', color: '#64748B', marginTop: '2px' }}>Patient: P-ANON-K12 (Sylhet) • Elapsed: <b>09:03 / 10:00</b></div>
                      </div>
                      <span className="webrtc-ping-badge moderate">86ms (3G Adaptive)</span>
                    </div>

                    <div className="webrtc-stream-card">
                      <div>
                        <strong style={{ fontSize: '12px', color: '#0F172A' }}>Call #897106 • Dr. Sadik Al-Amin (BMDC 68192)</strong>
                        <div style={{ fontSize: '10.5px', color: '#64748B', marginTop: '2px' }}>Patient: P-ANON-M22 (Chittagong) • Elapsed: <b>02:18 / 10:00</b></div>
                      </div>
                      <span className="webrtc-ping-badge good">29ms • HD Audio/Video</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* PAGE 3: SLOT MATRIX & LOCKS */}
          {activeAdminPage === 'slots' && (
            <div className="admin-page-view active" id="adminPage_slots">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <div className="admin-card-title">Timetable 10-Minute Micro-Slot Grid &amp; 1-Hour Lock Governance</div>
                    <div className="admin-card-subtitle">Slots within &lt;60 minutes of start are strictly locked to prevent patient abandonment</div>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span className="slot-chip available" style={{ padding: '4px 8px', fontSize: '10px' }}>Available</span>
                    <span className="slot-chip booked" style={{ padding: '4px 8px', fontSize: '10px' }}>Booked (10m)</span>
                    <span className="slot-chip locked" style={{ padding: '4px 8px', fontSize: '10px' }}>🔒 &lt;1h Locked</span>
                    <span className="slot-chip chat-only" style={{ padding: '4px 8px', fontSize: '10px' }}>💬 24h Chat</span>
                  </div>
                </div>

                <div style={{ overflowX: 'auto' }}>
                  <table className="admin-slot-grid-table">
                    <thead>
                      <tr>
                        <th style={{ width: '200px' }}>Specialist</th>
                        <th>10:30 AM</th>
                        <th>10:40 AM</th>
                        <th>10:50 AM</th>
                        <th>11:00 AM</th>
                        <th>11:10 AM</th>
                        <th>11:20 AM</th>
                        <th>11:30 AM</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr>
                        <td>
                          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                            <div style={{ width: '28px', height: '28px', borderRadius: '50%', background: '#FEF3C7', color: '#B45309', display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '10.5px' }}>SA</div>
                            <div>
                              <strong style={{ fontSize: '12px', color: '#0F172A' }}>Dr. Sabrina Akter</strong>
                              <div style={{ fontSize: '10px', color: '#64748B' }}>Internal Medicine</div>
                            </div>
                          </div>
                        </td>
                        <td><div className="slot-chip locked" onClick={() => showToast('Slot Locked 🔒', 'Rafiq Ahmed (10:30 AM) consultation cannot be cancelled within 1 hour.', '🔒', 'warning')}><span>10:30 • Rafiq A.</span><span style={{ fontSize: '9.5px', opacity: 0.85 }}>🔒 Locked</span></div></td>
                        <td><div className="slot-chip locked" onClick={() => showToast('Slot Locked 🔒', 'Nusrat Jahan (10:40 AM) locked under 1h policy.', '🔒', 'warning')}><span>10:40 • Nusrat J.</span><span style={{ fontSize: '9.5px', opacity: 0.85 }}>🔒 Locked</span></div></td>
                        <td><div className="slot-chip chat-only" onClick={() => showToast('Chat Consultation', 'Kamal Uddin subscribed for 24h dedicated chat only.', '💬', 'info')}><span>10:50 • Kamal U.</span><span style={{ fontSize: '9.5px' }}>💬 24h Chat</span></div></td>
                        <td><div className="slot-chip available" onClick={() => showToast('Open Slot', '11:00 AM available for patient booking.', '✓', 'success')}><span>11:00 • Available</span></div></td>
                        <td><div className="slot-chip available" onClick={() => showToast('Open Slot', '11:10 AM available for patient booking.', '✓', 'success')}><span>11:10 • Available</span></div></td>
                        <td><div className="slot-chip booked"><span>11:20 • Tanvir C.</span><span style={{ fontSize: '9.5px' }}>Booked (10m)</span></div></td>
                        <td><div className="slot-chip available"><span>11:30 • Available</span></div></td>
                      </tr>
                      <tr>
                        <td>
                          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                            <div style={{ width: '28px', height: '28px', borderRadius: '50%', background: '#EFF6FF', color: '#1D4ED8', display: 'grid', placeItems: 'center', fontWeight: 800, fontSize: '10.5px' }}>KH</div>
                            <div>
                              <strong style={{ fontSize: '12px', color: '#0F172A' }}>Dr. Karim Hossain</strong>
                              <div style={{ fontSize: '10px', color: '#64748B' }}>Cardiology</div>
                            </div>
                          </div>
                        </td>
                        <td><div className="slot-chip available"><span>10:30 • Available</span></div></td>
                        <td><div className="slot-chip locked"><span>10:40 • T. Hasan</span><span style={{ fontSize: '9.5px', opacity: 0.85 }}>🔒 Locked</span></div></td>
                        <td><div className="slot-chip available"><span>10:50 • Available</span></div></td>
                        <td><div className="slot-chip available"><span>11:00 • Available</span></div></td>
                        <td><div className="slot-chip booked"><span>11:10 • Farzana H.</span><span style={{ fontSize: '9.5px' }}>Booked (10m)</span></div></td>
                        <td><div className="slot-chip available"><span>11:20 • Available</span></div></td>
                        <td><div className="slot-chip available"><span>11:30 • Available</span></div></td>
                      </tr>
                    </tbody>
                  </table>
                </div>

                <div className="emergency-dispatch-card">
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <div style={{ fontSize: '24px' }}>🚨</div>
                    <div>
                      <strong style={{ fontSize: '13px', color: '#991B1B' }}>Emergency Patient Re-assignment Dispatch</strong>
                      <div style={{ fontSize: '11.5px', color: '#B91C1C', marginTop: '2px' }}>Override 1-hour cancellation lock for verified doctor emergency and re-route waiting patients without re-billing.</div>
                    </div>
                  </div>
                  <button className="emergency-dispatch-btn" onClick={() => showToast('Emergency Re-assignment Dispatched', 'Patient Rafiq Ahmed re-routed to Dr. Tanvir Hossain without re-billing.', '⚡', 'urgent')}>
                    <span>Execute Emergency Override ⚡</span>
                  </button>
                </div>
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
                    <div className="admin-kpi-val">28,450</div>
                    <div style={{ fontSize: '11px', color: '#059669', fontWeight: 750 }}>+14.2% YoY growth</div>
                  </div>
                  <div className="admin-kpi-icon-circle green">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/></svg>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div>
                    <span className="admin-kpi-sub">Active 24h Chat Subscribers</span>
                    <div className="admin-kpi-val">1,267</div>
                    <div style={{ fontSize: '11px', color: '#2563EB', fontWeight: 700 }}>+8.5% growth</div>
                  </div>
                  <div className="admin-kpi-icon-circle blue">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div>
                    <span className="admin-kpi-sub">Chronic Care Cohort</span>
                    <div className="admin-kpi-val">6,820</div>
                    <div style={{ fontSize: '11px', color: '#D97706', fontWeight: 700 }}>Hypertension &amp; T2DM</div>
                  </div>
                  <div className="admin-kpi-icon-circle amber">
                    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M22 12h-4l-3 9L9 3l-3 9H2"/></svg>
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div>
                    <span className="admin-kpi-sub">Total Patient Health Spend</span>
                    <div className="admin-kpi-val">৳ 4,890,000</div>
                    <div style={{ fontSize: '11px', color: '#059669', fontWeight: 750 }}>+18.1% turnover</div>
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
                        placeholder="Search patient name, #PT ID, phone, dx..."
                        value={patientSearchQuery}
                        onChange={(e) => setPatientSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 12px 7px 30px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', outline: 'none' }}
                      />
                    </div>

                    <div style={{ display: 'flex', gap: '6px' }}>
                      <button className={`admin-filter-pill ${patientFilterCategory === 'all' ? 'active' : ''}`} onClick={() => setPatientFilterCategory('all')}>All (6)</button>
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
                        <th>Total Spend (৳)</th>
                        <th>Chronic Condition</th>
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
                          <td><b>{pt.encounters.length * 4}</b></td>
                          <td>{pt.chatActive ? 45 : 18}</td>
                          <td><b style={{ color: '#0F172A', fontSize: '13px' }}>৳ 75,000</b></td>
                          <td>
                            <span className="badge" style={{ background: '#FEE2E2', color: '#DC2626', fontSize: '9.5px', fontWeight: 750 }}>{pt.cohort}</span>
                          </td>
                          <td>{pt.encounters[0]?.date || 'Recent'}<div style={{ fontSize: '10px', color: '#64748B' }}>by {pt.encounters[0]?.doctor || 'Dr. Sabrina'}</div></td>
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

          {/* PAGE 5: BMDC CREDENTIALING */}
          {activeAdminPage === 'bmdc' && (
            <div className="admin-page-view active" id="adminPage_bmdc">
              <div style={{ display: 'grid', gridTemplateColumns: '320px 1fr', gap: '18px' }}>
                <div className="admin-card" style={{ padding: '16px' }}>
                  <div style={{ fontSize: '14px', fontWeight: 800, color: '#0F172A', marginBottom: '12px' }}>Doctor Vetting Queue (3 Pending)</div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                    <div style={{ padding: '12px', background: '#ECFDF5', border: '1px solid #10B981', borderRadius: '12px', cursor: 'pointer' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <strong style={{ fontSize: '12.5px', color: '#065F46' }}>Dr. Sarah Rahman</strong>
                        <span className="badge badge-green">98% Match</span>
                      </div>
                      <div style={{ fontSize: '11px', color: '#047857', marginTop: '2px' }}>BMDC #A-48291 • Internal Medicine</div>
                      <div style={{ fontSize: '10px', color: '#64748B', marginTop: '4px' }}>Applied 2 hrs ago • NID Verified</div>
                    </div>

                    <div style={{ padding: '12px', background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '12px', cursor: 'pointer' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <strong style={{ fontSize: '12.5px', color: '#0F172A' }}>Dr. Ahmed Khan</strong>
                        <span className="badge badge-blue">95% Match</span>
                      </div>
                      <div style={{ fontSize: '11px', color: '#475569', marginTop: '2px' }}>BMDC #A-55102 • Cardiology</div>
                      <div style={{ fontSize: '10px', color: '#64748B', marginTop: '4px' }}>Applied 5 hrs ago • Renewal Check</div>
                    </div>
                  </div>
                </div>

                <div className="admin-card">
                  <div className="admin-card-header">
                    <div>
                      <div className="admin-card-title">In Review: Dr. Sarah Rahman (A-48291)</div>
                      <div style={{ fontSize: '12px', color: '#059669', fontWeight: 750, marginTop: '2px' }}>Govt BMDC Directory Match: 99.1% High Confidence</div>
                    </div>
                    <span className="badge badge-amber">Pending Final Approval</span>
                  </div>

                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px', marginBottom: '18px' }}>
                    <div style={{ background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '12px', padding: '14px' }}>
                      <div style={{ fontSize: '11.5px', fontWeight: 750, color: '#475569', marginBottom: '10px' }}>ORIGINAL UPLOADED SCANS</div>
                      <div style={{ background: '#fff', border: '1px dashed #CBD5E1', padding: '14px', borderRadius: '10px', textAlign: 'center' }}>
                        <div style={{ fontSize: '26px', marginBottom: '4px' }}>📜</div>
                        <strong style={{ fontSize: '12px', color: '#0F172A' }}>BMDC Registration Certificate (A-48291)</strong>
                        <div style={{ fontSize: '10px', color: '#64748B' }}>Issued: 15 Oct 2022 • Bangladesh Medical Council</div>
                      </div>
                      <div style={{ background: '#fff', border: '1px dashed #CBD5E1', padding: '14px', borderRadius: '10px', textAlign: 'center', marginTop: '10px' }}>
                        <div style={{ fontSize: '26px', marginBottom: '4px' }}>🪪</div>
                        <strong style={{ fontSize: '12px', color: '#0F172A' }}>National ID (NID #7849120356)</strong>
                        <div style={{ fontSize: '10px', color: '#64748B' }}>Match Verified with Election Commission</div>
                      </div>
                    </div>

                    <div style={{ background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '12px', padding: '14px' }}>
                      <div style={{ fontSize: '11.5px', fontWeight: 750, color: '#475569', marginBottom: '10px' }}>VERIFIED OCR DIGITAL DATA</div>
                      <div style={{ fontSize: '12px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                        <div><b>Full Name:</b> SARAH RAHMAN</div>
                        <div><b>BMDC Reg Number:</b> A-48291 <span className="badge badge-green" style={{ fontSize: '9.5px' }}>Active</span></div>
                        <div><b>NID Number:</b> 7849120356 (Matched 100%)</div>
                        <div><b>Degrees:</b> MBBS (Dhaka Med. Col., 2013), FCPS (BCPS, 2019)</div>
                        <div><b>Affiliated Hospital:</b> Apollo Hospitals Dhaka</div>
                        <div><b>Specialty:</b> Internal Medicine Consultant</div>
                      </div>
                    </div>
                  </div>

                  <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', borderTop: '1px solid #E2E8F0', paddingTop: '16px' }}>
                    <button className="slot-subtle-btn" style={{ background: '#FEE2E2', color: '#DC2626', border: 'none', padding: '8px 16px', borderRadius: '8px', fontSize: '12px', fontWeight: 750, cursor: 'pointer' }} onClick={() => handleBmdcAction('reject')}>
                      ✕ Reject Application
                    </button>
                    <button className="slot-subtle-btn" style={{ background: '#FEF3C7', color: '#B45309', border: 'none', padding: '8px 16px', borderRadius: '8px', fontSize: '12px', fontWeight: 750, cursor: 'pointer' }} onClick={() => handleBmdcAction('clarify')}>
                      ⚠️ Request Clarification
                    </button>
                    <button className="admin-btn-pri" style={{ padding: '8px 16px', fontSize: '12px' }} onClick={() => handleBmdcAction('approve')}>
                      ✓ Approve &amp; Activate Doctor
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* PAGE 6: CLINICAL PRESCRIPTION & Q&A AUDIT */}
          {activeAdminPage === 'compliance' && (
            <div className="admin-page-view active" id="adminPage_compliance">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <div className="admin-card-title">Clinical e-Prescription Audit &amp; DGDA Compliance</div>
                    <div className="admin-card-subtitle">AI-assisted antimicrobial stewardship &amp; narcotic Schedule H compliance surveillance</div>
                  </div>
                  <span className="badge badge-amber" style={{ fontWeight: 800 }}>1 Flagged for Review</span>
                </div>

                <div style={{ background: '#FEF2F2', border: '1px solid #FECACA', borderRadius: '14px', padding: '18px', marginBottom: '18px' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                      <span style={{ fontSize: '20px' }}>⚠️</span>
                      <div>
                        <strong style={{ color: '#991B1B', fontSize: '13.5px' }}>Schedule H Controlled Substance Alert — Rx #HD-891042</strong>
                        <div style={{ fontSize: '11px', color: '#B91C1C', marginTop: '2px' }}>Issued today by Dr. Sabrina Akter (BMDC #45821) • Patient: Sarah Khan (29F)</div>
                      </div>
                    </div>
                    <span className="badge" style={{ background: '#FEE2E2', color: '#991B1B', fontWeight: 800 }}>High Risk</span>
                  </div>

                  <div style={{ marginTop: '14px', background: '#fff', borderRadius: '10px', padding: '14px', border: '1px solid #FCA5A5', display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px' }}>
                    <div>
                      <div style={{ fontSize: '11px', color: '#64748B', fontWeight: 700 }}>PRESCRIBED MEDICATIONS</div>
                      <div style={{ fontSize: '12px', color: '#0F172A', marginTop: '4px' }}>
                        1. <b>Tab. Azithromycin 500mg</b> — 1 daily x 3 days (Broad spectrum)<br/>
                        2. <b style={{ color: '#DC2626' }}>Inj. Morphine Sulphate 10mg/mL</b> — SOS for severe trauma pain (Schedule H)
                      </div>
                    </div>
                    <div>
                      <div style={{ fontSize: '11px', color: '#64748B', fontWeight: 700 }}>COMPLIANCE AUDIT RATIONALE</div>
                      <div style={{ fontSize: '11.5px', color: '#7F1D1D', marginTop: '4px' }}>
                        Narcotic Schedule H analgesics require physical triage verification under DGDA Telemedicine circular 2024. Audit flag requires clinical ops counter-signature.
                      </div>
                    </div>
                  </div>

                  <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', marginTop: '14px' }}>
                    <button className="slot-subtle-btn" style={{ background: '#FEE2E2', color: '#DC2626', border: 'none', padding: '7px 14px', borderRadius: '8px', fontSize: '11.5px', fontWeight: 750, cursor: 'pointer' }} onClick={() => showToast('Prescription Flagged', 'Doctor notified to revise Schedule H dosage.', '⚠️', 'warning')}>
                      Request Revision from Doctor
                    </button>
                    <button className="admin-btn-pri" onClick={() => showToast('Prescription Approved ✓', 'Archived into Sarah Khan Health Vault with audit signature.', '✓', 'success')}>
                      Counter-Sign &amp; Release Rx ✓
                    </button>
                  </div>
                </div>

                <div style={{ background: '#F8FAFC', border: '1px solid #E2E8F0', borderRadius: '12px', padding: '16px' }}>
                  <div style={{ fontSize: '12px', fontWeight: 800, color: '#0F172A', marginBottom: '10px' }}>Automated DGDA Safety Checks (Passing)</div>
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '10px' }}>
                    <div style={{ padding: '10px', background: '#ECFDF5', border: '1px solid #A7F3D0', borderRadius: '8px', fontSize: '11px', color: '#065F46' }}>
                      <b>DDI Drug-Drug Interactions:</b> Passed ✓ No severe interactions detected.
                    </div>
                    <div style={{ padding: '10px', background: '#EFF6FF', border: '1px solid #BFDBFE', borderRadius: '8px', fontSize: '11px', color: '#1D4ED8' }}>
                      <b>Antibiotic Stewardship:</b> Standard 3-day course validated.
                    </div>
                    <div style={{ padding: '10px', background: '#ECFDF5', border: '1px solid #A7F3D0', borderRadius: '8px', fontSize: '11px', color: '#065F46' }}>
                      <b>BMDC Digital Signature:</b> SHA-256 authenticated against physician key.
                    </div>
                  </div>
                </div>
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
                  <button className="slot-subtle-btn" style={{ background: '#FEF2F2', color: '#DC2626', border: '1px solid #FECACA', padding: '7px 12px', borderRadius: '9px', fontSize: '11.5px', fontWeight: 750, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '5px' }} onClick={() => setSimulateModalOpen(true)}>
                    <span>⚡ Simulate App Failure</span>
                  </button>
                  <button className="slot-subtle-btn" style={{ background: '#F8FAFC', color: '#334155', border: '1px solid #E2E8F0', padding: '7px 12px', borderRadius: '9px', fontSize: '11.5px', fontWeight: 700, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '5px' }} onClick={() => showToast('Logs Exported 📥', `${errorLogs.length} incidents exported as JSON file.`, '📥', 'success')}>
                    <span>📥 Export Logs (.JSON)</span>
                  </button>
                  <button className="admin-btn-pri" onClick={() => showToast('Telemetry Synced 🔄', 'Refreshed latest error streams from mobile clusters.', '🔄', 'info')} style={{ padding: '7px 14px', fontSize: '11.5px' }}>
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
                  <div className="admin-kpi-val" style={{ fontSize: '19px', color: '#BE123C' }}>bKash PGW (38%)</div>
                  <span className="admin-kpi-delta" style={{ color: '#B45309', fontSize: '11px', fontWeight: 700 }}>Callback timeout &gt; 15,000ms</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Impacted User Accounts</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#EFF6FF', color: '#2563EB', display: 'grid', placeItems: 'center' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">12 Users</div>
                  <span className="admin-kpi-delta" style={{ color: '#2563EB', fontSize: '11px', fontWeight: 700 }}>7 Patients • 4 Doctors</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Mean Time to Resolution</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#ECFDF5', color: '#059669', display: 'grid', placeItems: 'center' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#047857' }}>4.2 mins</div>
                  <span className="admin-kpi-delta up">↗ -1.8 mins vs yesterday</span>
                </div>
              </div>

              <div className="admin-card">
                <div style={{ padding: '16px 20px', borderBottom: '1px solid #E2E8F0', background: '#FAFAFA' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '12px', marginBottom: '12px' }}>
                    <div style={{ position: 'relative', flex: 1, minWidth: '280px', maxWidth: '480px' }}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
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
                        <option value="DEGRADED">🔵 Degraded</option>
                      </select>

                      <select value={logSubsystemFilter} onChange={(e) => setLogSubsystemFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', color: '#334155', fontWeight: 600 }}>
                        <option value="ALL">All Subsystems</option>
                        <option value="Payment Gateway">💳 Payment Gateway</option>
                        <option value="Agora WebRTC">📹 Agora WebRTC</option>
                        <option value="DGDA Clinical Engine">💊 DGDA Clinical Rx</option>
                        <option value="Sync & Offline Cache">📶 Sync & Offline</option>
                      </select>

                      <select value={logStatusFilter} onChange={(e) => setLogStatusFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '11.5px', background: '#fff', color: '#334155', fontWeight: 600 }}>
                        <option value="ALL">All Statuses</option>
                        <option value="UNRESOLVED">⚠️ Unresolved</option>
                        <option value="RESOLVED">✓ Resolved</option>
                      </select>
                    </div>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap', fontSize: '11px' }}>
                    <span style={{ color: '#64748B', fontWeight: 700, marginRight: '4px' }}>Quick Filters:</span>
                    <button className={`log-quick-pill ${logQuickFilter === 'all' ? 'active' : ''}`} onClick={() => setLogQuickFilter('all')}>All Logs ({errorLogs.length})</button>
                    <button className={`log-quick-pill ${logQuickFilter === 'critical' ? 'active' : ''}`} onClick={() => setLogQuickFilter('critical')}>🔴 Critical</button>
                    <button className={`log-quick-pill ${logQuickFilter === 'payment' ? 'active' : ''}`} onClick={() => setLogQuickFilter('payment')}>💳 bKash PGW</button>
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
                            <span className={`badge ${log.status === 'RESOLVED' ? 'badge-green' : 'badge-rose'}`}>
                              {log.status === 'RESOLVED' ? '✓ Resolved' : '⚠️ Unresolved'}
                            </span>
                          </td>
                          <td style={{ textAlign: 'right' }}>
                            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '6px', alignItems: 'center' }}>
                              <button className="admin-btn-sec" onClick={(e) => { e.stopPropagation(); setSelectedLogId(log.id); }} style={{ padding: '4px 8px', fontSize: '11px' }}>Inspect 🔍</button>
                              <button className="slot-subtle-btn" onClick={(e) => { e.stopPropagation(); handleToggleLogResolve(log.id); }} style={{ padding: '4px 7px', fontSize: '11px' }}>
                                {log.status === 'RESOLVED' ? '↩' : '✓'}
                              </button>
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
                  <span className="admin-kpi-val" style={{ color: '#DC2626' }}>2</span>
                  <span className="admin-kpi-delta down">⚠️ Conduct / Rushed Call</span>
                </div>
                <div className="admin-kpi-card">
                  <span className="admin-kpi-sub">Claims Against System</span>
                  <span className="admin-kpi-val" style={{ color: '#2563EB' }}>2</span>
                  <span className="admin-kpi-delta neutral">⚙️ PGW / ICE Glitches</span>
                </div>
                <div className="admin-kpi-card">
                  <span className="admin-kpi-sub">Resolution &amp; Refund Rate</span>
                  <span className="admin-kpi-val" style={{ color: '#059669' }}>100%</span>
                  <span className="admin-kpi-delta up">⚡ Avg. Adjudication &lt; 28 mins</span>
                </div>
              </div>

              <div className="admin-card">
                <div style={{ padding: '16px 20px', borderBottom: '1px solid #E2E8F0', background: '#FAFAFA' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '12px', marginBottom: '12px' }}>
                    <div style={{ position: 'relative', flex: 1, minWidth: '280px', maxWidth: '480px' }}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" strokeWidth="2" style={{ position: 'absolute', left: '12px', top: '50%', transform: 'translateY(-50%)' }}><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                      <input
                        type="text"
                        placeholder="Filter by Patient, Doctor, BMDC #, Dispute ID, Category..."
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
                            <div style={{ fontSize: '10.5px', color: '#64748B' }}>{grv.doctor.bmdc}</div>
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
                  <button className="admin-action-btn" onClick={() => showToast('MFS Webhooks Reconciled', '0 discrepancies detected across bKash and Nagad payment gateways.', '✓', 'success')} style={{ background: '#EFF6FF', border: '1px solid #BFDBFE', color: '#1D4ED8', fontWeight: 700, fontSize: '12px', padding: '8px 14px', borderRadius: '8px', display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
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
                  <div className="admin-kpi-val">৳ 3,245,000</div>
                  <span className="admin-kpi-delta up">↗ +18.4% MoM • 1,864 Transactions</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Active Escrow Locked</span>
                    <div className="admin-kpi-icon-circle blue" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#EFF6FF', color: '#2563EB' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#2563EB' }}>৳ 890,800</div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>Held securely pending doctor sign-off</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">HeloDoc 20% Commission</span>
                    <div className="admin-kpi-icon-circle green" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#ECFDF5', color: '#059669' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val" style={{ color: '#059669' }}>৳ 568,400</div>
                  <span className="admin-kpi-delta up">↗ Net Platform Operating Revenue</span>
                </div>

                <div className="admin-kpi-card">
                  <div className="kpi-top-row">
                    <span className="admin-kpi-sub">Total Disbursed Outflows</span>
                    <div className="admin-kpi-icon-circle purple" style={{ width: '34px', height: '34px', borderRadius: '10px', background: '#F5F3FF', color: '#7C3AED' }}>
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><polyline points="13 17 18 12 13 7"/><polyline points="6 17 11 12 6 7"/></svg>
                    </div>
                  </div>
                  <div className="admin-kpi-val">৳ 2,676,600</div>
                  <span className="admin-kpi-delta" style={{ color: '#64748B' }}>Doctor Wallets + Disputed Refunds</span>
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
                        placeholder="Search TxID, Patient, Doctor, Ref..."
                        value={financeSearchQuery}
                        onChange={(e) => setFinanceSearchQuery(e.target.value)}
                        style={{ width: '100%', padding: '7px 10px 7px 32px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', outline: 'none', background: '#FFFFFF' }}
                      />
                    </div>

                    <select value={financeGatewayFilter} onChange={(e) => setFinanceGatewayFilter(e.target.value)} style={{ padding: '7px 10px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#FFFFFF', color: '#334155', outline: 'none' }}>
                      <option value="ALL">All Gateways</option>
                      <option value="bKash">bKash Merchant Pay</option>
                      <option value="Nagad">Nagad Business Pay</option>
                      <option value="Card">Visa / Mastercard (SSLCommerz)</option>
                      <option value="MFS Bulk API">MFS Bulk Payout API</option>
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
                        <th style={{ textAlign: 'right' }}>Gross (৳)</th>
                        <th style={{ textAlign: 'right' }}>HeloDoc Cut</th>
                        <th style={{ textAlign: 'right' }}>Net (৳)</th>
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
                            <span className={`payout-method-badge ${tx.gateway === 'bKash' ? 'bkash' : tx.gateway === 'Nagad' ? 'nagad' : 'bank'}`}>
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
                            <div style={{ fontSize: '10px', color: '#64748B' }}>{tx.patient.phone} • {tx.doctor.bmdc}</div>
                          </td>
                          <td style={{ textAlign: 'right', fontWeight: 800, color: '#0F172A', fontSize: '13px' }}>৳ {tx.grossAmount.toLocaleString()}</td>
                          <td style={{ textAlign: 'right', fontSize: '12px', color: '#059669', fontWeight: 700 }}>৳ {tx.platformFeeAmount.toLocaleString()} <span style={{ fontSize: '9.5px', color: '#64748B' }}>({tx.platformFeePercent}%)</span></td>
                          <td style={{ textAlign: 'right', fontWeight: 850, color: '#2563EB', fontSize: '13.5px' }}>৳ {tx.netAmount.toLocaleString()}</td>
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
                    <span className="badge badge-green">{selectedDocObj.bmdc}</span>
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
                    <th>Fee (৳)</th>
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
                        <b>{dsb.date}: {dsb.amount}</b> Disbursed via <span className="badge" style={{ background: '#FCE7F3', color: '#DB2777', fontWeight: 800 }}>{dsb.method}</span>
                        <div style={{ fontSize: '10px', color: '#64748B' }}>TxID: {dsb.txId} • Commission Deducted: {dsb.commission}</div>
                      </div>
                      <span className="badge badge-green">{dsb.status}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontSize: '11px', color: '#64748B' }}>BMDC Verified Credential Ledger • HelloDoctor Bangladesh</span>
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
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Latest Blood Pressure</div>
                <div style={{ fontSize: '16px', fontWeight: 800, color: '#DC2626', marginTop: '2px' }}>{selectedPatObj.bp}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Resting Pulse</div>
                <div style={{ fontSize: '16px', fontWeight: 800, color: '#0F172A', marginTop: '2px' }}>{selectedPatObj.pulse}</div>
              </div>
              <div style={{ background: '#fff', padding: '12px 16px', textAlign: 'center' }}>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Chronic Care Cohort</div>
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
                    <div style={{ fontSize: '10.5px', color: '#881337', fontWeight: 700 }}>FAILED SUBSYSTEM</div>
                    <div style={{ fontSize: '13px', fontWeight: 800, color: '#9F1239', marginTop: '2px' }}>{selectedLogObj.subsystem}</div>
                  </div>
                  <div>
                    <div style={{ fontSize: '10.5px', color: '#881337', fontWeight: 700 }}>COMPONENT / ENDPOINT</div>
                    <div style={{ fontSize: '12px', fontFamily: 'monospace', fontWeight: 700, color: '#9F1239', marginTop: '2px' }}>{selectedLogObj.component}</div>
                  </div>
                </div>
                <div style={{ background: '#fff', border: '1px solid #FDA4AF', borderRadius: '10px', padding: '12px' }}>
                  <div style={{ fontSize: '10.5px', color: '#64748B', fontWeight: 700 }}>CLINICAL &amp; SYSTEM ERROR SUMMARY</div>
                  <div style={{ fontSize: '12px', color: '#9F1239', marginTop: '2px', lineHeight: 1.45 }}>{selectedLogObj.errorMessage}</div>
                </div>
              </div>

              <div style={{ background: '#0F172A', borderRadius: '14px', padding: '16px', color: '#E2E8F0' }}>
                <pre style={{ margin: 0, fontFamily: 'monospace', fontSize: '11px', lineHeight: 1.55, color: '#38BDF8', whiteSpace: 'pre-wrap', wordBreak: 'break-all', maxHeight: '180px', overflowY: 'auto' }}>
                  {selectedLogObj.stackTrace}
                </pre>
              </div>
            </div>

            <div style={{ padding: '14px 22px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <button className="admin-btn-sec" onClick={() => setSelectedLogId(null)}>Close Dossier</button>
              <div style={{ display: 'flex', gap: '8px' }}>
                <button className="slot-subtle-btn" style={{ background: '#EFF6FF', color: '#1D4ED8', border: '1px solid #BFDBFE', padding: '8px 14px', borderRadius: '8px', fontSize: '11.5px', fontWeight: 750, cursor: 'pointer' }} onClick={() => showToast('Handshake Replayed ✓', `Upstream gateway accepted retry for ${selectedLogObj.user.name}.`, '✓', 'success')}>
                  Retry Webhook / Handshake 🔁
                </button>
                <button className="admin-btn-pri" onClick={() => handleToggleLogResolve(selectedLogObj.id)} style={{ padding: '8px 16px', fontSize: '11.5px' }}>
                  {selectedLogObj.status === 'RESOLVED' ? 'Re-open Incident ↩' : 'Mark as Resolved ✓'}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ── MODAL 4: SIMULATE FAILURE MODAL ── */}
      {simulateModalOpen && (
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
              <div>
                <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>Target Impacted User</label>
                <select value={simUser} onChange={(e) => setSimUser(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#fff', outline: 'none' }}>
                  <option value="sarah">Sarah Khan (Patient • +880 1711-234567)</option>
                  <option value="sabrina">Dr. Sabrina Akter (Doctor Web • BMDC #45821)</option>
                  <option value="rafiq">Rafiq Ahmed (Patient • +880 1819-987654)</option>
                  <option value="anika">Dr. Anika Rahman (Doctor Mobile • BMDC #A-74921)</option>
                </select>
              </div>

              <div>
                <label style={{ fontSize: '11.5px', fontWeight: 750, color: '#334155', display: 'block', marginBottom: '5px' }}>Failure Scenario Preset</label>
                <select value={simScenario} onChange={(e) => setSimScenario(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', background: '#fff', outline: 'none' }}>
                  <option value="bkash_timeout">💳 bKash PGW: HTTP 504 Gateway Callback Timeout</option>
                  <option value="webrtc_ice">📹 Agora WebRTC: ICE Candidate Relay Failed (Code 702)</option>
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
                  <input type="text" placeholder="e.g. Stress test injection" value={simCustomNote} onChange={(e) => setSimCustomNote(e.target.value)} style={{ width: '100%', padding: '9px 12px', border: '1px solid #CBD5E1', borderRadius: '8px', fontSize: '12px', outline: 'none' }} />
                </div>
              </div>
            </div>

            <div style={{ padding: '14px 20px', background: '#F8FAFC', borderTop: '1px solid #E2E8F0', display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button className="admin-btn-sec" onClick={() => setSimulateModalOpen(false)}>Cancel</button>
              <button className="admin-btn-pri" onClick={handleExecuteSimulateFailure} style={{ background: '#DC2626', borderColor: '#DC2626' }}>
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
                  <div style={{ fontSize: '11.5px', color: '#64748B', marginTop: '2px' }}>{selectedGrvObj.doctor.bmdc} • Fee: {selectedGrvObj.doctor.fee}</div>
                </div>
              </div>

              <div style={{ border: '1.5px solid #FCA5A5', borderRadius: '12px', background: '#FFF5F5', padding: '14px 16px' }}>
                <div style={{ fontSize: '12.5px', fontWeight: 800, color: '#991B1B' }}>{selectedGrvObj.category}</div>
                <div style={{ fontSize: '12px', lineHeight: 1.5, color: '#450A0A', background: '#FFFFFF', border: '1px solid #FECACA', borderRadius: '8px', padding: '10px 12px', fontStyle: 'italic', marginTop: '6px' }}>
                  "{selectedGrvObj.patientStatement}"
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
                <button className="admin-btn-pri" onClick={() => handleAdjudicateGrievance(selectedGrvObj.id, 'REFUND')} style={{ background: '#059669', borderColor: '#059669' }}>
                  <span>💳 Disburse Instant bKash Refund</span>
                </button>
                <button className="admin-btn-pri" onClick={() => handleAdjudicateGrievance(selectedGrvObj.id, 'WARN')} style={{ background: '#DC2626', borderColor: '#DC2626' }}>
                  <span>⚠️ Issue Doctor BMDC Warning</span>
                </button>
                <button className="admin-btn-sec" onClick={() => handleAdjudicateGrievance(selectedGrvObj.id, 'RESOLVE')}>
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
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#0F172A', marginTop: '4px' }}>৳ {selectedTxObj.grossAmount}</div>
                <div style={{ fontSize: '10.5px', color: '#10B981', fontWeight: 700 }}>Debited from MFS Wallet</div>
              </div>
              <div style={{ background: '#FFFFFF', padding: '14px 18px' }}>
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>HeloDoc Platform Fee (20%)</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#059669', marginTop: '4px' }}>৳ {selectedTxObj.platformFeeAmount} ({selectedTxObj.platformFeePercent}%)</div>
                <div style={{ fontSize: '10.5px', color: '#64748B' }}>Platform Commission Cut</div>
              </div>
              <div style={{ background: '#FFFFFF', padding: '14px 18px' }}>
                <div style={{ fontSize: '10px', textTransform: 'uppercase', fontWeight: 800, color: '#64748B' }}>Doctor Net / Refund</div>
                <div style={{ fontSize: '20px', fontWeight: 900, color: '#2563EB', marginTop: '4px' }}>৳ {selectedTxObj.netAmount}</div>
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
                  <div style={{ fontSize: '11px', color: '#64748B' }}>{selectedTxObj.doctor.bmdc} • {selectedTxObj.doctor.specialty}</div>
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
