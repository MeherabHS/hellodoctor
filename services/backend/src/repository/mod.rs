use crate::domain::models::*;
use crate::error::AppError;
use bigdecimal::BigDecimal;
use chrono::{Duration, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct AppState {
    pub users: Arc<RwLock<HashMap<Uuid, User>>>,
    pub auth_sessions: Arc<RwLock<HashMap<Uuid, AuthSession>>>,
    pub mfa_credentials: Arc<RwLock<HashMap<Uuid, MfaCredential>>>,
    pub patient_profiles: Arc<RwLock<HashMap<Uuid, PatientProfile>>>,
    pub doctor_profiles: Arc<RwLock<HashMap<Uuid, DoctorProfile>>>,
    pub schedule_slots: Arc<RwLock<HashMap<Uuid, ScheduleSlot>>>,
    pub appointments: Arc<RwLock<HashMap<Uuid, Appointment>>>,
    pub clinical_intakes: Arc<RwLock<HashMap<Uuid, AppointmentClinicalIntake>>>,
    pub intake_documents: Arc<RwLock<HashMap<Uuid, Vec<PrescriptionIntakeDocument>>>>,
    pub consultation_sessions: Arc<RwLock<HashMap<Uuid, ConsultationSession>>>,
    pub consultation_telemetries: Arc<RwLock<HashMap<Uuid, ConsultationTelemetry>>>,
    pub prescriptions: Arc<RwLock<HashMap<Uuid, Prescription>>>,
    pub chat_conversations: Arc<RwLock<HashMap<Uuid, ChatConversation>>>,
    pub chat_messages: Arc<RwLock<HashMap<Uuid, Vec<ChatMessage>>>>,
    pub grievance_reports: Arc<RwLock<HashMap<Uuid, GrievanceReport>>>,
    pub grievance_adjudications: Arc<RwLock<HashMap<Uuid, GrievanceAdjudication>>>,
    pub transactions: Arc<RwLock<HashMap<Uuid, Transaction>>>,
    pub doctor_wallets: Arc<RwLock<HashMap<Uuid, DoctorWallet>>>,
    pub disbursement_batches: Arc<RwLock<HashMap<Uuid, DisbursementBatch>>>,
    pub disbursement_items: Arc<RwLock<HashMap<Uuid, Vec<DisbursementItem>>>>,
    pub audit_events: Arc<RwLock<Vec<AuditEvent>>>,
    pub idempotency_records: Arc<RwLock<HashMap<String, IdempotencyRecord>>>,
    pub phone_otps: Arc<RwLock<HashMap<String, (String, chrono::DateTime<Utc>)>>>,
}

impl AppState {
    pub fn new_with_seeds() -> Self {
        let state = Self::default();

        // Seed Doctors matching prototype
        let doc1_id = Uuid::parse_str("da000001-0000-0000-0000-000000000001").unwrap();
        let doc1_user = Uuid::parse_str("10000001-0000-0000-0000-000000000001").unwrap();
        let doc1 = DoctorProfile {
            id: doc1_id,
            user_id: doc1_user,
            full_name: "Dr. Sabrina Akter".into(),
            license_number: "BMDC #45821".into(),
            license_authority: "BMDC".into(),
            license_country: "BD".into(),
            primary_specialty: "Internal Medicine".into(),
            experience_years: 12,
            current_hospital: "Dhaka Medical College Hospital".into(),
            qualifications: vec!["MBBS".into(), "FCPS".into(), "MD".into()],
            consultation_fee_video: BigDecimal::from(800),
            consultation_fee_chat: BigDecimal::from(500),
            residential_address: "Dhanmondi, Dhaka (House 42, Road 7A)".into(),
            verified_phone: "+880 1711-884920".into(),
            is_on_duty: true,
            is_verified: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        };

        let doc2_id = Uuid::parse_str("da000002-0000-0000-0000-000000000002").unwrap();
        let doc2_user = Uuid::parse_str("10000002-0000-0000-0000-000000000002").unwrap();
        let doc2 = DoctorProfile {
            id: doc2_id,
            user_id: doc2_user,
            full_name: "Dr. Anika Rahman".into(),
            license_number: "BMDC #52891".into(),
            license_authority: "BMDC".into(),
            license_country: "BD".into(),
            primary_specialty: "Cardiology".into(),
            experience_years: 9,
            current_hospital: "National Institute of Cardiovascular Diseases".into(),
            qualifications: vec!["MBBS".into(), "MD (Cardiology)".into()],
            consultation_fee_video: BigDecimal::from(1000),
            consultation_fee_chat: BigDecimal::from(600),
            residential_address: "Gulshan-2, Dhaka".into(),
            verified_phone: "+880 1712-445566".into(),
            is_on_duty: true,
            is_verified: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        };

        state.doctor_profiles.write().insert(doc1_id, doc1.clone());
        state.doctor_profiles.write().insert(doc2_id, doc2.clone());

        // Initialize doctor wallets
        state.doctor_wallets.write().insert(
            doc1_id,
            DoctorWallet {
                doctor_id: doc1_id,
                lifetime_gross: BigDecimal::from(0),
                lifetime_fee_withheld: BigDecimal::from(0),
                lifetime_net: BigDecimal::from(0),
                pending_disbursement: BigDecimal::from(0),
                last_disbursement_at: None,
                last_disbursement_amount: BigDecimal::from(0),
                updated_at: Utc::now(),
            },
        );

        state.doctor_wallets.write().insert(
            doc2_id,
            DoctorWallet {
                doctor_id: doc2_id,
                lifetime_gross: BigDecimal::from(0),
                lifetime_fee_withheld: BigDecimal::from(0),
                lifetime_net: BigDecimal::from(0),
                pending_disbursement: BigDecimal::from(0),
                last_disbursement_at: None,
                last_disbursement_amount: BigDecimal::from(0),
                updated_at: Utc::now(),
            },
        );

        // Seed available slots for today and tomorrow
        let now = Utc::now();
        for i in 1..=5 {
            let slot_id = Uuid::new_v4();
            let start = now + Duration::hours(i);
            let end = start + Duration::minutes(15);
            state.schedule_slots.write().insert(
                slot_id,
                ScheduleSlot {
                    id: slot_id,
                    doctor_id: doc1_id,
                    start_time: start,
                    end_time: end,
                    status: SlotStatus::Available,
                    created_at: now,
                },
            );
        }

        state
    }

    /// Atomically lock an available slot for payment
    pub fn lock_slot(&self, slot_id: Uuid, doctor_id: Uuid) -> Result<(), AppError> {
        let mut slots = self.schedule_slots.write();
        let slot = slots.get_mut(&slot_id).ok_or_else(|| {
            AppError::NotFound("Consultation schedule slot not found.".into())
        })?;

        if slot.doctor_id != doctor_id {
            return Err(AppError::Conflict("Slot does not belong to specified doctor.".into()));
        }

        if slot.status != SlotStatus::Available {
            return Err(AppError::Conflict("Selected time slot is already locked or booked.".into()));
        }

        slot.status = SlotStatus::LockedInPayment;
        Ok(())
    }

    /// Confirm booked slot upon successful payment webhook
    pub fn confirm_slot(&self, slot_id: Uuid) -> Result<(), AppError> {
        let mut slots = self.schedule_slots.write();
        let slot = slots.get_mut(&slot_id).ok_or_else(|| {
            AppError::NotFound("Consultation schedule slot not found.".into())
        })?;

        slot.status = SlotStatus::Booked;
        Ok(())
    }

    /// Release slot upon payment timeout or failure
    pub fn release_slot(&self, slot_id: Uuid) -> Result<(), AppError> {
        let mut slots = self.schedule_slots.write();
        if let Some(slot) = slots.get_mut(&slot_id) {
            if slot.status == SlotStatus::LockedInPayment {
                slot.status = SlotStatus::Available;
            }
        }
        Ok(())
    }
}
