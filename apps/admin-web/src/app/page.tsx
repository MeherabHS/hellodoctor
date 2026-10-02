'use client';

import React, { useState, useEffect, useCallback } from 'react';
import * as api from '../lib/api';

export default function AdminPortalPage() {
  // Navigation State
  const [activeTab, setActiveTab] = useState<string>('doctors');
  const [searchQuery, setSearchQuery] = useState<string>('');

  // Live Backend Data States
  const [loading, setLoading] = useState<boolean>(true);
  const [backendError, setBackendError] = useState<string | null>(null);

  const [overview, setOverview] = useState<api.AdminOverview>({
    total_doctors: 0,
    on_duty_doctors: 0,
    pending_bmdc: 0,
    active_consultations: 0,
    total_patients: 0,
    gross_volume: '0',
    platform_fees: '0',
    pending_disbursement: '0',
    pending_grievances: 0,
    unresolved_traces: 0,
    system_health: 'OPTIMAL',
  });

  const [doctors, setDoctors] = useState<api.AdminDoctor[]>([]);
  const [transactions, setTransactions] = useState<api.AdminTransaction[]>([]);
  const [financeSummary, setFinanceSummary] = useState<api.FinanceSummary>({
    total_gross: '0',
    total_platform_fee: '0',
    total_net: '0',
    pending_disbursement: '0',
    gateway_volume: { bkash: '0', nagad: '0', card: '0' },
  });
  const [commandTelemetry, setCommandTelemetry] = useState<api.CommandTelemetry>({
    active_specialists: 0,
    completed_today: 0,
    active_sessions_count: 0,
    active_streams: [],
  });
  const [slots, setSlots] = useState<api.AdminSlot[]>([]);
  const [patients, setPatients] = useState<api.AdminPatient[]>([]);
  const [bmdcQueue, setBmdcQueue] = useState<api.BmdcDoctor[]>([]);
  const [complianceAlerts, setComplianceAlerts] = useState<api.ComplianceAlert[]>([]);
  const [logs, setLogs] = useState<api.AdminLog[]>([]);
  const [grievances, setGrievances] = useState<api.AdminGrievance[]>([]);

  // Modals & Selected Entities
  const [selectedDoctorDossier, setSelectedDoctorDossier] = useState<api.DoctorDossier | null>(null);
  const [selectedPatientDossier, setSelectedPatientDossier] = useState<api.PatientDossier | null>(null);
  const [selectedLog, setSelectedLog] = useState<api.AdminLog | null>(null);
  const [selectedTransaction, setSelectedTransaction] = useState<api.AdminTransaction | null>(null);
  const [selectedGrievance, setSelectedGrievance] = useState<api.AdminGrievance | null>(null);

  // Form Modals
  const [showSimulateModal, setShowSimulateModal] = useState<boolean>(false);
  const [showOnboardDoctorModal, setShowOnboardDoctorModal] = useState<boolean>(false);
  const [showAddSlotModal, setShowAddSlotModal] = useState<boolean>(false);

  // Form States
  const [newDoctorForm, setNewDoctorForm] = useState({
    full_name: '',
    license_number: '',
    primary_specialty: 'Internal Medicine',
    experience_years: 5,
    current_hospital: '',
    verified_phone: '+880 1',
    consultation_fee_video: 800,
    consultation_fee_chat: 500,
    residential_address: 'Dhaka, Bangladesh',
  });

  const [newSlotForm, setNewSlotForm] = useState({
    doctor_id: '',
    start_time: '',
    end_time: '',
  });

  const [simForm, setSimForm] = useState({
    subsystem: 'Payment Clearing Subsystem (bKash Gateway)',
    action: 'BKASH_SETTLEMENT_TIMEOUT',
    severity: 'ERROR',
    error_summary: 'Gateway handshake timeout after 15,000ms. Re-queueing retry block.',
  });

  // Toasts
  const [toasts, setToasts] = useState<{ id: string; text: string; icon?: string }[]>([]);

  const showToast = (text: string, icon: string = 'ℹ️') => {
    const id = Math.random().toString(36).substring(2, 9);
    setToasts((prev) => [...prev, { id, text, icon }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  };

  // Fetch all live backend data
  const loadBackendData = useCallback(async () => {
    try {
      setLoading(true);
      setBackendError(null);

      const [
        overviewRes,
        docsRes,
        txsRes,
        finRes,
        cmdRes,
        slotsRes,
        ptsRes,
        bmdcRes,
        compRes,
        logsRes,
        grievRes,
      ] = await Promise.allSettled([
        api.getAdminOverview(),
        api.getAdminDoctors(),
        api.getAdminTransactions(),
        api.getFinanceSummary(),
        api.getCommandTelemetry(),
        api.getAdminSlots(),
        api.getAdminPatients(),
        api.getBmdcQueue(),
        api.getComplianceAlerts(),
        api.getAdminLogs(),
        api.getAdminGrievances(),
      ]);

      if (overviewRes.status === 'fulfilled') setOverview(overviewRes.value);
      if (docsRes.status === 'fulfilled') setDoctors(docsRes.value);
      if (txsRes.status === 'fulfilled') setTransactions(txsRes.value);
      if (finRes.status === 'fulfilled') setFinanceSummary(finRes.value);
      if (cmdRes.status === 'fulfilled') setCommandTelemetry(cmdRes.value);
      if (slotsRes.status === 'fulfilled') setSlots(slotsRes.value);
      if (ptsRes.status === 'fulfilled') setPatients(ptsRes.value);
      if (bmdcRes.status === 'fulfilled') setBmdcQueue(bmdcRes.value);
      if (compRes.status === 'fulfilled') setComplianceAlerts(compRes.value);
      if (logsRes.status === 'fulfilled') setLogs(logsRes.value);
      if (grievRes.status === 'fulfilled') setGrievances(grievRes.value);
    } catch (err: any) {
      console.error('Failed to load data from backend:', err);
      setBackendError(err.message || 'Unable to connect to HelloDoctor backend');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadBackendData();
    const interval = setInterval(loadBackendData, 15000); // 15s polling
    return () => clearInterval(interval);
  }, [loadBackendData]);

  // Actions
  const handleOpenDoctorDossier = async (docId: string) => {
    try {
      showToast('Fetching physician clinical dossier from backend...', '🩺');
      const dossier = await api.getDoctorDossier(docId);
      setSelectedDoctorDossier(dossier);
    } catch (err: any) {
      showToast(`Error fetching dossier: ${err.message}`, '❌');
    }
  };

  const handleOpenPatientDossier = async (ptId: string) => {
    try {
      showToast('Fetching patient clinical records from backend...', '👤');
      const dossier = await api.getPatientDossier(ptId);
      setSelectedPatientDossier(dossier);
    } catch (err: any) {
      showToast(`Error fetching patient records: ${err.message}`, '❌');
    }
  };

  const handleBatchDisbursement = async () => {
    try {
      showToast('Initiating 20% platform fee monthly disbursement batch...', '💳');
      const now = new Date();
      const start = new Date(now.getFullYear(), now.getMonth(), 1).toISOString().split('T')[0];
      const end = now.toISOString().split('T')[0];
      await api.initiateDisbursement(start, end);
      showToast('Disbursement batch executed successfully via backend clearing!', '✅');
      loadBackendData();
    } catch (err: any) {
      showToast(`Disbursement error: ${err.message}`, '⚠️');
    }
  };

  const handleCreateDoctor = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      showToast('Registering new physician in backend...', '🩺');
      await api.createDoctor({
        full_name: newDoctorForm.full_name,
        license_number: newDoctorForm.license_number,
        primary_specialty: newDoctorForm.primary_specialty,
        experience_years: Number(newDoctorForm.experience_years),
        current_hospital: newDoctorForm.current_hospital,
        verified_phone: newDoctorForm.verified_phone,
        consultation_fee_video: Number(newDoctorForm.consultation_fee_video),
        consultation_fee_chat: Number(newDoctorForm.consultation_fee_chat),
        residential_address: newDoctorForm.residential_address,
      });
      showToast('Physician onboarded successfully!', '✅');
      setShowOnboardDoctorModal(false);
      setNewDoctorForm({
        full_name: '',
        license_number: '',
        primary_specialty: 'Internal Medicine',
        experience_years: 5,
        current_hospital: '',
        verified_phone: '+880 1',
        consultation_fee_video: 800,
        consultation_fee_chat: 500,
        residential_address: 'Dhaka, Bangladesh',
      });
      loadBackendData();
    } catch (err: any) {
      showToast(`Failed to register physician: ${err.message}`, '❌');
    }
  };

  const handleToggleSlot = async (slotId: string) => {
    try {
      await api.toggleSlot(slotId);
      showToast('Slot status updated in backend', '⚡');
      loadBackendData();
    } catch (err: any) {
      showToast(`Failed to toggle slot: ${err.message}`, '❌');
    }
  };

  const handleCreateSlot = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      if (!newSlotForm.doctor_id) {
        showToast('Please select a doctor for this slot', '⚠️');
        return;
      }
      const start = new Date(newSlotForm.start_time).toISOString();
      const end = new Date(newSlotForm.end_time).toISOString();
      await api.createSlot(newSlotForm.doctor_id, start, end);
      showToast('Micro-shift slot registered in backend', '✅');
      setShowAddSlotModal(false);
      loadBackendData();
    } catch (err: any) {
      showToast(`Failed to create slot: ${err.message}`, '❌');
    }
  };

  const handleVerifyBmdc = async (id: string) => {
    try {
      await api.verifyBmdcDoctor(id, 'Verified by administrative governance board');
      showToast('Doctor BMDC credentials verified and activated!', '✅');
      loadBackendData();
    } catch (err: any) {
      showToast(`Failed to verify: ${err.message}`, '❌');
    }
  };

  const handleRejectBmdc = async (id: string) => {
    try {
      await api.rejectBmdcDoctor(id, 'Credentials could not be reconciled with BMDC registry');
      showToast('Doctor BMDC application rejected', '⚠️');
      loadBackendData();
    } catch (err: any) {
      showToast(`Failed to reject: ${err.message}`, '❌');
    }
  };

  const handleSimulateLog = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await api.simulateAdminLog({
        subsystem: simForm.subsystem,
        action: simForm.action,
        severity: simForm.severity,
        error_summary: simForm.error_summary,
      });
      showToast('Diagnostic failure trace injected into backend audit log', '🚨');
      setShowSimulateModal(false);
      loadBackendData();
    } catch (err: any) {
      showToast(`Failed to simulate log: ${err.message}`, '❌');
    }
  };

  const handleAdjudicateRefund = async (grievanceId: string) => {
    try {
      await api.adjudicateGrievanceRefund(
        grievanceId,
        'Direct patient MFS refund authorized by admin board'
      );
      showToast('Full refund approved & disbursed to patient MFS account', '💰');
      setSelectedGrievance(null);
      loadBackendData();
    } catch (err: any) {
      showToast(`Refund adjudication error: ${err.message}`, '❌');
    }
  };

  const handleAdjudicateWarn = async (grievanceId: string) => {
    try {
      await api.adjudicateGrievanceWarn(
        grievanceId,
        'Official clinical warning recorded in physician credential history'
      );
      showToast('Formal compliance warning issued to physician dossier', '⚠️');
      setSelectedGrievance(null);
      loadBackendData();
    } catch (err: any) {
      showToast(`Warning adjudication error: ${err.message}`, '❌');
    }
  };

  // Nav Title Helper
  const getHeaderTitle = () => {
    switch (activeTab) {
      case 'doctors':
        return 'Physician Directory & Earnings';
      case 'finance':
        return 'Financial Clearing & Settlement';
      case 'command':
        return 'Command & Telemetry';
      case 'slots':
        return 'Micro-Shift Timetable Matrix';
      case 'patients':
        return 'Patient Registry & Clinical Vault';
      case 'bmdc':
        return 'BMDC Credential Vetting Queue';
      case 'compliance':
        return 'Narcotics & Clinical Governance';
      case 'logs':
        return 'System Health & Diagnostics';
      case 'grievances':
        return 'Patient Grievances & Adjudication';
      default:
        return 'HelloDoctor Administration';
    }
  };

  return (
    <div className="admin-portal-shell">
      {/* Toast Notification Container */}
      <div id="adminToastContainer">
        {toasts.map((t) => (
          <div key={t.id} className="admin-toast">
            <span style={{ fontSize: '1.1rem' }}>{t.icon}</span>
            <span>{t.text}</span>
          </div>
        ))}
      </div>

      {/* Backend Status Alert (If Offline) */}
      {backendError && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            zIndex: 9999,
            background: '#ef4444',
            color: '#fff',
            padding: '8px 16px',
            textAlign: 'center',
            fontSize: '0.85rem',
            fontWeight: 600,
          }}
        >
          ⚠️ Backend Connection Error: {backendError}. Ensure `hellodoctor-backend` is running on http://localhost:8080
        </div>
      )}

      {/* SIDEBAR */}
      <aside className="admin-sidebar">
        <div className="admin-brand">
          <div className="admin-brand-icon">HD</div>
          <div className="admin-brand-text">
            <h2>HelloDoctor</h2>
            <span>ADMIN ENGINE v2.4</span>
          </div>
        </div>

        <nav className="admin-nav" id="adminSidebarNav">
          <div className="admin-nav-section-label">CLINICAL & EARNINGS</div>
          <button
            className={`admin-nav-item ${activeTab === 'doctors' ? 'active' : ''}`}
            onClick={() => setActiveTab('doctors')}
          >
            <span className="admin-nav-icon">🩺</span>
            <span className="admin-nav-label">Doctors & Wallets</span>
            <span className="admin-nav-badge">{doctors.length}</span>
          </button>
          <button
            className={`admin-nav-item ${activeTab === 'finance' ? 'active' : ''}`}
            onClick={() => setActiveTab('finance')}
          >
            <span className="admin-nav-icon">💳</span>
            <span className="admin-nav-label">Finance Clearing</span>
            <span className="admin-nav-badge">20% Fee</span>
          </button>

          <div className="admin-nav-section-label">TELEMETRY & OPERATIONS</div>
          <button
            className={`admin-nav-item ${activeTab === 'command' ? 'active' : ''}`}
            onClick={() => setActiveTab('command')}
          >
            <span className="admin-nav-icon">📡</span>
            <span className="admin-nav-label">Command Center</span>
            <span
              className="admin-nav-badge"
              style={{
                background: commandTelemetry.active_sessions_count > 0 ? '#10b981' : '#64748b',
                color: '#fff',
              }}
            >
              {commandTelemetry.active_sessions_count} Live
            </span>
          </button>
          <button
            className={`admin-nav-item ${activeTab === 'slots' ? 'active' : ''}`}
            onClick={() => setActiveTab('slots')}
          >
            <span className="admin-nav-icon">🗓️</span>
            <span className="admin-nav-label">10m Slot Matrix</span>
            <span className="admin-nav-badge">{slots.length}</span>
          </button>
          <button
            className={`admin-nav-item ${activeTab === 'patients' ? 'active' : ''}`}
            onClick={() => setActiveTab('patients')}
          >
            <span className="admin-nav-icon">👤</span>
            <span className="admin-nav-label">Patient Registry</span>
            <span className="admin-nav-badge">{patients.length}</span>
          </button>

          <div className="admin-nav-section-label">REGULATORY & GOVERNANCE</div>
          <button
            className={`admin-nav-item ${activeTab === 'bmdc' ? 'active' : ''}`}
            onClick={() => setActiveTab('bmdc')}
          >
            <span className="admin-nav-icon">🛡️</span>
            <span className="admin-nav-label">BMDC Vetting Queue</span>
            {bmdcQueue.length > 0 && (
              <span className="admin-nav-badge" style={{ background: '#f59e0b', color: '#fff' }}>
                {bmdcQueue.length}
              </span>
            )}
          </button>
          <button
            className={`admin-nav-item ${activeTab === 'compliance' ? 'active' : ''}`}
            onClick={() => setActiveTab('compliance')}
          >
            <span className="admin-nav-icon">⚖️</span>
            <span className="admin-nav-label">DGDA & Narcotics</span>
            {complianceAlerts.length > 0 && (
              <span className="admin-nav-badge" style={{ background: '#ef4444', color: '#fff' }}>
                {complianceAlerts.length}
              </span>
            )}
          </button>

          <div className="admin-nav-section-label">SYSTEM & DISPUTES</div>
          <button
            className={`admin-nav-item ${activeTab === 'logs' ? 'active' : ''}`}
            onClick={() => setActiveTab('logs')}
          >
            <span className="admin-nav-icon">📋</span>
            <span className="admin-nav-label">Exception Traces</span>
            <span className="admin-nav-badge">{logs.length}</span>
          </button>
          <button
            className={`admin-nav-item ${activeTab === 'grievances' ? 'active' : ''}`}
            onClick={() => setActiveTab('grievances')}
          >
            <span className="admin-nav-icon">🚨</span>
            <span className="admin-nav-label">Dispute Board</span>
            {grievances.length > 0 && (
              <span className="admin-nav-badge" style={{ background: '#ef4444', color: '#fff' }}>
                {grievances.length}
              </span>
            )}
          </button>
        </nav>

        <div className="admin-sidebar-footer">
          <div className="admin-footer-status">
            <span
              className="admin-pulse-dot"
              style={{
                background: overview.system_health === 'OPTIMAL' ? '#10b981' : '#ef4444',
              }}
            ></span>
            <span>DGHS Telemedicine Uplink: {overview.system_health}</span>
          </div>
          <div
            style={{
              fontSize: '0.65rem',
              color: 'var(--text-muted)',
              marginTop: '4px',
              fontFamily: 'var(--font-mono)',
            }}
          >
            Backend: http://localhost:8080/api/v1
          </div>
        </div>
      </aside>

      {/* MAIN CONTENT AREA */}
      <main className="admin-main">
        {/* TOPBAR */}
        <header className="admin-topbar">
          <div className="admin-topbar-left">
            <h1 id="adminPageHeaderTitle">{getHeaderTitle()}</h1>
            <span className="admin-breadcrumb" id="adminBreadcrumb">
              HelloDoctor / {getHeaderTitle()}
            </span>
          </div>

          <div className="admin-topbar-right">
            <div className="admin-search-box">
              <span className="admin-search-icon">🔍</span>
              <input
                type="text"
                id="adminGlobalSearchInput"
                placeholder="Search BMDC, TXN, Phone, Trace..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>

            <button
              className="admin-helpline-pill"
              onClick={() => showToast('Connecting to 16263 National Health Helpline...', '📞')}
            >
              <span className="admin-helpline-icon">🚨</span>
              <span>16263 Helpline Bridge</span>
            </button>

            <button
              className="admin-icon-btn"
              title="Refresh Data from Backend"
              onClick={() => {
                showToast('Synchronizing with backend state...', '🔄');
                loadBackendData();
              }}
            >
              🔄
            </button>

            <div className="admin-user-pill">
              <div className="admin-user-avatar">AR</div>
              <div className="admin-user-info">
                <span className="admin-user-name">Amina Rashid</span>
                <span className="admin-user-role">Platform Admin</span>
              </div>
            </div>
          </div>
        </header>

        {/* VIEW CONTAINER */}
        <div className="admin-content-view">
          {/* ============================================================== */}
          {/* TAB 1: DOCTORS & EARNINGS */}
          {/* ============================================================== */}
          {activeTab === 'doctors' && (
            <div id="adminPage_doctors" className="admin-page-section active">
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">REGISTERED PHYSICIANS</span>
                    <span className="admin-kpi-trend trend-neutral">Verified</span>
                  </div>
                  <div className="admin-kpi-value">{overview.total_doctors}</div>
                  <div className="admin-kpi-subtitle">
                    {overview.on_duty_doctors} currently on-duty
                  </div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">ACTIVE CONSULTATIONS</span>
                    <span className="admin-kpi-trend trend-positive">Agora WebRTC</span>
                  </div>
                  <div className="admin-kpi-value">{overview.active_consultations}</div>
                  <div className="admin-kpi-subtitle">Live telemedicine sessions</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">PLATFORM REVENUE (20%)</span>
                    <span className="admin-kpi-trend trend-positive">Fixed 20%</span>
                  </div>
                  <div className="admin-kpi-value">৳ {Number(overview.platform_fees).toLocaleString()}</div>
                  <div className="admin-kpi-subtitle">Withheld platform fee</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">PENDING DISBURSEMENTS</span>
                    <span className="admin-kpi-trend trend-warning">Monthly Batch</span>
                  </div>
                  <div className="admin-kpi-value">
                    ৳ {Number(overview.pending_disbursement).toLocaleString()}
                  </div>
                  <div className="admin-kpi-subtitle">Accrued net doctor balance</div>
                </div>
              </div>

              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">Physician Directory & Payout Ledgers</h3>
                    <p className="admin-card-subtitle">
                      Live doctor profiles fetched directly from backend database.
                    </p>
                  </div>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    <button
                      className="admin-btn admin-btn-outline"
                      onClick={() => setShowOnboardDoctorModal(true)}
                    >
                      + Onboard Physician
                    </button>
                    <button
                      className="admin-btn admin-btn-primary"
                      onClick={handleBatchDisbursement}
                    >
                      ⚡ Execute Monthly Payout
                    </button>
                  </div>
                </div>

                <div className="admin-table-container">
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Doctor</th>
                        <th>BMDC License</th>
                        <th>Specialty & Hospital</th>
                        <th>Duty Status</th>
                        <th>Accrued Net</th>
                        <th>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {doctors.length === 0 ? (
                        <tr>
                          <td
                            colSpan={6}
                            style={{
                              textAlign: 'center',
                              padding: '48px 24px',
                              color: 'var(--text-muted)',
                            }}
                          >
                            <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>🩺</div>
                            <div
                              style={{
                                fontWeight: 700,
                                fontSize: '1.1rem',
                                color: 'var(--text-primary)',
                              }}
                            >
                              No Registered Physicians in Backend
                            </div>
                            <div style={{ fontSize: '0.85rem', marginTop: '6px' }}>
                              The backend database currently has zero doctor records. Click below to onboard a physician.
                            </div>
                            <button
                              className="admin-btn admin-btn-primary"
                              style={{ marginTop: '16px' }}
                              onClick={() => setShowOnboardDoctorModal(true)}
                            >
                              + Onboard First Physician
                            </button>
                          </td>
                        </tr>
                      ) : (
                        doctors
                          .filter(
                            (d) =>
                              d.full_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
                              d.license_number.toLowerCase().includes(searchQuery.toLowerCase()) ||
                              d.primary_specialty.toLowerCase().includes(searchQuery.toLowerCase())
                          )
                          .map((doc) => (
                            <tr
                              key={doc.id}
                              style={{ cursor: 'pointer' }}
                              onClick={() => handleOpenDoctorDossier(doc.id)}
                            >
                              <td>
                                <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                                  <div
                                    className="admin-user-avatar"
                                    style={{ background: '#059669', color: '#fff' }}
                                  >
                                    {doc.full_name.slice(3, 5).toUpperCase()}
                                  </div>
                                  <div>
                                    <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>
                                      {doc.full_name}
                                    </div>
                                    <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                                      {doc.verified_phone}
                                    </div>
                                  </div>
                                </div>
                              </td>
                              <td>
                                <span className="admin-badge badge-blue">{doc.license_number}</span>
                              </td>
                              <td>
                                <div>{doc.primary_specialty}</div>
                                <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                                  {doc.current_hospital}
                                </div>
                              </td>
                              <td>
                                <span
                                  className={`admin-badge ${
                                    doc.is_on_duty ? 'badge-green' : 'badge-slate'
                                  }`}
                                >
                                  {doc.is_on_duty ? '● On Duty' : '○ Off Duty'}
                                </span>
                              </td>
                              <td style={{ fontWeight: 600 }}>
                                ৳ {Number(doc.pending_disbursement).toLocaleString()}
                              </td>
                              <td>
                                <button
                                  className="admin-btn admin-btn-outline"
                                  style={{ padding: '4px 10px', fontSize: '0.75rem' }}
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleOpenDoctorDossier(doc.id);
                                  }}
                                >
                                  View Dossier ↗
                                </button>
                              </td>
                            </tr>
                          ))
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 2: FINANCE & SETTLEMENT */}
          {/* ============================================================== */}
          {activeTab === 'finance' && (
            <div id="adminPage_finance" className="admin-page-section active">
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">TOTAL TRANSACTION VOLUME</span>
                    <span className="admin-kpi-trend trend-positive">100% Gross</span>
                  </div>
                  <div className="admin-kpi-value">
                    ৳ {Number(financeSummary.total_gross).toLocaleString()}
                  </div>
                  <div className="admin-kpi-subtitle">Total patient payments</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">PLATFORM CHARGE (20%)</span>
                    <span className="admin-kpi-trend trend-positive">Strict 20% Cut</span>
                  </div>
                  <div className="admin-kpi-value">
                    ৳ {Number(financeSummary.total_platform_fee).toLocaleString()}
                  </div>
                  <div className="admin-kpi-subtitle">Platform operational fee</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">PHYSICIAN NET (80%)</span>
                    <span className="admin-kpi-trend trend-positive">Payable Net</span>
                  </div>
                  <div className="admin-kpi-value">
                    ৳ {Number(financeSummary.total_net).toLocaleString()}
                  </div>
                  <div className="admin-kpi-subtitle">Doctor accrued earnings</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">PENDING DISBURSEMENT</span>
                    <span className="admin-kpi-trend trend-warning">Unsettled</span>
                  </div>
                  <div className="admin-kpi-value">
                    ৳ {Number(financeSummary.pending_disbursement).toLocaleString()}
                  </div>
                  <div className="admin-kpi-subtitle">Pending monthly batch</div>
                </div>
              </div>

              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">Omnichannel Clearing Ledger</h3>
                    <p className="admin-card-subtitle">
                      bKash, Nagad, and Card transactions recorded with 3-way reconciliation split.
                    </p>
                  </div>
                  <button
                    className="admin-btn admin-btn-primary"
                    onClick={handleBatchDisbursement}
                  >
                    ⚡ Process Monthly Disbursement
                  </button>
                </div>

                <div className="admin-table-container">
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Transaction #</th>
                        <th>Gateway</th>
                        <th>Patient</th>
                        <th>Physician</th>
                        <th>Gross</th>
                        <th>20% Fee</th>
                        <th>Net (80%)</th>
                        <th>Status</th>
                        <th>Action</th>
                      </tr>
                    </thead>
                    <tbody>
                      {transactions.length === 0 ? (
                        <tr>
                          <td
                            colSpan={9}
                            style={{
                              textAlign: 'center',
                              padding: '48px 24px',
                              color: 'var(--text-muted)',
                            }}
                          >
                            <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>💳</div>
                            <div
                              style={{
                                fontWeight: 700,
                                fontSize: '1.1rem',
                                color: 'var(--text-primary)',
                              }}
                            >
                              No Transactions Found in Clearing Ledger
                            </div>
                            <div style={{ fontSize: '0.85rem', marginTop: '6px' }}>
                              Financial records will populate here in real-time as patients book appointments.
                            </div>
                          </td>
                        </tr>
                      ) : (
                        transactions.map((tx) => (
                          <tr key={tx.id}>
                            <td>
                              <span style={{ fontFamily: 'var(--font-mono)', fontWeight: 600 }}>
                                {tx.transaction_number}
                              </span>
                            </td>
                            <td>
                              <span
                                className={`admin-badge ${
                                  tx.gateway === 'BKASH'
                                    ? 'badge-pink'
                                    : tx.gateway === 'NAGAD'
                                    ? 'badge-orange'
                                    : 'badge-blue'
                                }`}
                              >
                                {tx.gateway}
                              </span>
                            </td>
                            <td>{tx.patient_name}</td>
                            <td>{tx.doctor_name}</td>
                            <td style={{ fontWeight: 600 }}>৳ {tx.gross_amount}</td>
                            <td style={{ color: '#ef4444' }}>- ৳ {tx.platform_fee_amount}</td>
                            <td style={{ color: '#10b981', fontWeight: 600 }}>৳ {tx.net_amount}</td>
                            <td>
                              <span
                                className={`admin-badge ${
                                  tx.payment_status === 'SETTLED_TO_DOCTOR'
                                    ? 'badge-green'
                                    : tx.payment_status === 'PAYMENT_HELD'
                                    ? 'badge-amber'
                                    : 'badge-slate'
                                }`}
                              >
                                {tx.payment_status}
                              </span>
                            </td>
                            <td>
                              <button
                                className="admin-btn admin-btn-outline"
                                style={{ padding: '4px 8px', fontSize: '0.75rem' }}
                                onClick={() => setSelectedTransaction(tx)}
                              >
                                Inspect ↗
                              </button>
                            </td>
                          </tr>
                        ))
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 3: COMMAND CENTER & TELEMETRY */}
          {/* ============================================================== */}
          {activeTab === 'command' && (
            <div id="adminPage_command" className="admin-page-section active">
              <div className="admin-kpi-grid">
                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">ACTIVE SPECIALISTS ON-DUTY</span>
                    <span className="admin-kpi-trend trend-positive">Live Shift</span>
                  </div>
                  <div className="admin-kpi-value">{commandTelemetry.active_specialists}</div>
                  <div className="admin-kpi-subtitle">Available for instant routing</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">ACTIVE STREAMS (WEBRTC)</span>
                    <span className="admin-kpi-trend trend-positive">Agora Engine</span>
                  </div>
                  <div className="admin-kpi-value">{commandTelemetry.active_sessions_count}</div>
                  <div className="admin-kpi-subtitle">Simultaneous consultations</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">COMPLETED TODAY</span>
                    <span className="admin-kpi-trend trend-positive">Encounters</span>
                  </div>
                  <div className="admin-kpi-value">{commandTelemetry.completed_today}</div>
                  <div className="admin-kpi-subtitle">Concluded successfully</div>
                </div>

                <div className="admin-kpi-card">
                  <div className="admin-kpi-meta">
                    <span className="admin-kpi-label">NETWORK JITTER BUFFER</span>
                    <span className="admin-kpi-trend trend-positive">Under 35ms</span>
                  </div>
                  <div className="admin-kpi-value">22 ms</div>
                  <div className="admin-kpi-subtitle">Bangladesh Edge Nodes</div>
                </div>
              </div>

              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">Live Active WebRTC Streams</h3>
                    <p className="admin-card-subtitle">
                      Real-time Agora media channels, packet loss %, and connection stability monitor.
                    </p>
                  </div>
                </div>

                <div style={{ padding: '20px' }}>
                  {commandTelemetry.active_streams.length === 0 ? (
                    <div
                      style={{
                        textAlign: 'center',
                        padding: '48px 24px',
                        background: 'var(--bg-surface)',
                        borderRadius: '12px',
                        border: '1px dashed var(--border-color)',
                      }}
                    >
                      <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>📡</div>
                      <div
                        style={{
                          fontWeight: 700,
                          fontSize: '1.1rem',
                          color: 'var(--text-primary)',
                        }}
                      >
                        Zero Active WebRTC Channels
                      </div>
                      <div style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginTop: '6px' }}>
                        No video or audio teleconsultations are currently streaming. Live channel telemetry will appear here when appointments start.
                      </div>
                    </div>
                  ) : (
                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(320px, 1fr))', gap: '16px' }}>
                      {commandTelemetry.active_streams.map((st) => (
                        <div
                          key={st.session_id}
                          style={{
                            padding: '16px',
                            background: 'var(--bg-surface)',
                            border: '1px solid var(--border-color)',
                            borderRadius: '8px',
                          }}
                        >
                          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '8px' }}>
                            <span className="admin-badge badge-green">● Transmitting</span>
                            <span style={{ fontSize: '0.75rem', fontFamily: 'var(--font-mono)' }}>
                              {st.rtt_ms}ms RTT
                            </span>
                          </div>
                          <div style={{ fontWeight: 600 }}>{st.doctor_name}</div>
                          <div style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>
                            with {st.patient_name}
                          </div>
                          <div
                            style={{
                              marginTop: '12px',
                              fontSize: '0.75rem',
                              fontFamily: 'var(--font-mono)',
                              color: 'var(--text-muted)',
                            }}
                          >
                            Channel: {st.channel_name} • Loss: {st.packet_loss_percent}%
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 4: SLOTS MATRIX */}
          {/* ============================================================== */}
          {activeTab === 'slots' && (
            <div id="adminPage_slots" className="admin-page-section active">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">10-Minute Micro-Shift Timetable Matrix</h3>
                    <p className="admin-card-subtitle">
                      Atomic slot allocation preventing overbooking and schedule collisions.
                    </p>
                  </div>
                  <button
                    className="admin-btn admin-btn-primary"
                    onClick={() => setShowAddSlotModal(true)}
                  >
                    + Add Micro-Slot
                  </button>
                </div>

                <div className="admin-table-container">
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Doctor</th>
                        <th>Start Time</th>
                        <th>End Time</th>
                        <th>Status</th>
                        <th>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {slots.length === 0 ? (
                        <tr>
                          <td
                            colSpan={5}
                            style={{
                              textAlign: 'center',
                              padding: '48px 24px',
                              color: 'var(--text-muted)',
                            }}
                          >
                            <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>🗓️</div>
                            <div
                              style={{
                                fontWeight: 700,
                                fontSize: '1.1rem',
                                color: 'var(--text-primary)',
                              }}
                            >
                              No Micro-Shift Slots Found
                            </div>
                            <div style={{ fontSize: '0.85rem', marginTop: '6px' }}>
                              There are currently no scheduled consultation slots registered in the system.
                            </div>
                            <button
                              className="admin-btn admin-btn-primary"
                              style={{ marginTop: '16px' }}
                              onClick={() => setShowAddSlotModal(true)}
                            >
                              + Add Micro-Slot
                            </button>
                          </td>
                        </tr>
                      ) : (
                        slots.map((slot) => (
                          <tr key={slot.id}>
                            <td style={{ fontWeight: 600 }}>{slot.doctor_name}</td>
                            <td style={{ fontFamily: 'var(--font-mono)' }}>
                              {new Date(slot.start_time).toLocaleTimeString()}
                            </td>
                            <td style={{ fontFamily: 'var(--font-mono)' }}>
                              {new Date(slot.end_time).toLocaleTimeString()}
                            </td>
                            <td>
                              <span
                                className={`admin-badge ${
                                  slot.status === 'AVAILABLE'
                                    ? 'badge-green'
                                    : slot.status === 'BOOKED'
                                    ? 'badge-blue'
                                    : slot.status === 'LOCKED_IN_PAYMENT'
                                    ? 'badge-amber'
                                    : 'badge-slate'
                                }`}
                              >
                                {slot.status}
                              </span>
                            </td>
                            <td>
                              <button
                                className="admin-btn admin-btn-outline"
                                style={{ padding: '4px 10px', fontSize: '0.75rem' }}
                                onClick={() => handleToggleSlot(slot.id)}
                              >
                                {slot.status === 'AVAILABLE' ? 'Cancel' : 'Make Available'}
                              </button>
                            </td>
                          </tr>
                        ))
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 5: PATIENT REGISTRY & VAULT */}
          {/* ============================================================== */}
          {activeTab === 'patients' && (
            <div id="adminPage_patients" className="admin-page-section active">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">Patient Registry & Clinical Vault</h3>
                    <p className="admin-card-subtitle">
                      Demographic records, chronic cohorts, and encrypted health intake documents.
                    </p>
                  </div>
                </div>

                <div className="admin-table-container">
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Patient Name</th>
                        <th>Date of Birth / Gender</th>
                        <th>Blood Group</th>
                        <th>Emergency Contact</th>
                        <th>Encounters</th>
                        <th>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {patients.length === 0 ? (
                        <tr>
                          <td
                            colSpan={6}
                            style={{
                              textAlign: 'center',
                              padding: '48px 24px',
                              color: 'var(--text-muted)',
                            }}
                          >
                            <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>👤</div>
                            <div
                              style={{
                                fontWeight: 700,
                                fontSize: '1.1rem',
                                color: 'var(--text-primary)',
                              }}
                            >
                              No Patients Registered in System
                            </div>
                            <div style={{ fontSize: '0.85rem', marginTop: '6px' }}>
                              Patient profiles will be registered upon OTP verification and onboarding.
                            </div>
                          </td>
                        </tr>
                      ) : (
                        patients.map((pt) => (
                          <tr
                            key={pt.id}
                            style={{ cursor: 'pointer' }}
                            onClick={() => handleOpenPatientDossier(pt.id)}
                          >
                            <td style={{ fontWeight: 600 }}>{pt.full_name}</td>
                            <td>
                              {pt.date_of_birth} • {pt.gender}
                            </td>
                            <td>
                              <span className="admin-badge badge-blue">{pt.blood_group || 'N/A'}</span>
                            </td>
                            <td>{pt.emergency_contact_phone || 'None'}</td>
                            <td>{pt.consultations_count} visits</td>
                            <td>
                              <button
                                className="admin-btn admin-btn-outline"
                                style={{ padding: '4px 10px', fontSize: '0.75rem' }}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleOpenPatientDossier(pt.id);
                                }}
                              >
                                View Vault ↗
                              </button>
                            </td>
                          </tr>
                        ))
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 6: BMDC VETTING QUEUE */}
          {/* ============================================================== */}
          {activeTab === 'bmdc' && (
            <div id="adminPage_bmdc" className="admin-page-section active">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">BMDC Credential Verification Queue</h3>
                    <p className="admin-card-subtitle">
                      Cross-reference medical license numbers with Bangladesh Medical & Dental Council registry.
                    </p>
                  </div>
                </div>

                <div style={{ padding: '20px' }}>
                  {bmdcQueue.length === 0 ? (
                    <div
                      style={{
                        textAlign: 'center',
                        padding: '48px 24px',
                        background: 'var(--bg-surface)',
                        borderRadius: '12px',
                        border: '1px dashed var(--border-color)',
                      }}
                    >
                      <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>🛡️</div>
                      <div
                        style={{
                          fontWeight: 700,
                          fontSize: '1.1rem',
                          color: 'var(--text-primary)',
                        }}
                      >
                        BMDC Vetting Queue Clear
                      </div>
                      <div style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginTop: '6px' }}>
                        All registered physicians have been fully verified and activated. No pending applications.
                      </div>
                    </div>
                  ) : (
                    bmdcQueue.map((doc) => (
                      <div
                        key={doc.id}
                        style={{
                          padding: '16px',
                          border: '1px solid var(--border-color)',
                          borderRadius: '8px',
                          marginBottom: '12px',
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                        }}
                      >
                        <div>
                          <div style={{ fontWeight: 600, fontSize: '1rem' }}>{doc.full_name}</div>
                          <div style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>
                            {doc.primary_specialty} • {doc.current_hospital}
                          </div>
                          <div
                            style={{
                              marginTop: '6px',
                              fontFamily: 'var(--font-mono)',
                              fontSize: '0.8rem',
                            }}
                          >
                            License: <strong>{doc.license_number}</strong> • Phone:{' '}
                            {doc.verified_phone}
                          </div>
                        </div>
                        <div style={{ display: 'flex', gap: '8px' }}>
                          <button
                            className="admin-btn admin-btn-primary"
                            onClick={() => handleVerifyBmdc(doc.id)}
                          >
                            Approve & Verify ✓
                          </button>
                          <button
                            className="admin-btn admin-btn-outline"
                            style={{ color: '#ef4444', borderColor: '#ef4444' }}
                            onClick={() => handleRejectBmdc(doc.id)}
                          >
                            Reject ✕
                          </button>
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 7: COMPLIANCE & NARCOTICS */}
          {/* ============================================================== */}
          {activeTab === 'compliance' && (
            <div id="adminPage_compliance" className="admin-page-section active">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">DGDA Schedule H & Controlled Substances Audit</h3>
                    <p className="admin-card-subtitle">
                      Automated screening for Morphine, Pethidine, and restricted narcotics requiring counter-signature.
                    </p>
                  </div>
                </div>

                <div style={{ padding: '20px' }}>
                  {complianceAlerts.length === 0 ? (
                    <div
                      style={{
                        textAlign: 'center',
                        padding: '48px 24px',
                        background: 'var(--bg-surface)',
                        borderRadius: '12px',
                        border: '1px dashed var(--border-color)',
                      }}
                    >
                      <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>⚖️</div>
                      <div
                        style={{
                          fontWeight: 700,
                          fontSize: '1.1rem',
                          color: 'var(--text-primary)',
                        }}
                      >
                        100% DGDA Compliance Adherence
                      </div>
                      <div style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginTop: '6px' }}>
                        No Schedule H narcotics infractions or unauthorized prescriptions flagged across the network.
                      </div>
                    </div>
                  ) : (
                    complianceAlerts.map((alt) => (
                      <div
                        key={alt.id}
                        style={{
                          padding: '16px',
                          border: '1px solid #ef4444',
                          borderRadius: '8px',
                          marginBottom: '12px',
                          background: 'rgba(239, 68, 68, 0.05)',
                        }}
                      >
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                          <span className="admin-badge badge-amber">{alt.category}</span>
                          <span style={{ fontSize: '0.75rem', fontFamily: 'var(--font-mono)' }}>
                            {alt.rx_number}
                          </span>
                        </div>
                        <div style={{ fontWeight: 600, marginTop: '8px' }}>
                          Doctor: {alt.doctor_name} → Patient: {alt.patient_name}
                        </div>
                        <div style={{ fontSize: '0.85rem', marginTop: '4px', color: '#b91c1c' }}>
                          {alt.flag_reason}
                        </div>
                        <div
                          style={{
                            marginTop: '8px',
                            fontFamily: 'var(--font-mono)',
                            fontSize: '0.8rem',
                            background: 'var(--bg-surface)',
                            padding: '8px',
                            borderRadius: '4px',
                          }}
                        >
                          Notes: {alt.doctor_notes}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 8: SYSTEM LOGS & DIAGNOSTICS */}
          {/* ============================================================== */}
          {activeTab === 'logs' && (
            <div id="adminPage_logs" className="admin-page-section active">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">System Audit Events & Exception Traces</h3>
                    <p className="admin-card-subtitle">
                      Zero-loss forensic audit trail logging all backend mutations and subsystem errors.
                    </p>
                  </div>
                  <button
                    className="admin-btn admin-btn-outline"
                    onClick={() => setShowSimulateModal(true)}
                  >
                    🚨 Inject Failure Trace
                  </button>
                </div>

                <div className="admin-table-container">
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Action</th>
                        <th>Subsystem</th>
                        <th>Result</th>
                        <th>Reason / Notes</th>
                        <th>Timestamp</th>
                      </tr>
                    </thead>
                    <tbody>
                      {logs.length === 0 ? (
                        <tr>
                          <td
                            colSpan={5}
                            style={{
                              textAlign: 'center',
                              padding: '48px 24px',
                              color: 'var(--text-muted)',
                            }}
                          >
                            <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>📋</div>
                            <div
                              style={{
                                fontWeight: 700,
                                fontSize: '1.1rem',
                                color: 'var(--text-primary)',
                              }}
                            >
                              System Health Optimal — Zero Exceptions
                            </div>
                            <div style={{ fontSize: '0.85rem', marginTop: '6px' }}>
                              No unhandled exceptions or failed handshakes recorded in backend audit storage.
                            </div>
                            <button
                              className="admin-btn admin-btn-outline"
                              style={{ marginTop: '16px' }}
                              onClick={() => setShowSimulateModal(true)}
                            >
                              🚨 Inject Test Failure Trace
                            </button>
                          </td>
                        </tr>
                      ) : (
                        logs.map((lg) => (
                          <tr
                            key={lg.id}
                            style={{ cursor: 'pointer' }}
                            onClick={() => setSelectedLog(lg)}
                          >
                            <td style={{ fontFamily: 'var(--font-mono)', fontWeight: 600 }}>
                              {lg.action}
                            </td>
                            <td>{lg.resource_type}</td>
                            <td>
                              <span
                                className={`admin-badge ${
                                  lg.result === 'SUCCESS' || lg.result === 'OPTIMAL'
                                    ? 'badge-green'
                                    : lg.result === 'ERROR' || lg.result === 'FAILURE'
                                    ? 'badge-amber'
                                    : 'badge-blue'
                                }`}
                              >
                                {lg.result}
                              </span>
                            </td>
                            <td>{lg.reason || 'Operation completed'}</td>
                            <td style={{ fontSize: '0.75rem', fontFamily: 'var(--font-mono)' }}>
                              {new Date(lg.created_at).toLocaleString()}
                            </td>
                          </tr>
                        ))
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {/* ============================================================== */}
          {/* TAB 9: GRIEVANCES & DISPUTES */}
          {/* ============================================================== */}
          {activeTab === 'grievances' && (
            <div id="adminPage_grievances" className="admin-page-section active">
              <div className="admin-card">
                <div className="admin-card-header">
                  <div>
                    <h3 className="admin-card-title">Patient Grievances & Adjudication Board</h3>
                    <p className="admin-card-subtitle">
                      Agora WebRTC telemetry forensics with one-click direct MFS refund adjudication.
                    </p>
                  </div>
                </div>

                <div className="admin-table-container">
                  <table className="admin-table">
                    <thead>
                      <tr>
                        <th>Grievance #</th>
                        <th>Patient</th>
                        <th>Accused Doctor</th>
                        <th>Category</th>
                        <th>Claim Summary</th>
                        <th>Status</th>
                        <th>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {grievances.length === 0 ? (
                        <tr>
                          <td
                            colSpan={7}
                            style={{
                              textAlign: 'center',
                              padding: '48px 24px',
                              color: 'var(--text-muted)',
                            }}
                          >
                            <div style={{ fontSize: '2.5rem', marginBottom: '12px' }}>🚨</div>
                            <div
                              style={{
                                fontWeight: 700,
                                fontSize: '1.1rem',
                                color: 'var(--text-primary)',
                              }}
                            >
                              Grievance Board Clear
                            </div>
                            <div style={{ fontSize: '0.85rem', marginTop: '6px' }}>
                              Zero unresolved patient disputes or clinical claims in the queue.
                            </div>
                          </td>
                        </tr>
                      ) : (
                        grievances.map((g) => (
                          <tr
                            key={g.id}
                            style={{ cursor: 'pointer' }}
                            onClick={() => setSelectedGrievance(g)}
                          >
                            <td style={{ fontFamily: 'var(--font-mono)', fontWeight: 600 }}>
                              {g.grievance_number}
                            </td>
                            <td>{g.patient_name}</td>
                            <td>{g.doctor_name}</td>
                            <td>
                              <span className="admin-badge badge-blue">{g.category}</span>
                            </td>
                            <td>{g.claim_summary}</td>
                            <td>
                              <span
                                className={`admin-badge ${
                                  g.status === 'REFUNDED'
                                    ? 'badge-green'
                                    : g.status === 'WARNED'
                                    ? 'badge-amber'
                                    : 'badge-pink'
                                }`}
                              >
                                {g.status}
                              </span>
                            </td>
                            <td>
                              <button
                                className="admin-btn admin-btn-outline"
                                style={{ padding: '4px 8px', fontSize: '0.75rem' }}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setSelectedGrievance(g);
                                }}
                              >
                                Review Forensics ↗
                              </button>
                            </td>
                          </tr>
                        ))
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}
        </div>
      </main>

      {/* ============================================================== */}
      {/* MODAL 1: DOCTOR DOSSIER MODAL */}
      {/* ============================================================== */}
      {selectedDoctorDossier && (
        <div
          className="admin-modal-overlay active"
          onClick={() => setSelectedDoctorDossier(null)}
        >
          <div
            className="admin-modal"
            style={{ maxWidth: '820px' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="admin-modal-header">
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <div
                  className="admin-user-avatar"
                  style={{ width: '48px', height: '48px', fontSize: '1.2rem', background: '#059669', color: '#fff' }}
                >
                  {selectedDoctorDossier.full_name.slice(3, 5).toUpperCase()}
                </div>
                <div>
                  <h3 className="admin-modal-title" style={{ fontSize: '1.25rem' }}>
                    {selectedDoctorDossier.full_name}
                  </h3>
                  <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                    {selectedDoctorDossier.license_number} • {selectedDoctorDossier.primary_specialty} •{' '}
                    {selectedDoctorDossier.current_hospital}
                  </div>
                </div>
              </div>
              <button
                className="admin-modal-close"
                onClick={() => setSelectedDoctorDossier(null)}
              >
                ✕
              </button>
            </div>

            <div className="admin-modal-body">
              {/* Financial & Clinical Stats Strip */}
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(4, 1fr)',
                  gap: '12px',
                  marginBottom: '20px',
                }}
              >
                <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '8px' }}>
                  <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)' }}>LIFETIME GROSS</div>
                  <div style={{ fontSize: '1.1rem', fontWeight: 700 }}>
                    ৳ {Number(selectedDoctorDossier.wallet.lifetime_gross).toLocaleString()}
                  </div>
                </div>
                <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '8px' }}>
                  <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)' }}>20% PLATFORM FEE</div>
                  <div style={{ fontSize: '1.1rem', fontWeight: 700, color: '#ef4444' }}>
                    ৳ {Number(selectedDoctorDossier.wallet.lifetime_fee_withheld).toLocaleString()}
                  </div>
                </div>
                <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '8px' }}>
                  <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)' }}>LIFETIME NET (80%)</div>
                  <div style={{ fontSize: '1.1rem', fontWeight: 700, color: '#10b981' }}>
                    ৳ {Number(selectedDoctorDossier.wallet.lifetime_net).toLocaleString()}
                  </div>
                </div>
                <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '8px' }}>
                  <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)' }}>PENDING PAYOUT</div>
                  <div style={{ fontSize: '1.1rem', fontWeight: 700, color: '#f59e0b' }}>
                    ৳ {Number(selectedDoctorDossier.wallet.pending_disbursement).toLocaleString()}
                  </div>
                </div>
              </div>

              {/* Consultation Encounters */}
              <h4 style={{ fontSize: '0.95rem', fontWeight: 600, marginBottom: '8px' }}>
                Recent Encounters ({selectedDoctorDossier.encounters.length})
              </h4>
              <div className="admin-table-container" style={{ maxHeight: '200px' }}>
                <table className="admin-table">
                  <thead>
                    <tr>
                      <th>Appointment #</th>
                      <th>Patient</th>
                      <th>Modality</th>
                      <th>Fee</th>
                      <th>Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {selectedDoctorDossier.encounters.length === 0 ? (
                      <tr>
                        <td colSpan={5} style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
                          No consultation encounters recorded for this physician yet.
                        </td>
                      </tr>
                    ) : (
                      selectedDoctorDossier.encounters.map((enc) => (
                        <tr key={enc.appointment_id}>
                          <td>{enc.appointment_number}</td>
                          <td>{enc.patient_name}</td>
                          <td>{enc.modality}</td>
                          <td>৳ {enc.consultation_fee}</td>
                          <td>
                            <span className="admin-badge badge-green">{enc.status}</span>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>
            </div>

            <div className="admin-modal-footer">
              <button
                className="admin-btn admin-btn-outline"
                onClick={() => setSelectedDoctorDossier(null)}
              >
                Close Dossier
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ============================================================== */}
      {/* MODAL 2: PATIENT DOSSIER MODAL */}
      {/* ============================================================== */}
      {selectedPatientDossier && (
        <div
          className="admin-modal-overlay active"
          onClick={() => setSelectedPatientDossier(null)}
        >
          <div
            className="admin-modal"
            style={{ maxWidth: '780px' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="admin-modal-header">
              <div>
                <h3 className="admin-modal-title">
                  {selectedPatientDossier.patient.full_name}
                </h3>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                  DOB: {selectedPatientDossier.patient.date_of_birth} • Gender:{' '}
                  {selectedPatientDossier.patient.gender} • Blood:{' '}
                  {selectedPatientDossier.patient.blood_group || 'N/A'}
                </div>
              </div>
              <button
                className="admin-modal-close"
                onClick={() => setSelectedPatientDossier(null)}
              >
                ✕
              </button>
            </div>

            <div className="admin-modal-body">
              <h4 style={{ fontSize: '0.95rem', fontWeight: 600, marginBottom: '8px' }}>
                Consultation History ({selectedPatientDossier.encounters.length})
              </h4>
              <div className="admin-table-container" style={{ maxHeight: '200px' }}>
                <table className="admin-table">
                  <thead>
                    <tr>
                      <th>Encounter #</th>
                      <th>Attending Physician</th>
                      <th>Modality</th>
                      <th>Fee</th>
                      <th>Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {selectedPatientDossier.encounters.length === 0 ? (
                      <tr>
                        <td colSpan={5} style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
                          Zero encounters in clinical vault.
                        </td>
                      </tr>
                    ) : (
                      selectedPatientDossier.encounters.map((enc) => (
                        <tr key={enc.appointment_id}>
                          <td>{enc.appointment_number}</td>
                          <td>{enc.doctor_name}</td>
                          <td>{enc.modality}</td>
                          <td>৳ {enc.fee}</td>
                          <td>{enc.status}</td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>
            </div>

            <div className="admin-modal-footer">
              <button
                className="admin-btn admin-btn-outline"
                onClick={() => setSelectedPatientDossier(null)}
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ============================================================== */}
      {/* MODAL 3: ONBOARD PHYSICIAN MODAL */}
      {/* ============================================================== */}
      {showOnboardDoctorModal && (
        <div
          className="admin-modal-overlay active"
          onClick={() => setShowOnboardDoctorModal(false)}
        >
          <div
            className="admin-modal"
            style={{ maxWidth: '600px' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="admin-modal-header">
              <h3 className="admin-modal-title">🩺 Onboard New Physician</h3>
              <button
                className="admin-modal-close"
                onClick={() => setShowOnboardDoctorModal(false)}
              >
                ✕
              </button>
            </div>

            <form onSubmit={handleCreateDoctor}>
              <div className="admin-modal-body">
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px' }}>
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Doctor Full Name</label>
                    <input
                      type="text"
                      className="admin-input"
                      placeholder="Dr. Rahman..."
                      required
                      value={newDoctorForm.full_name}
                      onChange={(e) =>
                        setNewDoctorForm({ ...newDoctorForm, full_name: e.target.value })
                      }
                      style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                    />
                  </div>
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>BMDC License #</label>
                    <input
                      type="text"
                      className="admin-input"
                      placeholder="BMDC #A-58291"
                      required
                      value={newDoctorForm.license_number}
                      onChange={(e) =>
                        setNewDoctorForm({ ...newDoctorForm, license_number: e.target.value })
                      }
                      style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                    />
                  </div>
                </div>

                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr 1fr',
                    gap: '12px',
                    marginTop: '12px',
                  }}
                >
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Primary Specialty</label>
                    <input
                      type="text"
                      className="admin-input"
                      required
                      value={newDoctorForm.primary_specialty}
                      onChange={(e) =>
                        setNewDoctorForm({ ...newDoctorForm, primary_specialty: e.target.value })
                      }
                      style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                    />
                  </div>
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Years of Experience</label>
                    <input
                      type="number"
                      className="admin-input"
                      required
                      value={newDoctorForm.experience_years}
                      onChange={(e) =>
                        setNewDoctorForm({
                          ...newDoctorForm,
                          experience_years: Number(e.target.value),
                        })
                      }
                      style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                    />
                  </div>
                </div>

                <div style={{ marginTop: '12px' }}>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Hospital Affiliation</label>
                  <input
                    type="text"
                    className="admin-input"
                    placeholder="Dhaka Medical College Hospital"
                    required
                    value={newDoctorForm.current_hospital}
                    onChange={(e) =>
                      setNewDoctorForm({ ...newDoctorForm, current_hospital: e.target.value })
                    }
                    style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                  />
                </div>

                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr 1fr',
                    gap: '12px',
                    marginTop: '12px',
                  }}
                >
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Verified Phone</label>
                    <input
                      type="text"
                      className="admin-input"
                      required
                      value={newDoctorForm.verified_phone}
                      onChange={(e) =>
                        setNewDoctorForm({ ...newDoctorForm, verified_phone: e.target.value })
                      }
                      style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                    />
                  </div>
                  <div>
                    <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Video Fee (৳)</label>
                    <input
                      type="number"
                      className="admin-input"
                      required
                      value={newDoctorForm.consultation_fee_video}
                      onChange={(e) =>
                        setNewDoctorForm({
                          ...newDoctorForm,
                          consultation_fee_video: Number(e.target.value),
                        })
                      }
                      style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                    />
                  </div>
                </div>
              </div>

              <div className="admin-modal-footer">
                <button
                  type="button"
                  className="admin-btn admin-btn-outline"
                  onClick={() => setShowOnboardDoctorModal(false)}
                >
                  Cancel
                </button>
                <button type="submit" className="admin-btn admin-btn-primary">
                  Save to Backend Database
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* ============================================================== */}
      {/* MODAL 4: ADD MICRO-SLOT MODAL */}
      {/* ============================================================== */}
      {showAddSlotModal && (
        <div
          className="admin-modal-overlay active"
          onClick={() => setShowAddSlotModal(false)}
        >
          <div
            className="admin-modal"
            style={{ maxWidth: '520px' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="admin-modal-header">
              <h3 className="admin-modal-title">🗓️ Add 10-Minute Micro-Shift Slot</h3>
              <button
                className="admin-modal-close"
                onClick={() => setShowAddSlotModal(false)}
              >
                ✕
              </button>
            </div>

            <form onSubmit={handleCreateSlot}>
              <div className="admin-modal-body">
                <div>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Select Physician</label>
                  <select
                    className="admin-input"
                    required
                    value={newSlotForm.doctor_id}
                    onChange={(e) =>
                      setNewSlotForm({ ...newSlotForm, doctor_id: e.target.value })
                    }
                    style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                  >
                    <option value="">-- Choose Doctor --</option>
                    {doctors.map((d) => (
                      <option key={d.id} value={d.id}>
                        {d.full_name} ({d.primary_specialty})
                      </option>
                    ))}
                  </select>
                </div>

                <div style={{ marginTop: '12px' }}>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Start Time</label>
                  <input
                    type="datetime-local"
                    className="admin-input"
                    required
                    value={newSlotForm.start_time}
                    onChange={(e) =>
                      setNewSlotForm({ ...newSlotForm, start_time: e.target.value })
                    }
                    style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                  />
                </div>

                <div style={{ marginTop: '12px' }}>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>End Time</label>
                  <input
                    type="datetime-local"
                    className="admin-input"
                    required
                    value={newSlotForm.end_time}
                    onChange={(e) =>
                      setNewSlotForm({ ...newSlotForm, end_time: e.target.value })
                    }
                    style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                  />
                </div>
              </div>

              <div className="admin-modal-footer">
                <button
                  type="button"
                  className="admin-btn admin-btn-outline"
                  onClick={() => setShowAddSlotModal(false)}
                >
                  Cancel
                </button>
                <button type="submit" className="admin-btn admin-btn-primary">
                  Commit Slot to Matrix
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* ============================================================== */}
      {/* MODAL 5: SIMULATE FAILURE MODAL */}
      {/* ============================================================== */}
      {showSimulateModal && (
        <div
          className="admin-modal-overlay active"
          onClick={() => setShowSimulateModal(false)}
        >
          <div
            className="admin-modal"
            style={{ maxWidth: '560px' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="admin-modal-header">
              <h3 className="admin-modal-title">🚨 Inject Subsystem Diagnostics Event</h3>
              <button
                className="admin-modal-close"
                onClick={() => setShowSimulateModal(false)}
              >
                ✕
              </button>
            </div>

            <form onSubmit={handleSimulateLog}>
              <div className="admin-modal-body">
                <div>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Subsystem Target</label>
                  <select
                    className="admin-input"
                    value={simForm.subsystem}
                    onChange={(e) => setSimForm({ ...simForm, subsystem: e.target.value })}
                    style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                  >
                    <option value="PAYMENT_GATEWAY_BKASH">bKash MFS Webhook Pipeline</option>
                    <option value="AGORA_RTC_ORCHESTRATOR">Agora WebRTC Media Bridge</option>
                    <option value="BMDC_VERIFICATION_ADAPTER">BMDC Registry Uplink</option>
                    <option value="PRESCRIPTION_STORAGE">S3 Encrypted Vault Sync</option>
                  </select>
                </div>

                <div style={{ marginTop: '12px' }}>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Action Code</label>
                  <input
                    type="text"
                    className="admin-input"
                    value={simForm.action}
                    onChange={(e) => setSimForm({ ...simForm, action: e.target.value })}
                    style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                  />
                </div>

                <div style={{ marginTop: '12px' }}>
                  <label style={{ fontSize: '0.8rem', fontWeight: 600 }}>Error Summary</label>
                  <textarea
                    className="admin-input"
                    rows={3}
                    value={simForm.error_summary}
                    onChange={(e) =>
                      setSimForm({ ...simForm, error_summary: e.target.value })
                    }
                    style={{ width: '100%', marginTop: '4px', padding: '8px' }}
                  />
                </div>
              </div>

              <div className="admin-modal-footer">
                <button
                  type="button"
                  className="admin-btn admin-btn-outline"
                  onClick={() => setShowSimulateModal(false)}
                >
                  Cancel
                </button>
                <button type="submit" className="admin-btn admin-btn-primary">
                  Inject Audit Record
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* ============================================================== */}
      {/* MODAL 6: GRIEVANCE DETAIL & TELEMETRY FORENSICS */}
      {/* ============================================================== */}
      {selectedGrievance && (
        <div
          className="admin-modal-overlay active"
          onClick={() => setSelectedGrievance(null)}
        >
          <div
            className="admin-modal"
            style={{ maxWidth: '720px' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="admin-modal-header">
              <div>
                <h3 className="admin-modal-title">
                  Dispute Case: {selectedGrievance.grievance_number}
                </h3>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                  Filed against: {selectedGrievance.doctor_name} by{' '}
                  {selectedGrievance.patient_name}
                </div>
              </div>
              <button
                className="admin-modal-close"
                onClick={() => setSelectedGrievance(null)}
              >
                ✕
              </button>
            </div>

            <div className="admin-modal-body">
              <div
                style={{
                  background: 'var(--bg-surface)',
                  padding: '16px',
                  borderRadius: '8px',
                  border: '1px solid var(--border-color)',
                  marginBottom: '16px',
                }}
              >
                <div style={{ fontWeight: 600, fontSize: '0.9rem', marginBottom: '4px' }}>
                  Patient Claim:
                </div>
                <div style={{ fontSize: '0.9rem', color: 'var(--text-primary)' }}>
                  "{selectedGrievance.claim_summary}"
                </div>
              </div>

              {selectedGrievance.telemetry ? (
                <div>
                  <h4 style={{ fontSize: '0.9rem', fontWeight: 600, marginBottom: '8px' }}>
                    📡 Forensic Agora WebRTC Telemetry
                  </h4>
                  <div
                    style={{
                      display: 'grid',
                      gridTemplateColumns: 'repeat(3, 1fr)',
                      gap: '12px',
                    }}
                  >
                    <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '6px' }}>
                      <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Call Duration</div>
                      <div style={{ fontWeight: 700, fontSize: '1rem' }}>
                        {selectedGrievance.telemetry.duration_seconds}s
                      </div>
                    </div>
                    <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '6px' }}>
                      <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Packet Loss</div>
                      <div style={{ fontWeight: 700, fontSize: '1rem', color: '#ef4444' }}>
                        {selectedGrievance.telemetry.packet_loss_percent}%
                      </div>
                    </div>
                    <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '6px' }}>
                      <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Round-Trip Time</div>
                      <div style={{ fontWeight: 700, fontSize: '1rem' }}>
                        {selectedGrievance.telemetry.rtt_ms}ms
                      </div>
                    </div>
                  </div>
                </div>
              ) : (
                <div style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>
                  No automated WebRTC telemetry session associated with this dispute.
                </div>
              )}
            </div>

            <div className="admin-modal-footer">
              <button
                className="admin-btn admin-btn-outline"
                onClick={() => setSelectedGrievance(null)}
              >
                Close
              </button>
              <button
                className="admin-btn admin-btn-outline"
                style={{ color: '#f59e0b', borderColor: '#f59e0b' }}
                onClick={() => handleAdjudicateWarn(selectedGrievance.id)}
              >
                Issue Warning to Doctor
              </button>
              <button
                className="admin-btn admin-btn-primary"
                onClick={() => handleAdjudicateRefund(selectedGrievance.id)}
              >
                Disburse Direct Refund (bKash/Nagad)
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ============================================================== */}
      {/* MODAL 7: TRANSACTION DETAIL MODAL */}
      {/* ============================================================== */}
      {selectedTransaction && (
        <div
          className="admin-modal-overlay active"
          onClick={() => setSelectedTransaction(null)}
        >
          <div
            className="admin-modal"
            style={{ maxWidth: '640px' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="admin-modal-header">
              <div>
                <h3 className="admin-modal-title">
                  Transaction Receipt: {selectedTransaction.transaction_number}
                </h3>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>
                  Gateway: {selectedTransaction.gateway} • Reference:{' '}
                  {selectedTransaction.gateway_reference || 'N/A'}
                </div>
              </div>
              <button
                className="admin-modal-close"
                onClick={() => setSelectedTransaction(null)}
              >
                ✕
              </button>
            </div>

            <div className="admin-modal-body">
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: '1fr 1fr 1fr',
                  gap: '12px',
                  marginBottom: '16px',
                }}
              >
                <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '8px' }}>
                  <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>GROSS COLLECTED</div>
                  <div style={{ fontSize: '1.2rem', fontWeight: 700 }}>
                    ৳ {selectedTransaction.gross_amount}
                  </div>
                </div>
                <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '8px' }}>
                  <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>PLATFORM CUT (20%)</div>
                  <div style={{ fontSize: '1.2rem', fontWeight: 700, color: '#ef4444' }}>
                    ৳ {selectedTransaction.platform_fee_amount}
                  </div>
                </div>
                <div style={{ background: 'var(--bg-surface)', padding: '12px', borderRadius: '8px' }}>
                  <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>DOCTOR NET (80%)</div>
                  <div style={{ fontSize: '1.2rem', fontWeight: 700, color: '#10b981' }}>
                    ৳ {selectedTransaction.net_amount}
                  </div>
                </div>
              </div>

              <div style={{ fontSize: '0.85rem' }}>
                <div>Patient: <strong>{selectedTransaction.patient_name}</strong></div>
                <div style={{ marginTop: '4px' }}>
                  Physician: <strong>{selectedTransaction.doctor_name}</strong>
                </div>
                <div style={{ marginTop: '4px' }}>
                  Status: <span className="admin-badge badge-green">{selectedTransaction.payment_status}</span>
                </div>
              </div>
            </div>

            <div className="admin-modal-footer">
              <button
                className="admin-btn admin-btn-outline"
                onClick={() => setSelectedTransaction(null)}
              >
                Close Receipt
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
