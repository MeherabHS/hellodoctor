use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{consultation_repo, doctor_repo, transaction_repo, wallet_repo, AppState};
use crate::services::agora_token_service::{AgoraTokenResponse, AgoraTokenService};
use chrono::Utc;
use uuid::Uuid;

pub struct ConsultationService;

impl ConsultationService {
    /// Mint secure, unpredictable Agora RTC Token (server-side only)
    pub async fn mint_agora_token(
        state: &AppState,
        appointment_id: Uuid,
        user_id: Uuid,
        agora_app_id: &str,
        agora_certificate: &str,
    ) -> Result<AgoraTokenResponse, AppError> {
        let apt = crate::repository::appointment_repo::find_by_id(&state.db, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment record not found.".into()))?;

        let is_patient = apt.patient_id == user_id;
        let is_doctor = doctor_repo::find_by_id(&state.db, apt.doctor_id)
            .await?
            .map(|d| d.user_id == user_id)
            .unwrap_or(false);

        if !is_patient && !is_doctor {
            return Err(AppError::Forbidden(
                "Caller is not an authorized participant in this consultation.".into(),
            ));
        }

        if apt.status != AppointmentStatus::Confirmed && apt.status != AppointmentStatus::InConsultation {
            return Err(AppError::Conflict(
                "Consultation room is only accessible for CONFIRMED appointments.".into(),
            ));
        }

        let session = match consultation_repo::find_by_appointment(&state.db, appointment_id).await? {
            Some(s) => s,
            None => {
                let s = ConsultationSession {
                    id: Uuid::new_v4(),
                    appointment_id,
                    agora_channel_name: format!("rtc_{}", Uuid::new_v4().to_string().replace('-', "")),
                    started_at: Some(Utc::now()),
                    ended_at: None,
                    status: SessionStatus::Active,
                    created_at: Utc::now(),
                };
                consultation_repo::insert_session(&state.db, &s).await?;
                s
            }
        };

        AgoraTokenService::mint(&session.agora_channel_name, user_id, agora_app_id, agora_certificate)
    }

    /// Record session telemetry received from Agora onRtcStats
    pub async fn record_telemetry(
        state: &AppState,
        appointment_id: Uuid,
        call_duration_seconds: i32,
        connection_state: String,
        prescription_issued: bool,
    ) -> Result<ConsultationTelemetry, AppError> {
        let session = consultation_repo::find_by_appointment(&state.db, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Consultation session not found.".into()))?;

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

        consultation_repo::insert_telemetry(&state.db, &telemetry).await?;
        Ok(telemetry)
    }

    /// Complete consultation and settle payment to doctor's wallet
    pub async fn complete_consultation(state: &AppState, appointment_id: Uuid, outcome: ClinicalOutcome) -> Result<(), AppError> {
        let mut tx = state.db.begin().await?;

        let apt = crate::repository::appointment_repo::find_by_id(&mut *tx, appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

        crate::repository::appointment_repo::complete(&mut *tx, appointment_id, outcome.as_db_str()).await?;

        if let Some(session) = consultation_repo::find_by_appointment(&mut *tx, appointment_id).await? {
            consultation_repo::set_status(&mut *tx, session.id, SessionStatus::Completed).await?;
        }

        if let Some(txn) = transaction_repo::find_by_appointment(&mut *tx, appointment_id).await? {
            if txn.payment_status == PaymentStatus::PaymentHeld {
                transaction_repo::mark_settled(&mut *tx, txn.id).await?;
                wallet_repo::credit(&mut *tx, apt.doctor_id, &txn.gross_amount, &txn.platform_fee_amount, &txn.net_amount).await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }
}
