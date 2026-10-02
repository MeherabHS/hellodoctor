use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

pub struct AgoraTokenResponse {
    pub channel_name: String,
    pub token: String,
    pub uid: u32,
    pub app_id: String,
    pub expires_in: u32,
}

pub struct ConsultationService;

impl ConsultationService {
    /// Mint secure, unpredictable Agora RTC Token (server-side only)
    pub fn mint_agora_token(
        state: &AppState,
        appointment_id: Uuid,
        user_id: Uuid,
        agora_app_id: &str,
        agora_certificate: &str,
    ) -> Result<AgoraTokenResponse, AppError> {
        let apts = state.appointments.read();
        let apt = apts.get(&appointment_id).ok_or_else(|| {
            AppError::NotFound("Appointment record not found.".into())
        })?;

        // Authorization check: Must be the assigned patient or doctor
        let is_patient = apt.patient_id == user_id;
        let is_doctor = {
            let docs = state.doctor_profiles.read();
            docs.get(&apt.doctor_id).map(|d| d.user_id == user_id).unwrap_or(false)
        };

        if !is_patient && !is_doctor {
            return Err(AppError::Forbidden("Caller is not an authorized participant in this consultation.".into()));
        }

        if apt.status != AppointmentStatus::Confirmed && apt.status != AppointmentStatus::InConsultation {
            return Err(AppError::Conflict("Consultation room is only accessible for CONFIRMED appointments.".into()));
        }

        // Fetch or create session with unpredictable channel UUID
        let mut sessions = state.consultation_sessions.write();
        let session = sessions.entry(appointment_id).or_insert_with(|| {
            ConsultationSession {
                id: Uuid::new_v4(),
                appointment_id,
                agora_channel_name: format!("rtc_{}", Uuid::new_v4().to_string().replace('-', "")),
                started_at: Some(Utc::now()),
                ended_at: None,
                status: SessionStatus::Active,
                created_at: Utc::now(),
            }
        });

        // Numerical UID derived from user UUID
        let uid = (user_id.as_u128() % 100_000) as u32;

        // Generate HMAC-SHA256 Agora Token signature
        let mut mac = Hmac::<Sha256>::new_from_slice(agora_certificate.as_bytes())
            .map_err(|e| AppError::Internal(format!("HMAC error: {}", e)))?;
        let message = format!("{}:{}:{}", agora_app_id, session.agora_channel_name, uid);
        mac.update(message.as_bytes());
        let token = format!("007eJx{}", hex::encode(mac.finalize().into_bytes()));

        Ok(AgoraTokenResponse {
            channel_name: session.agora_channel_name.clone(),
            token,
            uid,
            app_id: agora_app_id.to_string(),
            expires_in: 3600,
        })
    }

    /// Record session telemetry received from Agora onRtcStats
    pub fn record_telemetry(
        state: &AppState,
        appointment_id: Uuid,
        call_duration_seconds: i32,
        connection_state: String,
        prescription_issued: bool,
    ) -> Result<ConsultationTelemetry, AppError> {
        let sessions = state.consultation_sessions.read();
        let session = sessions.get(&appointment_id).ok_or_else(|| {
            AppError::NotFound("Consultation session not found.".into())
        })?;

        let premature_end = call_duration_seconds < 30;

        let telemetry = ConsultationTelemetry {
            id: Uuid::new_v4(),
            session_id: session.id,
            call_duration_seconds,
            connection_state,
            packet_loss_percent: None,
            round_trip_time_ms: None,
            premature_end,
            prescription_issued,
            captured_at: Utc::now(),
        };

        state.consultation_telemetries.write().insert(session.id, telemetry.clone());
        Ok(telemetry)
    }

    /// Complete consultation and settle payment to doctor's wallet
    pub fn complete_consultation(
        state: &AppState,
        appointment_id: Uuid,
        outcome: ClinicalOutcome,
    ) -> Result<(), AppError> {
        // 1. Mark appointment COMPLETED
        let (doctor_id, _fee) = {
            let mut apts = state.appointments.write();
            let apt = apts.get_mut(&appointment_id).ok_or_else(|| {
                AppError::NotFound("Appointment not found.".into())
            })?;
            apt.status = AppointmentStatus::Completed;
            apt.clinical_outcome = Some(outcome);
            apt.completed_at = Some(Utc::now());
            apt.updated_at = Utc::now();
            (apt.doctor_id, apt.consultation_fee.clone())
        };

        // 2. Locate transaction and settle to doctor
        let mut txns = state.transactions.write();
        if let Some(txn) = txns.values_mut().find(|t| t.appointment_id == appointment_id) {
            if txn.payment_status == PaymentStatus::PaymentHeld {
                txn.payment_status = PaymentStatus::SettledToDoctor;
                txn.settled_at = Some(Utc::now());

                // 3. Credit doctor's wallet
                let mut wallets = state.doctor_wallets.write();
                if let Some(wallet) = wallets.get_mut(&doctor_id) {
                    wallet.lifetime_gross += &txn.gross_amount;
                    wallet.lifetime_fee_withheld += &txn.platform_fee_amount;
                    wallet.lifetime_net += &txn.net_amount;
                    wallet.pending_disbursement += &txn.net_amount;
                    wallet.updated_at = Utc::now();
                }
            }
        }

        Ok(())
    }
}
