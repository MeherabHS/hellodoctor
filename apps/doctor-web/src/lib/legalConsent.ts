// Shared between the BFF (src/lib/server.ts) and the registration UI
// (src/components/Auth.tsx). Framework-agnostic on purpose, so both the
// Node/Next.js API route and the client component can import it unchanged.
//
// Neither function invents, hardcodes, or caches a document type/version:
// both operate only on whatever GET /legal/documents most recently returned
// (fetched at src/lib/server.ts's handleLegalDocuments), which remains the
// single source of truth the backend's own validate_accepted_documents
// already enforces server-side (services/backend/src/domain/validation.rs).
// These are a shape/completeness pre-check only, never a replacement for it.

export type AcceptedDocumentSubmission = { type: string; version: string };

/// Shape-only validation of the accepted_documents array a client submitted:
/// a non-empty array of {type, version} pairs of non-empty strings. Does
/// NOT check the type is a known one or the version is current -- that is,
/// and must remain, the backend's job alone.
export function validateAcceptedDocumentsShape(value: unknown): AcceptedDocumentSubmission[] | null {
  if (!Array.isArray(value) || value.length === 0) return null;
  for (const item of value) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) return null;
    const record = item as Record<string, unknown>;
    if (typeof record.type !== 'string' || !record.type) return null;
    if (typeof record.version !== 'string' || !record.version) return null;
  }
  return value as AcceptedDocumentSubmission[];
}

/// The exact document_type values a Doctor registration applies to --
/// DOCTOR_TERMS/PRIVACY_NOTICE/PROFESSIONAL_VERIFICATION_DECLARATION/
/// TELEMEDICINE_PRACTITIONER_POLICY (mandatory) plus MARKETING_COMMUNICATIONS
/// (optional, shared with Patient). GET /legal/documents is a single shared,
/// role-agnostic registry (confirmed in services/backend/src/repository/
/// legal_repo.rs::list_current -- no audience filter exists server-side), so
/// it also returns four Patient-only document types
/// (PATIENT_DATA_SHARING_CONSENT/PATIENT_TERMS/PATIENT_PRIVACY_NOTICE/
/// PATIENT_DATA_RIGHTS) that must never reach a Doctor registration form.
/// An explicit allow-list, not a Patient-type exclude-list, so a future new
/// Patient-only document type is excluded by default rather than requiring
/// this file to be remembered and updated.
const DOCTOR_APPLICABLE_DOCUMENT_TYPES = new Set([
  'DOCTOR_TERMS',
  'PRIVACY_NOTICE',
  'PROFESSIONAL_VERIFICATION_DECLARATION',
  'TELEMEDICINE_PRACTITIONER_POLICY',
  'MARKETING_COMMUNICATIONS',
]);

export function filterDoctorApplicableDocuments<T extends { document_type: string }>(documents: T[]): T[] {
  return documents.filter(doc => DOCTOR_APPLICABLE_DOCUMENT_TYPES.has(doc.document_type));
}

export type LegalDocumentRequirement = { document_type: string; required: boolean };

/// True only if every document the backend currently marks required has a
/// truthy entry in `accepted` (keyed by document_type). Optional documents
/// (e.g. Marketing Communications) never block this check either way.
export function allRequiredDocumentsAccepted(
  documents: LegalDocumentRequirement[],
  accepted: Record<string, boolean>,
): boolean {
  return documents.every(doc => !doc.required || !!accepted[doc.document_type]);
}
