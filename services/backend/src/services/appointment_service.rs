use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{
    appointment_repo, chat_repo, consultation_repo, doctor_repo, payment_event_repo,
    transaction_repo, AppState, DbRole,
};
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
    /// Booking reads only the safe public professional projection; the
    /// slot lock, appointment, clinical intake, and initial transaction record
    /// are booking/payment lifecycle state (see AUTH_SECURITY.md §12.3) and so
    /// run together, atomically, on the payment-worker pool — the only role
    /// granted write access to appointments/doctor_schedule_slots/transactions.
    #[allow(clippy::too_many_arguments)]
    pub async fn book_appointment(
        state: &AppState,
        ctx: &AuthContext,
        patient_id: PatientProfileId,
        doctor_id: DoctorProfileId,
        slot_id: Uuid,
        modality: Modality,
        gateway: Gateway,
        chief_complaint: Option<String>,
    ) -> Result<AppointmentBookingResult, AppError> {
        if state.config.environment != "development" {
            return Err(AppError::Conflict("Payment collection is not configured. No checkout or confirmed payment was created.".into()));
        }
        let doctor = doctor_repo::find_public(&state.db, doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
        if doctor.professional_verification_status != ProfessionalVerificationStatus::Verified {
            return Err(AppError::Forbidden(
                "This Doctor is awaiting professional verification.".into(),
            ));
        }

        let mut tx = state.begin_authed_as(ctx, DbRole::PaymentWorker).await?;

        let locked = doctor_repo::try_lock_slot(&mut tx, slot_id, doctor_id).await?;
        if !locked {
            return Err(AppError::Conflict(
                "Selected time slot is already locked or booked, or does not belong to the specified doctor.".into(),
            ));
        }

        let fee = match modality {
            Modality::Video => doctor.consultation_fee_video.clone(),
            Modality::Chat => doctor.consultation_fee_chat.clone(),
        };

        // Compute 20% platform fee debarment
        let platform_fee = &fee * BigDecimal::from_str("0.20").unwrap();
        let net_doctor_amount = &fee - &platform_fee;

        let apt_id = Uuid::new_v4();
        // The first 8 hex chars of a v4 UUID (32 bits of randomness), not 4 (16 bits,
        // only 65,536 values/day) — the narrower version was confirmed to collide for
        // real under load-test concurrency (`appointments_appointment_number_key`
        // unique-violation, 500 to the caller, after roughly 100 same-day bookings —
        // exactly the birthday-paradox range for a 65,536-value space).
        let apt_num = format!(
            "APT-{}-{}",
            Utc::now().format("%Y%m%d"),
            apt_id.to_string()[0..8].to_uppercase()
        );

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
            currency: "KES".into(),
            disbursement_item_id: None,
        };
        transaction_repo::insert(&mut *tx, &transaction).await?;

        tx.commit().await?;

        let gateway_str = match gateway {
            Gateway::Bkash => "bkash",
            Gateway::Nagad => "nagad",
            Gateway::Card => "card",
            Gateway::Mpesa => "mpesa",
            Gateway::BankTransfer => "bank-transfer",
        };

        let payment_redirect_url = format!(
            "https://payment-provider-not-configured.invalid/{}/sess_{}",
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

    /// Process payment provider webhook callback. Runs as the trusted system identity
    /// (see `AuthContext::system`) — the gateway has no patient/doctor session of its own,
    /// and the HMAC signature check in `webhook_handlers` is what authorizes this call.
    ///
    /// Spans three capability-role pools: no single role is granted write access
    /// to transactions/appointments/doctor_schedule_slots (payment-worker),
    /// consultation_sessions (ordinary API role), and chat_conversations
    /// (system-worker) all at once (see migrations/20261003000001_rls_hardening).
    /// Because this is webhook-triggered and gateways retry undelivered
    /// callbacks, every step below is written find-or-create rather than
    /// assuming a single pass completes everything — a retry after a partial
    /// failure (e.g. session created but conversation insert crashed) finishes
    /// the missing piece instead of silently no-op'ing on the transaction's
    /// already-PAYMENT_HELD status.
    ///
    /// `provider_event_id` is additionally deduplicated against the
    /// pre-existing unique `(gateway, payment_events.provider_event_id)`
    /// index: a literal replay of the same event id is detected at the
    /// database layer and treated identically to the already-PAYMENT_HELD
    /// case below (find, don't redo), so a cryptographically-valid replayed
    /// callback has zero *additional* financial effect regardless of which
    /// of the two dedup mechanisms would have caught it on its own.
    pub async fn process_payment_webhook(
        state: &AppState,
        payment_session_id: Uuid,
        gateway: Gateway,
        gateway_reference: &str,
        provider_event_id: &str,
    ) -> Result<Appointment, AppError> {
        let ctx = AuthContext::system();

        let mut payment_tx = state.begin_authed_as(&ctx, DbRole::PaymentWorker).await?;
        let txn = transaction_repo::find_by_payment_session(&mut *payment_tx, payment_session_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Transaction session not found.".into()))?;

        // The provider path is untrusted routing input. A valid HMAC from one
        // provider must never confirm a transaction opened against another
        // provider. Keep this check inside the payment-worker transaction,
        // next to the transaction lookup, so the state transition cannot be
        // separated from the invariant it protects.
        if txn.gateway != gateway {
            return Err(AppError::Conflict(
                "Payment provider does not match the transaction gateway.".into(),
            ));
        }

        let is_new_event = payment_event_repo::try_record_event(
            &mut *payment_tx,
            txn.id,
            gateway,
            provider_event_id,
            "PAYMENT_SUCCESS",
        )
        .await?;

        let mut apt = if !is_new_event || txn.payment_status == PaymentStatus::PaymentHeld {
            appointment_repo::find_by_id(&mut *payment_tx, txn.appointment_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?
        } else {
            transaction_repo::mark_payment_held(&mut *payment_tx, txn.id, gateway_reference)
                .await?;

            let apt = appointment_repo::find_by_id(&mut *payment_tx, txn.appointment_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

            appointment_repo::set_status(&mut *payment_tx, apt.id, AppointmentStatus::Confirmed)
                .await?;
            doctor_repo::set_slot_status(&mut *payment_tx, apt.slot_id, SlotStatus::Booked).await?;
            apt
        };
        payment_tx.commit().await?;
        apt.status = AppointmentStatus::Confirmed;

        // These existence checks must run under `begin_authed` (not the raw
        // pool) — both tables are FORCE RLS, and `ctx` here is the system
        // pseudo-identity (PlatformAdmin), which every policy's admin branch
        // allows through. A raw-pool query would see zero rows unconditionally
        // and every retry would attempt (and fail on the unique constraint) a
        // duplicate insert instead of recognizing the existing row.
        let mut check_tx = state.begin_authed(&ctx).await?;
        let session_exists = consultation_repo::find_by_appointment(&mut *check_tx, apt.id)
            .await?
            .is_some();
        let conversation_exists =
            chat_repo::find_conversation_by_appointment(&mut *check_tx, apt.id)
                .await?
                .is_some();
        check_tx.commit().await?;

        if !session_exists {
            let mut api_tx = state.begin_authed(&ctx).await?;
            let channel_name = format!("rtc_{}", Uuid::new_v4().to_string().replace('-', ""));
            consultation_repo::insert_session(
                &mut *api_tx,
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
            api_tx.commit().await?;
        }

        if !conversation_exists {
            let mut system_tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
            chat_repo::insert_conversation(
                &mut *system_tx,
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
            system_tx.commit().await?;
        }

        Ok(apt)
    }
}
