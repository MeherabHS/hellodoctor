import {tr, type MessageKey} from './i18n/core.ts';

/** "ADMIN_ARTICLE_PUBLISHED" -> "Admin article published" (unit-tested). */
function humanize(code: string): string {
  const words = code.toLowerCase().split('_').filter(Boolean).join(' ');
  return words ? words[0].toUpperCase() + words.slice(1) : code;
}

/** One item from GET /admin/notifications (backend activity_handlers.rs). */
export type AdminNotice =
  | {id: string; kind: 'activity'; at: string; action: string; resource_type: string; result: string; actor_role: string; actor_email: string | null}
  | {id: string; kind: 'grievance'; at: string; grievance_number: string}
  | {id: string; kind: 'incident'; at: string; severity: string; component: string};

export type NoticeTone = 'info' | 'success' | 'warning' | 'urgent';
/** Page ids from components/shell/AdminSidebar.tsx. */
export type NoticePage = 'verification' | 'peers' | 'patients' | 'grievances' | 'logs' | 'blogs' | 'quizzes' | 'topics' | 'facilities' | 'campaigns' | 'finance' | 'team' | 'compliance' | 'activity';
export interface NoticeView {title: string; body: string; page: NoticePage; tone: NoticeTone}

const PAGE_BY_RESOURCE: Record<string, NoticePage> = {
  ADMIN_ARTICLE: 'blogs', ARTICLE_QUIZ: 'quizzes', ARTICLE_CATEGORY: 'topics', DOCTOR_ARTICLE: 'blogs',
  HEALTH_FACILITY: 'facilities', CAMPAIGN: 'campaigns', NOTIFICATION_CAMPAIGN: 'campaigns',
  DOCTOR_PROFILE: 'verification', PEER_NAVIGATOR: 'peers', PATIENT_PROFILE: 'patients', PATIENT: 'patients', ERASURE_JOB: 'patients',
  DATA_RIGHTS_REQUEST: 'patients', PATIENT_DATA_REQUEST: 'patients',
  GRIEVANCE_REPORT: 'grievances', SYSTEM_ERROR: 'logs', SYSTEM_ERROR_LOG: 'logs', HEALTH_ANSWER: 'compliance',
  ADMIN_INVITATION: 'team', TRANSACTION: 'finance', DISBURSEMENT: 'finance', DISBURSEMENT_ITEM: 'finance',
  DISBURSEMENT_BATCH: 'finance', FINANCIAL_PERIOD: 'finance', FINANCIAL_REFUND_REQUEST: 'finance', REFUND_REQUEST: 'finance',
};

/** Plain-language titles for the events Admins most need to notice (Swahili: messages `notice`). */
const ACTION_TITLE: Record<string, string> = {
  DOCTOR_PROFILE_CREATED: 'New doctor application',
  DOCTOR_PROFESSIONAL_VERIFY: 'Doctor verified',
  DOCTOR_PROFESSIONAL_REJECT: 'Doctor application rejected',
  DOCTOR_PROFESSIONAL_REVOKE: 'Doctor verification revoked',
  PEER_NAVIGATOR_CREATED: 'New Peer Navigator application',
  PEER_NAVIGATOR_APPROVED: 'Peer Navigator approved',
  PEER_NAVIGATOR_REJECTED: 'Peer Navigator application rejected',
  PEER_NAVIGATOR_SUSPENDED: 'Peer Navigator suspended',
  PEER_NAVIGATOR_HISTORY_VIEWED: 'Peer Navigator record viewed',
  PEER_NAVIGATOR_RECORD_VIEWED: 'Peer Navigator application viewed',
  PATIENT_RECORD_VIEWED: 'Patient record viewed',
  DOCTOR_RECORD_VIEWED: 'Doctor record viewed',
  DOCTOR_DOCUMENT_VIEWED: 'Doctor verification document viewed',
  ADMIN_RECORD_VIEWED: 'Record details viewed',
  ADMIN_ACTION: 'Admin action',
  DATA_RIGHTS_ERASURE_REQUESTED: 'Account deletion requested',
  DATA_RIGHTS_PATIENT_ERASED: 'Patient account deleted',
  DATA_RIGHTS_PATIENT_ANONYMIZED: 'Patient account anonymized',
  DATA_RIGHTS_DOCTOR_ERASED: 'Doctor account deleted',
  DATA_RIGHTS_DOCTOR_ANONYMIZED: 'Doctor account anonymized',
  AUTH_ROLE_MISMATCH: 'Blocked a forged session',
  CONCURRENT_LIVE_PARTICIPATION_DETECTED: 'Same account in two live consultations',
  LATE_PAYMENT_RECONCILIATION_REQUIRED: 'Late payment needs reconciliation',
  ADMIN_INVITATION_SENT: 'Admin invited',
  ADMIN_INVITATION_RESENT: 'Admin invitation resent',
  ADMIN_INVITATION_REVOKED: 'Admin invitation revoked',
  ADMIN_INVITATION_ACCEPTED: 'New Admin joined',
  GRIEVANCE_REFUND_APPROVED: 'Grievance refunded',
  GRIEVANCE_WARN: 'Grievance: doctor warned',
  GRIEVANCE_DISMISS: 'Grievance dismissed',
  GRIEVANCE_APPEAL_RECORDED: 'Grievance appeal recorded',
  GRIEVANCE_APPEAL_UPHELD: 'Grievance appeal upheld',
  GRIEVANCE_APPEAL_OVERTURNED: 'Grievance appeal overturned',
  QNA_ANSWER_FLAGGED: 'Doctor answer flagged',
  QNA_ANSWER_APPROVED: 'Doctor answer approved',
  SYSTEM_ERROR_ACKNOWLEDGED: 'Incident acknowledged',
  SYSTEM_ERROR_RESOLVED: 'Incident resolved',
};
const URGENT = new Set(['AUTH_ROLE_MISMATCH', 'CONCURRENT_LIVE_PARTICIPATION_DETECTED', 'DATA_RIGHTS_ERASURE_REQUESTED']);
const ATTENTION = new Set(['DOCTOR_PROFILE_CREATED', 'PEER_NAVIGATOR_CREATED', 'LATE_PAYMENT_RECONCILIATION_REQUIRED']);
const ROLES = new Set(['ADMIN', 'DOCTOR', 'PEER', 'PATIENT', 'SYSTEM']);

export function describeNotice(n: AdminNotice): NoticeView {
  if (n.kind === 'grievance') {
    return {title: tr('notice.newGrievance'), body: tr('notice.grievanceWaiting', {number: n.grievance_number}), page: 'grievances', tone: 'warning'};
  }
  // Incident notices stay in English, like the incident log itself.
  if (n.kind === 'incident') {
    const sev = n.severity.toUpperCase();
    return {
      title: `${humanize(sev)} app incident`,
      body: `${n.component} reported an error.`,
      page: 'logs',
      tone: sev === 'CRITICAL' || sev === 'ERROR' ? 'urgent' : 'warning',
    };
  }
  const who = n.actor_email ?? (ROLES.has(n.actor_role) ? tr(`notice.role_${n.actor_role}` as MessageKey) : humanize(n.actor_role));
  const title = Object.hasOwn(ACTION_TITLE, n.action) ? tr(`notice.${n.action}` as MessageKey) : humanize(n.action);
  const page = PAGE_BY_RESOURCE[n.resource_type] ?? 'activity';
  const tone: NoticeTone = URGENT.has(n.action) ? 'urgent' : ATTENTION.has(n.action) ? 'warning' : 'info';
  const body = n.action === 'DOCTOR_PROFILE_CREATED' ? tr('notice.doctorWaiting')
    : n.action === 'PEER_NAVIGATOR_CREATED' ? tr('notice.peerWaiting') : tr('notice.byWho', {who});
  return {title, body, page, tone};
}
