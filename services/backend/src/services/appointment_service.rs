use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{appointment_repo, chat_repo, consultation_repo, doctor_repo, transaction_repo, AppState};
use bigdecimal::BigDecimal;
use chrono::Utc;
use std::str::FromStr;
use uuid::Uuid;

pub struct AppointmentBookingResult {
    pub appointment_id: Uuid,
    pub appointment_number: String,
    pub status: AppointmentStatus,
    pub payment_session_id: Uuid,
    pub payment_redirect_url: String,
    pub amount: BigDecimal,
}

pub struct AppointmentService;

impl AppointmentService {
    /// Atomic two-stage appointment booking. The slot lock, appointment, clinical intake,
    /// and initial transaction record are all committed in a single DB transaction.
    pub async fn book_appointment(
        state: &AppState,
        patient_id: Uuid,
        doctor_id: Uuid,
        slot_id: Uuid,
        modality: Modality,
        gateway: Gateway,
        chief_complaint: Option<String>,
    ) -> Result<AppointmentBookingResult, AppError> {
        let mut tx = state.db.begin().await?;

        let locked = doctor_repo::try_lock_slot(&mut tx, slot_id, doctor_id).await?;
        if !locked {
            return Err(AppError::Conflict(
                "Selected time slot is already locked or booked, or does not belong to the specified doctor.".into(),
            ));
        }

        let doctor = doctor_repo::find_by_id(&mut *tx, doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;

        let fee = match modality {
            Modality::Video => doctor.consultation_fee_video.clone(),
            Modality::Chat => doctor.consultation_fee_chat.clone(),
        };

        // Compute 20% platform fee debarment
        let platform_fee = &fee * BigDecimal::from_str("0.20").unwrap();
        let net_doctor_amount = &fee - &platform_fee;

        let apt_id = Uuid::new_v4();
        let apt_num = format!("APT-{}-{}", Utc::now().format("%Y%m%d"), apt_id.to_string()[0..4].to_uppercase());

        let appointment = Appointment {
            id: apt_id,
            appointment_number: apt_num.clone(),
            patient_id,
            doctor_id,
            slot_id,
            modality,
            status: AppointmentStatus::PendingPayment,
            consultation_fee: fee.clone(),
            clinical_outcome: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        appointment_repo::insert(&mut *tx, &appointment).await?;

        appointment_repo::insert_clinical_intake(
            &mut *tx,
            &AppointmentClinicalIntake {
                appointment_id: apt_id,
                patient_id,
                chief_complaint,
                clinical_notes: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        )
        .await?;

        let payment_session_id = Uuid::new_v4();
        let txn_id = Uuid::new_v4();
        let txn_num = format!("TXN-{}", txn_id.to_string()[0..8].to_uppercase());

        let transaction = Transaction {
            id: txn_id,
            transaction_number: txn_num,
            appointment_id: apt_id,
            payment_session_id,
            gateway,
            gateway_reference: None,
            gross_amount: fee.clone(),
            platform_fee_amount: platform_fee,
            net_amount: net_doctor_amount,
            payment_status: PaymentStatus::Initiated,
            created_at: Utc::now(),
            settled_at: None,
        };
        transaction_repo::insert(&mut *tx, &transaction).await?;

        tx.commit().await?;

        let gateway_str = match gateway {
            Gateway::Bkash => "bkash",
            Gateway::Nagad => "nagad",
            Gateway::Card => "card",
            Gateway::Mpesa => "mpesa",
        };

        let payment_redirect_url = format!(
            "https://gateway.{}.com/pay/sess_{}",
            gateway_str,
            payment_session_id.to_string().replace('-', "")
        );

        Ok(AppointmentBookingResult {
            appointment_id: apt_id,
            appointment_number: apt_num,
            status: AppointmentStatus::PendingPayment,
            payment_session_id,
            payment_redirect_url,
            amount: fee,
        })
    }

    /// Process payment provider webhook callback
    pub async fn process_payment_webhook(
        state: &AppState,
        payment_session_id: Uuid,
        gateway_reference: &str,
    ) -> Result<Appointment, AppError> {
        let mut tx = state.db.begin().await?;

        let txn = transaction_repo::find_by_payment_session(&mut *tx, payment_session_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction session not found.".into()))?;

        if txn.payment_status == PaymentStatus::PaymentHeld {
            // Idempotent: already confirmed
            let apt = appointment_repo::find_by_id(&mut *tx, txn.appointment_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;
            tx.commit().await?;
            return Ok(apt);
        }

        transaction_repo::mark_payment_held(&mut *tx, txn.id, gateway_reference).await?;

        let mut apt = appointment_repo::find_by_id(&mut *tx, txn.appointment_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

        appointment_repo::set_status(&mut *tx, apt.id, AppointmentStatus::Confirmed).await?;
        doctor_repo::set_slot_status(&mut *tx, apt.slot_id, SlotStatus::Booked).await?;

        let channel_name = format!("rtc_{}", Uuid::new_v4().to_string().replace('-', ""));
        consultation_repo::insert_session(
            &mut *tx,
            &ConsultationSession {
                id: Uuid::new_v4(),
                appointment_id: apt.id,
                agora_channel_name: channel_name,
                started_at: None,
                ended_at: None,
                status: SessionStatus::Initialized,
                created_at: Utc::now(),
            },
        )
        .await?;

        chat_repo::insert_conversation(
            &mut *tx,
            &ChatConversation {
                id: Uuid::new_v4(),
                appointment_id: apt.id,
                patient_id: apt.patient_id,
                doctor_id: apt.doctor_id,
                expires_at: Utc::now() + chrono::Duration::hours(24),
                is_locked: false,
                created_at: Utc::now(),
            },
        )
        .await?;

        tx.commit().await?;

        apt.status = AppointmentStatus::Confirmed;
        Ok(apt)
    }
}
