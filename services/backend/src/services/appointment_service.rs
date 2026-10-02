use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::AppState;
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
    /// Atomic two-stage appointment booking
    pub fn book_appointment(
        state: &AppState,
        patient_id: Uuid,
        doctor_id: Uuid,
        slot_id: Uuid,
        modality: Modality,
        gateway: Gateway,
        chief_complaint: Option<String>,
    ) -> Result<AppointmentBookingResult, AppError> {
        // 1. Atomically lock the slot (SELECT FOR UPDATE equivalent)
        state.lock_slot(slot_id, doctor_id)?;

        // 2. Look up doctor fee
        let doctor = {
            let docs = state.doctor_profiles.read();
            docs.get(&doctor_id).cloned().ok_or_else(|| {
                state.release_slot(slot_id).ok();
                AppError::NotFound("Doctor profile not found.".into())
            })?
        };

        let fee = match modality {
            Modality::Video => doctor.consultation_fee_video.clone(),
            Modality::Chat => doctor.consultation_fee_chat.clone(),
        };

        // 3. Compute 20% platform fee debarment
        let platform_fee = &fee * BigDecimal::from_str("0.20").unwrap();
        let net_doctor_amount = &fee - &platform_fee;

        // 4. Create appointment with status PENDING_PAYMENT
        let apt_id = Uuid::new_v4();
        let apt_num = format!("APT-{}-{}", Utc::now().format("%Y%m%d"), &apt_id.to_string()[0..4].to_uppercase());

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

        state.appointments.write().insert(apt_id, appointment);

        // 5. Store PHI clinical complaint in separate clinical intake table
        state.clinical_intakes.write().insert(
            apt_id,
            AppointmentClinicalIntake {
                appointment_id: apt_id,
                patient_id,
                chief_complaint,
                clinical_notes: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );

        // 6. Persist initial transaction record with status INITIATED before redirecting
        let payment_session_id = Uuid::new_v4();
        let txn_id = Uuid::new_v4();
        let txn_num = format!("TXN-{}", &txn_id.to_string()[0..8].to_uppercase());

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

        state.transactions.write().insert(txn_id, transaction);

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
    pub fn process_payment_webhook(
        state: &AppState,
        payment_session_id: Uuid,
        gateway_reference: &str,
    ) -> Result<Appointment, AppError> {
        // 1. Locate transaction
        let mut txns = state.transactions.write();
        let txn = txns
            .values_mut()
            .find(|t| t.payment_session_id == payment_session_id)
            .ok_or_else(|| AppError::NotFound("Transaction session not found.".into()))?;

        if txn.payment_status == PaymentStatus::PaymentHeld {
            // Idempotent: already confirmed
            let apts = state.appointments.read();
            return apts
                .get(&txn.appointment_id)
                .cloned()
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()));
        }

        // 2. Update transaction status to PAYMENT_HELD
        txn.payment_status = PaymentStatus::PaymentHeld;
        txn.gateway_reference = Some(gateway_reference.to_string());
        let apt_id = txn.appointment_id;

        // 3. Confirm appointment and lock slot to BOOKED
        let mut apts = state.appointments.write();
        let apt = apts.get_mut(&apt_id).ok_or_else(|| {
            AppError::NotFound("Appointment not found.".into())
        })?;

        apt.status = AppointmentStatus::Confirmed;
        apt.updated_at = Utc::now();

        // 4. Update slot to BOOKED
        state.confirm_slot(apt.slot_id)?;

        // 5. Instantiate Consultation Session for Agora RTC
        let channel_name = format!("rtc_{}", Uuid::new_v4().to_string().replace('-', ""));
        state.consultation_sessions.write().insert(
            apt_id,
            ConsultationSession {
                id: Uuid::new_v4(),
                appointment_id: apt_id,
                agora_channel_name: channel_name,
                started_at: None,
                ended_at: None,
                status: SessionStatus::Initialized,
                created_at: Utc::now(),
            },
        );

        // 6. Instantiate 24-hour Clinical Chat Conversation
        state.chat_conversations.write().insert(
            apt_id,
            ChatConversation {
                id: Uuid::new_v4(),
                appointment_id: apt_id,
                patient_id: apt.patient_id,
                doctor_id: apt.doctor_id,
                expires_at: Utc::now() + chrono::Duration::hours(24),
                is_locked: false,
                created_at: Utc::now(),
            },
        );

        Ok(apt.clone())
    }
}
