import {request} from './api';

/** Grievance appeals recorded by an Admin (docs/admin-ui/ADMIN_REDESIGN_PLAN.md). */
export interface GrievanceAppeal {
  id: string;
  grievance_id: string;
  grievance_number: string;
  category: string;
  target: 'DOCTOR' | 'SYSTEM';
  grievance_status: string;
  /** The decision appealed against: WARNING or DISMISS. */
  appealed_action: string;
  appealed_at: string;
  reason: string;
  status: 'OPEN' | 'UPHELD' | 'OVERTURNED';
  recorded_by: string;
  recorded_at: string;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
}
export const listAppeals = () => request<GrievanceAppeal[]>('/admin/grievances/appeals');
export const recordAppeal = (grievanceId: string, reason: string) =>
  request<{appeal_id: string}>(`/admin/grievances/${encodeURIComponent(grievanceId)}/appeals`, {method: 'POST', body: JSON.stringify({reason})});
export const decideAppeal = (appealId: string, decision: 'UPHOLD' | 'OVERTURN', note: string) =>
  request<{grievance_status: string}>(`/admin/grievance-appeals/${encodeURIComponent(appealId)}/decide`, {method: 'POST', body: JSON.stringify({decision, note})});

/** Q&A answer audit. The patient author is never part of these responses. */
export type AuditStatus = 'UNREVIEWED' | 'APPROVED' | 'FLAGGED';
export interface QnaAnswer {
  answer_id: string;
  question_id: string;
  question_text: string;
  category: string | null;
  asked_at: string;
  answer_text: string;
  answered_at: string;
  doctor_id: string;
  doctor_name: string | null;
  doctor_specialty: string | null;
  audit_status: AuditStatus;
  audit_updated_at: string | null;
  last_review: {decision: 'APPROVED' | 'FLAGGED'; note: string; reviewed_at: string; reviewed_by: string} | null;
}
export interface QnaReview {id: string; decision: 'APPROVED' | 'FLAGGED'; note: string; reviewed_by: string; reviewed_at: string}
export interface QnaAnswerPage {items: QnaAnswer[]; total: number; limit: number; offset: number; counts: Record<AuditStatus | 'ALL', number>}
export const listQnaAnswers = (query: URLSearchParams) => request<QnaAnswerPage>(`/admin/qna/answers?${query}`);
export const listQnaReviews = (answerId: string) => request<QnaReview[]>(`/admin/qna/answers/${encodeURIComponent(answerId)}/reviews`);
export const reviewQnaAnswer = (answerId: string, decision: 'APPROVED' | 'FLAGGED', note: string) =>
  request<{audit_status: AuditStatus; hidden_from_patients: boolean}>(`/admin/qna/answers/${encodeURIComponent(answerId)}/reviews`, {method: 'POST', body: JSON.stringify({decision, note})});
