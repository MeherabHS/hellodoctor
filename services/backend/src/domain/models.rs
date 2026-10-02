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
            UserRole::PlatformAdmin => "PLATFORM_ADMIN",
            UserRole::ClinicalAdmin => "CLINICAL_ADMIN",
            UserRole::FinanceAdmin => "FINANCE_ADMIN",
            UserRole::Support => "SUPPORT",
            UserRole::Compliance => "COMPLIANCE",
            UserRole::SecurityAdmin => "SECURITY_ADMIN",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
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
    pub user_id: Uuid,
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
#[sqlx(type_name = "mfa_method_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MfaMethod {
    Totp,
    Passkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MfaCredential {
    pub id: Uuid,
    pub user_id: Uuid,
    pub method: MfaMethod,
    pub totp_secret_encrypted: Vec<u8>,
    pub recovery_codes_hash: Option<Vec<String>>,
    pub is_enabled: bool,
    pub enabled_at: Option<DateTime<Utc>>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
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
    pub id: Uuid,
    pub user_id: Uuid,
    pub full_name: String,
    pub date_of_birth: NaiveDate,
    pub gender: Gender,
    pub blood_group: Option<String>,
    pub weight_kg: Option<BigDecimal>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relation: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DoctorProfile {
    pub id: Uuid,
    pub user_id: Uuid,
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
    pub is_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "slot_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SlotStatus {
    Available,
    LockedInPayment,
    Booked,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduleSlot {
    pub id: Uuid,
    pub doctor_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub status: SlotStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "appointment_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
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
    pub patient_id: Uuid,
    pub doctor_id: Uuid,
    pub slot_id: Uuid,
    pub modality: Modality,
    pub status: AppointmentStatus,
    pub consultation_fee: BigDecimal,
    pub clinical_outcome: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppointmentClinicalIntake {
    pub appointment_id: Uuid,
    pub patient_id: Uuid,
    pub chief_complaint: Option<String>,
    pub clinical_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PrescriptionIntakeDocument {
    pub id: Uuid,
    pub appointment_id: Uuid,
    pub patient_id: Uuid,
    pub page_number: i16,
    pub object_bucket: String,
    pub object_key: String,
    pub content_hash: Option<String>,
    pub file_size_bytes: i64,
    pub mime_type: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Prescription {
    pub id: Uuid,
    pub appointment_id: Uuid,
    pub doctor_id: Uuid,
    pub patient_id: Uuid,
    pub rx_number: String,
    pub integrity_verification_hash: Option<String>,
    pub rx_image_object_bucket: Option<String>,
    pub rx_image_object_key: Option<String>,
    pub doctor_notes: Option<String>,
    pub is_no_rx_required: bool,
    pub no_rx_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatConversation {
    pub id: Uuid,
    pub appointment_id: Uuid,
    pub patient_id: Uuid,
    pub doctor_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub is_locked: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
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
#[sqlx(type_name = "grievance_target_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrievanceTarget {
    Doctor,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "grievance_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GrievanceStatus {
    PendingReview,
    UnderInvestigation,
    Refunded,
    Warned,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GrievanceReport {
    pub id: Uuid,
    pub grievance_number: String,
    pub patient_id: Uuid,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(type_name = "gateway_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Gateway {
    Bkash,
    Nagad,
    Card,
    Mpesa,
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
    pub created_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DoctorWallet {
    pub doctor_id: Uuid,
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
    pub doctor_id: Uuid,
    pub gross_amount: BigDecimal,
    pub platform_fee: BigDecimal,
    pub net_amount: BigDecimal,
    pub gateway: Gateway,
    pub gateway_reference: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_role: UserRole,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub ip_hash: Option<String>,
    pub result: String,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IdempotencyRecord {
    pub id: Uuid,
    pub key_hash: String,
    pub user_id: Option<Uuid>,
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
    pub user_id: Uuid,
    pub code_hash: String,
    pub expires_at: DateTime<Utc>,
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
