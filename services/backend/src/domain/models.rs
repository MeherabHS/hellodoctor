use super::ids::{DoctorProfileId, PatientProfileId, UserId};
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "user_role_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserRole {
    #[default]
    Patient,
    Doctor,
    Admin,
    PlatformAdmin,
    ClinicalAdmin,
    FinanceAdmin,
    Support,
    Compliance,
    SecurityAdmin,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Patient => "PATIENT",
            UserRole::Doctor => "DOCTOR",
            UserRole::Admin => "ADMIN",
            UserRole::PlatformAdmin => "PLATFORM_ADMIN",
            UserRole::ClinicalAdmin => "CLINICAL_ADMIN",
            UserRole::FinanceAdmin => "FINANCE_ADMIN",
            UserRole::Support => "SUPPORT",
            UserRole::Compliance => "COMPLIANCE",
            UserRole::SecurityAdmin => "SECURITY_ADMIN",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ADMIN" => Some(UserRole::Admin),
            "PATIENT" => Some(UserRole::Patient),
            "DOCTOR" => Some(UserRole::Doctor),
            "PLATFORM_ADMIN" => Some(UserRole::PlatformAdmin),
            "CLINICAL_ADMIN" => Some(UserRole::ClinicalAdmin),
            "FINANCE_ADMIN" => Some(UserRole::FinanceAdmin),
            "SUPPORT" => Some(UserRole::Support),
            "COMPLIANCE" => Some(UserRole::Compliance),
            "SECURITY_ADMIN" => Some(UserRole::SecurityAdmin),
            _ => None,
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::Admin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: UserId,
    pub phone_number: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub role: UserRole,
    pub preferred_language: String,
    pub is_active: bool,
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuthSession {
    pub id: Uuid,
    pub user_id: UserId,
    pub token_family_id: Uuid,
    pub refresh_token_hash: String,
    pub device_fingerprint: Option<String>,
    pub ip_hash: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub rotated_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "gender_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Gender {
    Male,
    Female,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PatientProfile {
    pub id: PatientProfileId,
    /// `NULL` for a dependent profile (one with no login of its own) — see
    /// `guardian_patient_id` and `patient_profiles_self_xor_dependent`
    /// (migration `20261101000001_patient_profile_dependents`): a profile is
    /// always exactly one of a real login-linked self profile or a
    /// guardian-managed dependent.
    pub user_id: Option<UserId>,
    pub full_name: String,
    pub date_of_birth: NaiveDate,
    pub gender: Gender,
    pub blood_group: Option<String>,
    pub weight_kg: Option<BigDecimal>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relation: Option<String>,
    /// Non-null only for a dependent profile — the owning account's own
    /// self-profile `id`. Single-level only (a dependent can never itself
    /// guardian another dependent), enforced by a DB trigger.
    pub guardian_patient_id: Option<PatientProfileId>,
    /// Free-text display label for a dependent ("Spouse", "Child", ...) —
    /// not a backend-interpreted enum, always `NULL` for a self profile.
    pub relationship: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// HelloDoctor's manual registration review; independent of account suspension,
/// annual practice licensure, MFA, duty and payout readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "professional_verification_status_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ProfessionalVerificationStatus {
    Pending,
    Verified,
    Rejected,
    /// A formerly-VERIFIED Doctor whose clinical capability was withdrawn by
    /// an Admin (h22 remediation). Terminal, like Rejected, but reachable
    /// only from Verified — see guard_professional_verification() in
    /// migrations/20261107000002_doctor_verification_revocation.up.sql.
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DoctorProfile {
    pub id: DoctorProfileId,
    pub user_id: UserId,
    pub full_name: String,
    pub license_number: String,
    pub license_authority: String,
    pub license_country: String,
    pub primary_specialty: String,
    pub experience_years: i16,
    pub current_hospital: String,
    pub qualifications: Vec<String>,
    pub consultation_fee_video: BigDecimal,
    pub consultation_fee_chat: BigDecimal,
    pub residential_address: String,
    pub verified_phone: String,
    pub is_on_duty: bool,
    /// Legacy response compatibility only; database-derived from explicit status.
    pub is_verified: bool,
    pub professional_verification_status: ProfessionalVerificationStatus,
    pub professional_verification_reviewed_at: Option<DateTime<Utc>>,
    pub professional_verification_reviewed_by: Option<UserId>,
    pub professional_verification_note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl DoctorProfile {
    pub fn is_professionally_verified(&self) -> bool {
        self.professional_verification_status == ProfessionalVerificationStatus::Verified
    }
}

/// The five documents a Doctor can acknowledge at registration. The first
/// four are mandatory; `MarketingCommunications` is the one optional,
/// independently revocable acknowledgement. See
/// docs/ADR/ADR-011-legal-consent-ledger.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "legal_document_type_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum LegalDocumentType {
    DoctorTerms,
    PrivacyNotice,
    ProfessionalVerificationDeclaration,
    TelemedicinePractitionerPolicy,
    MarketingCommunications,
    PatientDataSharingConsent,
    PatientTerms,
    PatientPrivacyNotice,
    PatientDataRights,
}

impl LegalDocumentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LegalDocumentType::DoctorTerms => "DOCTOR_TERMS",
            LegalDocumentType::PrivacyNotice => "PRIVACY_NOTICE",
            LegalDocumentType::ProfessionalVerificationDeclaration => {
                "PROFESSIONAL_VERIFICATION_DECLARATION"
            }
            LegalDocumentType::TelemedicinePractitionerPolicy => "TELEMEDICINE_PRACTITIONER_POLICY",
            LegalDocumentType::MarketingCommunications => "MARKETING_COMMUNICATIONS",
            LegalDocumentType::PatientDataSharingConsent => "PATIENT_DATA_SHARING_CONSENT",
            LegalDocumentType::PatientTerms => "PATIENT_TERMS",
            LegalDocumentType::PatientPrivacyNotice => "PATIENT_PRIVACY_NOTICE",
            LegalDocumentType::PatientDataRights => "PATIENT_DATA_RIGHTS",
        }
    }
}

/// A server-owned registry row: the single source of truth for the text and
/// version currently required. `document_hash` is computed by Postgres from
/// `body` (a generated column) — never supplied by application code.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LegalDocument {
    pub id: Uuid,
    pub document_type: LegalDocumentType,
    pub version: String,
    pub title: String,
    pub body: String,
    pub document_hash: String,
    pub required: bool,
    pub is_current: bool,
    pub created_at: DateTime<Utc>,
}

/// One immutable, append-only acceptance record. Rows are never updated or
/// deleted (no role has those privileges on `legal_acceptances`); a changed
/// document version produces a new row, not a mutation of an old one.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LegalAcceptance {
    pub id: Uuid,
    pub user_id: UserId,
    pub document_type: LegalDocumentType,
    pub document_version: String,
    pub document_hash: String,
    pub accepted_at: DateTime<Utc>,
    pub request_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub ip_hash: Option<String>,
    pub app_version: Option<String>,
    pub platform: Option<String>,
    pub locale: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "slot_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SlotStatus {
    Available,
    Blocked,
    LockedInPayment,
    Booked,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduleSlot {
    pub id: Uuid,
    pub doctor_id: DoctorProfileId,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SlotStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "payout_method_type_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum PayoutMethodType {
    Mpesa,
    Bkash,
    BankCard,
    BankTransfer,
}

/// Where Finance should send a doctor's scheduled disbursement. Setting or
/// updating this never moves money or calls an external payment provider —
/// see the migration comment (DEFERRED_EXTERNAL_PAYMENT_INTEGRATION is
/// unaffected). `account_identifier_encrypted` is only ever decrypted by
/// Finance tooling that actually processes a payout, never by the doctor-facing
/// read path, which uses `account_identifier_last4` instead.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DoctorPayoutMethod {
    pub doctor_id: DoctorProfileId,
    pub method_type: PayoutMethodType,
    pub display_label: String,
    #[serde(skip_serializing)]
    pub account_identifier_encrypted: Vec<u8>,
    pub account_identifier_last4: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "appointment_status_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum AppointmentStatus {
    PendingPayment,
    Confirmed,
    Waiting,
    InConsultation,
    Completed,
    Cancelled,
    Expired,
    PrematureTermination,
    Disputed,
    Refunded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "modality_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Modality {
    Video,
    Chat,
}

/// Recorded once, at booking time, from the `FinancialModuleMode` in effect
/// *then* — never re-derived later, so flipping the server-wide mode can
/// never retroactively change what an existing appointment owes or doesn't.
/// `Required` is the default for every historically-existing row (see
/// migration 20261204000002). `NotRequired` is set only when
/// `AppointmentService::book_appointment` runs in `FinancialModuleMode::Deferred`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "payment_policy_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum PaymentPolicy {
    Required,
    NotRequired,
}

/// Stored as plain TEXT + CHECK constraint in Postgres (not a native enum type),
/// so DB rows carry it as `Option<String>` and convert through this type at the edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClinicalOutcome {
    CompletedWithRx,
    CompletedNoRx,
    Referred,
    Escalated,
}

impl ClinicalOutcome {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ClinicalOutcome::CompletedWithRx => "COMPLETED_WITH_RX",
            ClinicalOutcome::CompletedNoRx => "COMPLETED_NO_RX",
            ClinicalOutcome::Referred => "REFERRED",
            ClinicalOutcome::Escalated => "ESCALATED",
        }
    }

    pub fn parse_db_str(value: &str) -> Option<Self> {
        match value {
            "COMPLETED_WITH_RX" => Some(ClinicalOutcome::CompletedWithRx),
            "COMPLETED_NO_RX" => Some(ClinicalOutcome::CompletedNoRx),
            "REFERRED" => Some(ClinicalOutcome::Referred),
            "ESCALATED" => Some(ClinicalOutcome::Escalated),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Appointment {
    pub id: Uuid,
    pub appointment_number: String,
    pub patient_id: PatientProfileId,
    pub doctor_id: DoctorProfileId,
    pub slot_id: Uuid,
    pub modality: Modality,
    pub status: AppointmentStatus,
    pub consultation_fee: BigDecimal,
    pub payment_policy: PaymentPolicy,
    pub clinical_outcome: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppointmentClinicalIntake {
    pub appointment_id: Uuid,
    pub patient_id: PatientProfileId,
    pub chief_complaint: Option<String>,
    pub clinical_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PrescriptionIntakeDocument {
    pub id: Uuid,
    pub appointment_id: Uuid,
    pub patient_id: PatientProfileId,
    pub page_number: i16,
    pub object_bucket: String,
    pub object_key: String,
    pub content_hash: Option<String>,
    pub file_size_bytes: i64,
    pub mime_type: String,
    pub security_status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "session_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionStatus {
    Initialized,
    Active,
    Completed,
    PrematureTermination,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConsultationSession {
    pub id: Uuid,
    pub appointment_id: Uuid,
    pub agora_channel_name: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub recovery_state: String,
    pub empty_room_deadline: Option<DateTime<Utc>>,
    pub patient_present: bool,
    pub doctor_present: bool,
    pub patient_presence_sequence: i64,
    pub doctor_presence_sequence: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConsultationTelemetry {
    pub id: Uuid,
    pub session_id: Uuid,
    pub call_duration_seconds: i32,
    pub connection_state: String,
    pub packet_loss_percent: Option<BigDecimal>,
    pub round_trip_time_ms: Option<i32>,
    pub premature_end: bool,
    pub prescription_issued: bool,
    pub captured_at: DateTime<Utc>,
}

/// Detailed, ordered, per-perspective supplement to the single aggregate
/// `ConsultationTelemetry` row above — see migrations/20261004000002 and
/// `ConsultationService::record_telemetry_events`/`get_session_timeline`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConsultationTelemetryEvent {
    pub id: Uuid,
    pub ingest_seq: i64,
    pub session_id: Uuid,
    pub event_type: String,
    pub actor_perspective: String,
    pub rtt_ms: Option<i32>,
    pub packet_loss_percent: Option<BigDecimal>,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Prescription {
    pub id: Uuid,
    pub appointment_id: Uuid,
    pub doctor_id: DoctorProfileId,
    pub patient_id: PatientProfileId,
    pub rx_number: String,
    pub integrity_verification_hash: Option<String>,
    pub rx_image_object_bucket: Option<String>,
    pub rx_image_object_key: Option<String>,
    pub doctor_notes: Option<String>,
    pub is_no_rx_required: bool,
    pub no_rx_reason: Option<String>,
    pub security_status: String,
    pub created_at: DateTime<Utc>,
    pub page_number: i16,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatConversation {
    pub id: Uuid,
    pub appointment_id: Uuid,
    pub patient_id: PatientProfileId,
    pub doctor_id: DoctorProfileId,
    pub expires_at: DateTime<Utc>,
    pub is_locked: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: UserId,
    pub content: String,
    pub attachment_bucket: Option<String>,
    pub attachment_object_key: Option<String>,
    pub attachment_mime: Option<String>,
    pub attachment_size: Option<i64>,
    pub attachment_hash: Option<String>,
    pub is_read: bool,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "grievance_target_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum GrievanceTarget {
    Doctor,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "grievance_status_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum GrievanceStatus {
    PendingReview,
    UnderInvestigation,
    RefundApproved,
    Refunded,
    Warned,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GrievanceReport {
    pub id: Uuid,
    pub grievance_number: String,
    pub patient_id: PatientProfileId,
    pub consultation_id: Uuid,
    pub target_type: GrievanceTarget,
    pub category: String,
    pub claim_summary: String,
    pub status: GrievanceStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GrievanceAdjudication {
    pub id: Uuid,
    pub grievance_id: Uuid,
    pub adjudicated_by_admin_id: Uuid,
    pub board_remedy: String,
    pub action_type: String,
    pub audit_notes: String,
    pub adjudicated_at: DateTime<Utc>,
}

/// A refund approved by `GrievanceService::adjudicate_refund` but not yet
/// (necessarily) resolved — `status` tracks the actual provider-side outcome
/// so Finance has a visible, transitionable lifecycle instead of the request
/// going silently unresolved forever once approved (GAP-C03).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FinancialRefundRequest {
    pub id: Uuid,
    pub grievance_id: Uuid,
    pub transaction_id: Uuid,
    pub approved_by: Uuid,
    pub approved_at: DateTime<Utc>,
    pub currency: String,
    pub amount: BigDecimal,
    pub previous_payment_status: String,
    pub status: String,
    pub provider_reference: Option<String>,
    pub refunded_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "gateway_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Gateway {
    Bkash,
    Nagad,
    Card,
    Mpesa,
    BankTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "payment_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentStatus {
    Initiated,
    PaymentHeld,
    SettledToDoctor,
    Disbursed,
    RefundedToPatient,
    RefundPending,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Transaction {
    pub id: Uuid,
    pub transaction_number: String,
    pub appointment_id: Uuid,
    pub payment_session_id: Uuid,
    pub gateway: Gateway,
    pub gateway_reference: Option<String>,
    pub gross_amount: BigDecimal,
    pub platform_fee_amount: BigDecimal, // Strictly 20%
    pub net_amount: BigDecimal,          // Strictly 80%
    pub payment_status: PaymentStatus,
    /// Captured funds without a valid booking require reviewed reconciliation.
    #[serde(default)]
    pub reconciliation_required: bool,
    pub created_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
    pub currency: String,
    pub disbursement_item_id: Option<Uuid>,
    /// Which of the patient's saved `patient_payment_methods` rows (if any)
    /// this transaction was paid with — nullable, since a patient may still
    /// pay ad hoc with no saved method at all, exactly as before this
    /// feature existed.
    pub payment_method_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DoctorWallet {
    pub doctor_id: DoctorProfileId,
    pub lifetime_gross: BigDecimal,
    pub lifetime_fee_withheld: BigDecimal,
    pub lifetime_net: BigDecimal,
    pub pending_disbursement: BigDecimal,
    pub last_disbursement_at: Option<DateTime<Utc>>,
    pub last_disbursement_amount: BigDecimal,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DisbursementBatch {
    pub id: Uuid,
    pub batch_number: String,
    pub initiated_by: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_doctors: i32,
    pub total_gross: BigDecimal,
    pub total_platform_fee: BigDecimal,
    pub total_net_disbursed: BigDecimal,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DisbursementItem {
    pub id: Uuid,
    pub batch_id: Uuid,
    pub doctor_id: DoctorProfileId,
    pub gross_amount: BigDecimal,
    pub platform_fee: BigDecimal,
    pub net_amount: BigDecimal,
    pub gateway: Gateway,
    pub gateway_reference: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub payout_destination_id: Option<Uuid>,
    pub currency: String,
    pub provider_reference_snapshot: Option<String>,
    pub masked_destination_snapshot: Option<String>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub reconciliation_state: String,
    pub statement_id: Option<Uuid>,
    pub adjustments: BigDecimal,
    pub provider_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DoctorPayoutDestination {
    pub id: Uuid,
    pub doctor_id: DoctorProfileId,
    pub provider_code: String,
    pub method_type: String,
    pub currency: String,
    pub provider_reference: String,
    pub masked_destination: String,
    pub binding_status: String,
    pub verification_status: String,
    pub payout_capable: bool,
    /// Which `platform_payment_methods` row this was (simulated-)verified
    /// against — `None` for rows created before this column existed or by
    /// `create_dev_payout_destination`'s dev-only fixture.
    pub platform_payment_method_id: Option<Uuid>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A patient-saved payment method — the same multi-method/one-default shape
/// `DoctorPayoutDestination` already proves out, applied to the paying side
/// instead of the payout side. `provider_reference` is encrypted at rest by
/// the service layer before insert (same as a doctor's own reference); this
/// struct only ever holds whatever the column currently contains, exactly
/// like `DoctorPayoutDestination` does.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PatientPaymentMethod {
    pub id: Uuid,
    pub patient_id: PatientProfileId,
    pub gateway: Gateway,
    pub provider_code: String,
    pub provider_reference: String,
    pub masked_destination: String,
    pub binding_status: String,
    /// Result of `services::payment_provider_verification` at save time —
    /// never `VERIFIED` until a real provider driver exists.
    pub verification_status: String,
    /// Which `platform_payment_methods` row this was (simulated-)verified
    /// against — nullable only for rows saved before this column existed.
    pub platform_payment_method_id: Option<Uuid>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The platform's own merchant/collection configuration — distinct from
/// both of the above: not how a patient pays, nor where a doctor is paid,
/// but the platform's own identity with its payment provider(s). More than
/// one may be active at once (e.g. M-Pesa and a card aggregator together),
/// so "primary" is scoped per `provider_code`, not a single platform-wide
/// choice. `credentials_encrypted` holds a JSON blob of whatever fields that
/// provider's real driver will eventually need (paybill number, consumer
/// key/secret, etc.) — encrypted at rest the same way every other
/// credential in this codebase is, never returned to any client in
/// decrypted form.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PlatformPaymentMethod {
    pub id: Uuid,
    pub provider_code: String,
    pub label: String,
    pub currency: String,
    pub masked_display: String,
    #[serde(skip_serializing)]
    pub credentials_encrypted: String,
    pub is_primary: bool,
    pub is_active: bool,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub actor_id: Option<UserId>,
    pub actor_role: UserRole,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub ip_hash: Option<String>,
    pub result: String,
    pub reason: Option<String>,
    /// Snapshot of the resource immediately before the mutation, or `None`
    /// when the action has no meaningful "before" (e.g. creation). Only
    /// operational/professional/financial data goes in here — never patient
    /// clinical content — so no PHI-redaction pass is needed on this path
    /// (contrast `system_error.rs`'s free-text redaction, which exists
    /// precisely because that path *can* carry arbitrary error text).
    pub before_data: Option<serde_json::Value>,
    pub after_data: Option<serde_json::Value>,
    /// Top-level keys that differ between `before_data`/`after_data`, computed
    /// once at write time so an admin reading the trail doesn't have to diff
    /// two JSON blobs by eye. `None` when either snapshot is absent.
    pub changed_fields: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IdempotencyRecord {
    pub id: Uuid,
    pub key_hash: String,
    pub user_id: Option<UserId>,
    pub route: String,
    pub request_hash: String,
    pub response_status: i16,
    pub response_body_ref: Option<Uuid>,
    pub response_body: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PendingMfaChallenge {
    pub token: String,
    pub user_id: UserId,
    pub code_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub attempt_count: i16,
    pub max_attempts: i16,
}

/// Phase 2: Secure Email OTP Password Recovery. State is derived rather than
/// stored (see migrations/20261207000001_password_reset's doc comment):
/// `authorization_hash.is_none()` means still awaiting OTP verification,
/// `Some` means verified and awaiting the final `complete_reset` call.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PasswordResetRequest {
    pub token: String,
    pub user_id: UserId,
    pub code_hash: String,
    pub attempt_count: i16,
    pub max_attempts: i16,
    pub otp_expires_at: DateTime<Utc>,
    pub authorization_hash: Option<String>,
    pub authorization_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PhoneOtp {
    pub phone_number: String,
    pub code_hash: String,
    pub attempts: i16,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl ErrorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorSeverity::Info => "INFO",
            ErrorSeverity::Warning => "WARNING",
            ErrorSeverity::Error => "ERROR",
            ErrorSeverity::Critical => "CRITICAL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SystemErrorLog {
    pub id: Uuid,
    pub trace_id: String,
    pub subsystem: String,
    pub component: String,
    pub severity: String,
    pub action_attempted: String,
    pub error_message: String,
    pub stack_trace: Option<String>,
    pub impacted_user_id: Option<UserId>,
    pub error_code: Option<String>,
    pub fingerprint: Option<String>,
    pub http_status: Option<i16>,
    pub endpoint: Option<String>,
    pub occurrence_count: i32,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub status: String,
    pub acknowledged_by: Option<UserId>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<UserId>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_note: Option<String>,
    pub logged_at: DateTime<Utc>,
    pub client_context: Option<serde_json::Value>,
}

/// Doctor Engagement extension: one row in a user's in-app notification
/// inbox. Deliberately never carries PHI — `title`/`body` are short,
/// backend-constructed strings (see `services::notification_service`'s copy
/// templates), never symptoms/diagnosis/prescription/patient-clinical
/// content.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: UserId,
    #[sqlx(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub related_entity_type: Option<String>,
    pub related_entity_id: Option<Uuid>,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub delivery_status: String,
    pub dedup_key: String,
    pub metadata: Option<serde_json::Value>,
}
