import { tr } from './i18n/core.ts';
import type { DoctorHistoryItem, PatientRecordItem, GrievanceItem } from '@/data/adminStore';

// Dependency-free leaf module (see finance-projection.ts's own precedent):
// no value-level relative imports, so this is directly test-importable under
// Node's --experimental-strip-types runner without extension-resolution
// issues. Extracted from useBackendSync.ts, which re-exports these.

export const AVATAR_COLORS = ['#059669', '#2563EB', '#7C3AED', '#D97706', '#DC2626', '#0891B2'];

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
    residence: doc.residential_address || tr('lib.notProvided'),
    email: doc.email || tr('lib.notProvided'),
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
      // A deferred (payment_policy='NOT_REQUIRED') appointment still carries
      // a nominal consultation_fee (unchanged modality pricing — see
      // appointment_service.rs::book_appointment_deferred's own comment),
      // but it was never actually charged. Showing that figure here would
      // misrepresent it as real revenue.
      fee: enc.payment_policy === 'NOT_REQUIRED'
        ? 'Deferred'
        : `KES  ${enc.consultation_fee || enc.fee || 0}`,
      diagnosis: enc.clinical_outcome || tr('lib.consultation'),
      rx: enc.clinical_outcome === 'COMPLETED_WITH_RX' ? tr('lib.rxSynced') : tr('lib.noRx'),
    })),
    disbursements: (doc.disbursements || []).map((d: any) => ({
      date: d.created_at ? new Date(d.created_at).toLocaleDateString() : 'Recent',
      amount: `KES  ${Number(d.net_amount || 0).toLocaleString()}`,
      method: `${d.provider_code || tr('lib.providerNotConnected')} ${d.masked_destination_snapshot || ''}`,
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
  // Only a REQUIRED-policy encounter ever had real money collected — a
  // deferred appointment's nominal consultation_fee must never inflate this
  // patient's reported total spend.
  const totalSpend = encounters
    .filter((e) => e.payment_policy !== 'NOT_REQUIRED')
    .reduce((sum, e) => sum + Number(e.fee ?? e.consultation_fee ?? 0), 0);

  return {
    key: pt.id,
    id: `#PT-${pt.id.slice(0, 5).toUpperCase()}`,
    name: pt.full_name,
    chatActive: (pt.chat_count ?? chatSessions) > 0,
    avatar: initials,
    bgColor: AVATAR_COLORS[(idx + 2) % AVATAR_COLORS.length],
    meta: `${pt.date_of_birth || '30'} • ${pt.gender || 'F'} • ${pt.emergency_contact_phone || 'Kenya'}`,
    cohort: pt.blood_group ? `Blood ${pt.blood_group}` : tr('lib.generalPatient'),
    labs: tr('lib.documentsInVault', { count: (pt.vault_records || []).length }),
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
    backendId: pt.id,
    dateOfBirth: pt.date_of_birth ?? null,
    gender: pt.gender ?? null,
    consultationsCount: pt.consultations_count ?? encounters.length,
    lastConsultAt: pt.last_consult_at ?? encounters.map((e) => e.created_at).filter(Boolean).sort().at(-1) ?? null,
    registeredAt: pt.created_at ?? null,
    visits: encounters.map((enc: any) => ({
      appointmentId: enc.appointment_id,
      appointmentNumber: enc.appointment_number ?? '',
      doctor: enc.doctor_name || 'Doctor',
      modality: enc.modality ?? '',
      status: enc.status ?? '',
      outcome: enc.clinical_outcome ?? null,
      createdAt: enc.created_at ?? null,
    })),
    encounters: encounters.map((enc: any) => ({
      date: enc.created_at ? new Date(enc.created_at).toLocaleDateString() : 'Recent',
      doctor: enc.doctor_name || tr('lib.attendingPhysician'),
      spec: 'Telemedicine',
      mode: enc.modality === 'VIDEO' ? '📹 Video Visit' : '💬 24h Chat',
      badgeCls: enc.modality === 'VIDEO' ? 'badge-green' : 'badge-blue',
      diag: enc.clinical_outcome || enc.status || tr('lib.generalConsult'),
      rx: enc.clinical_outcome === 'COMPLETED_WITH_RX' ? tr('lib.rxSynced') : tr('lib.noRx'),
    })),
  };
}

export function backendGrievanceToStoreItem(g: any): GrievanceItem {
  return {
    id: g.grievance_number || g.id,
    backendId: g.id,
    timestamp: g.created_at || '',
    timeFormatted: g.created_at ? new Date(g.created_at).toLocaleTimeString() : tr('lib.notSupplied'),
    relativeTime: g.created_at ? new Date(g.created_at).toLocaleDateString() : tr('lib.notSupplied'),
    patient: {
      id: g.patient_id,
      name: g.patient_name || tr('lib.notSupplied'),
      phone: tr('lib.notProvided'),
      ageGender: tr('lib.demographicsNotSupplied'),
    },
    doctor: {
      id: g.doctor_id || tr('lib.notSupplied'),
      name: g.doctor_name || tr('lib.notSupplied'),
      registrationNumber: g.doctor_license || tr('lib.notProvided'),
      specialty: g.doctor_specialty || tr('lib.notSupplied'),
      hospital: g.doctor_hospital || tr('lib.notSupplied'),
      // See the identical note in backendDocToStoreItem — a deferred
      // appointment's nominal consultation_fee was never actually charged.
      fee: g.appointment_payment_policy === 'NOT_REQUIRED'
        ? 'Deferred'
        : g.consultation_fee == null ? tr('lib.notSupplied') : `KES ${g.consultation_fee}`,
    },
    consultationId: g.consultation_id,
    consultationType: g.modality === 'CHAT' ? tr('lib.chatConsultation') : g.modality === 'VIDEO' ? tr('lib.videoConsultation') : tr('lib.consultation'),
    target: g.target_type || 'DOCTOR',
    category: g.category || tr('lib.notSupplied'),
    severity: ['CRITICAL', 'HIGH', 'MEDIUM'].includes(g.severity) ? g.severity : 'UNASSESSED',
    claimSummary: g.claim_summary,
    patientStatement: g.claim_summary,
    boardRemedy:
      g.status === 'REFUND_APPROVED'
        ? tr('lib.refundApprovedPending')
        : g.status === 'REFUNDED'
        ? tr('lib.refundConfirmed')
        : g.status === 'WARNED'
        ? tr('lib.warningLogged')
        : g.status === 'DISMISSED'
        ? tr('lib.claimDismissed')
        : g.status === 'UNDER_INVESTIGATION'
        ? tr('lib.underInvestigation')
        : tr('lib.underGovernanceReview'),
    telemetryEvidence: {
      callDuration: tr('lib.mediaNotImplemented'),
      connectionStatus: tr('lib.noProviderEvidence'),
      rxIssued: tr('lib.seeRxRecords'),
      paymentStatus: g.payment_status || tr('lib.seeTimeline'),
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
