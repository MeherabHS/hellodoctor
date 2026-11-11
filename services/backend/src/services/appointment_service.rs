use super::consultation_trace::{self, Event};
use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId};
use crate::domain::models::*;
use crate::error::AppError;
use crate::repository::{
    appointment_repo, chat_repo, consultation_repo, doctor_repo, email_outbox_repo,
    payment_event_repo, payment_methods_repo, transaction_repo, AppState, DbRole,
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use std::str::FromStr;
use uuid::Uuid;

pub struct AppointmentBookingResult {
    pub appointment_id: Uuid,
    pub appointment_number: String,
    pub status: AppointmentStatus,
    pub payment_policy: PaymentPolicy,
    /// `None` for a `FinancialModuleMode::Deferred` booking — no checkout
    /// session or transaction was ever created, so there is nothing to
    /// redirect the patient to pay.
    pub payment_session_id: Option<Uuid>,
    pub payment_redirect_url: Option<String>,
    pub amount: BigDecimal,
    /// `None` for a deferred booking: it is `CONFIRMED` immediately, so
    /// there is no payment reservation deadline to report.
    pub hold_expires_at: Option<chrono::DateTime<Utc>>,
    pub server_time: chrono::DateTime<Utc>,
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
        gateway: Option<Gateway>,
        chief_complaint: Option<String>,
        payment_method_id: Option<Uuid>,
    ) -> Result<AppointmentBookingResult, AppError> {
        let deferred = state.config.financial_module_deferred();
        if !deferred && state.config.environment != "development" {
            return Err(AppError::Conflict("Payment collection is not configured. No checkout or confirmed payment was created.".into()));
        }
        let Some(gateway) = gateway else {
            if deferred {
                return Self::book_appointment_deferred(
                    state,
                    ctx,
                    patient_id,
                    doctor_id,
                    slot_id,
                    modality,
                    chief_complaint,
                )
                .await;
            }
            return Err(AppError::Validation(
                "Either gateway or payment_method_id is required.".into(),
            ));
        };
        let doctor = doctor_repo::find_public(&state.db, doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
        if doctor.professional_verification_status != ProfessionalVerificationStatus::Verified {
            return Err(AppError::Forbidden(
                "This Doctor is awaiting professional verification.".into(),
            ));
        }

        let mut tx = state.begin_authed_as(ctx, DbRole::PaymentWorker).await?;

        // INV-BOOK-02: serialize this Patient's concurrent bookings. Without
        // it, two bookings for overlapping slots with different Doctors each
        // hold their own slot lock and then wait on the other's uncommitted
        // exclusion-constraint tuple - a deadlock PostgreSQL resolves by
        // aborting one with a 500. Taken first, before any row lock.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("patient-booking:{}", uuid::Uuid::from(patient_id)))
            .execute(&mut *tx)
            .await?;

        // Lock order is always appointment -> slot (the payment webhook's
        // order). Reclaiming due holds *before* locking the slot keeps that
        // order; the previous lock-slot-then-reclaim order was inverted.
        // Both reclaims touch only holds whose deadline has already passed,
        // and are race-safe against a concurrent payment-success webhook (see
        // appointment_repo::expire_abandoned_hold_for_slot). The second one
        // frees this Patient's own abandoned checkout with another Doctor,
        // which would otherwise keep blocking through appt_patient_no_overlap.
        let window = appointment_repo::BOOKING_HOLD_SECONDS;
        appointment_repo::expire_abandoned_hold_for_slot(&mut tx, slot_id, window).await?;
        appointment_repo::expire_due_holds_overlapping_for_patient(
            &mut tx,
            patient_id.into(),
            slot_id,
            window,
        )
        .await?;

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

        if fee <= 0 {
            return Err(AppError::Conflict(
                "This consultation modality is not offered by the Doctor.".into(),
            ));
        }

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
            payment_policy: PaymentPolicy::Required,
            clinical_outcome: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let (hold_expires_at, server_time) =
            appointment_repo::insert_pending_hold(&mut *tx, &appointment, window).await?;

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

        // Re-validated here, not trusted from the handler alone (the same
        // BOLA-safe discipline every other ownership check in this service
        // already follows): a saved method must actually belong to this
        // patient and still be active, or booking fails outright rather
        // than silently falling back to the raw `gateway` the client sent.
        if let Some(method_id) = payment_method_id {
            let method = payment_methods_repo::find_patient_payment_method(&mut *tx, method_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound("No saved payment method with that id.".into())
                })?;
            if method.patient_id != patient_id || method.binding_status != "ACTIVE" {
                return Err(AppError::NotFound(
                    "No saved payment method with that id belongs to this profile.".into(),
                ));
            }
        }

        let payment_session_id = Uuid::new_v4();
        let txn_id = Uuid::new_v4();
        // Milestone 8 sustained-reliability finding: truncating to the
        // first 8 hex chars (32 bits) made a real collision on
        // transactions_transaction_number_key observable at ~9.4k
        // transactions in a single benchmark session, surfacing as an
        // unhandled 500 instead of a graceful outcome. The full UUID (122
        // random bits) makes a collision cryptographically negligible;
        // VARCHAR(50) has ample room for "TXN-" + 36 chars.
        let txn_num = format!("TXN-{}", txn_id.to_string().to_uppercase());

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
            reconciliation_required: false,
            created_at: Utc::now(),
            settled_at: None,
            currency: "KES".into(),
            disbursement_item_id: None,
            payment_method_id,
        };
        transaction_repo::insert(&mut *tx, &transaction).await?;
        consultation_trace::record(
            &mut tx,
            apt_id,
            None,
            Event::accepted("APPOINTMENT_CREATED", Some(("APPOINTMENT", apt_id))),
        )
        .await?;
        consultation_trace::record(
            &mut tx,
            apt_id,
            None,
            Event::accepted("PAYMENT_INITIATED", Some(("TRANSACTION", txn_id))),
        )
        .await?;

        tx.commit().await?;

        // Notification creation is post-commit so an inbox failure cannot
        // roll back an otherwise valid reservation. The scheduler retries the
        // same deduplicated event if this attempt is interrupted.
        if let Err(error) =
            crate::services::notification_scheduler::ensure_hold_created_notification(state, apt_id)
                .await
        {
            tracing::warn!(error = ?error, appointment_id = %apt_id, "booking hold notification deferred to scheduler");
        }

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
            payment_policy: PaymentPolicy::Required,
            payment_session_id: Some(payment_session_id),
            payment_redirect_url: Some(payment_redirect_url),
            amount: fee,
            hold_expires_at: Some(hold_expires_at),
            server_time,
        })
    }

    /// `FinancialModuleMode::Deferred` counterpart to the payment path above:
    /// same ownership/verification/concurrency/slot-lock discipline, zero
    /// `transactions` row, appointment is `CONFIRMED` immediately (never
    /// `PENDING_PAYMENT`). Never invents a paid status or a fabricated
    /// transaction — there simply isn't one.
    async fn book_appointment_deferred(
        state: &AppState,
        ctx: &AuthContext,
        patient_id: PatientProfileId,
        doctor_id: DoctorProfileId,
        slot_id: Uuid,
        modality: Modality,
        chief_complaint: Option<String>,
    ) -> Result<AppointmentBookingResult, AppError> {
        let doctor = doctor_repo::find_public(&state.db, doctor_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
        if doctor.professional_verification_status != ProfessionalVerificationStatus::Verified {
            return Err(AppError::Forbidden(
                "This Doctor is awaiting professional verification.".into(),
            ));
        }

        let mut tx = state.begin_authed_as(ctx, DbRole::PaymentWorker).await?;

        // Same INV-BOOK-02 patient-concurrency serialization as the paid
        // path, taken first, before any row lock.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("patient-booking:{}", uuid::Uuid::from(patient_id)))
            .execute(&mut *tx)
            .await?;

        // No payment hold ever exists for a deferred booking, so there is
        // nothing of this patient's or this slot's to reclaim here — unlike
        // the paid path, this is not a race condition to guard against.
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
        // Deliberately no `fee <= 0` rejection here: whether this modality
        // is "offered" is still decided by the doctor's configured fee
        // (unchanged, existing signal — see `consultationAction`/`supportsVideo`
        // across every client), but a deferred booking never charges that
        // fee regardless of its value, so a doctor who simply hasn't bothered
        // pricing a modality while payments are off must not be blocked from
        // offering it. Operationally, a Doctor should still configure a
        // nonzero fee for every modality they offer in deferred mode, purely
        // so existing availability checks keep recognizing it.
        if fee < 0 {
            return Err(AppError::Conflict(
                "This consultation modality is not offered by the Doctor.".into(),
            ));
        }

        let apt_id = Uuid::new_v4();
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
            status: AppointmentStatus::Confirmed,
            consultation_fee: fee.clone(),
            payment_policy: PaymentPolicy::NotRequired,
            clinical_outcome: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let server_time =
            appointment_repo::insert_confirmed_no_payment(&mut *tx, &appointment).await?;

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

        doctor_repo::set_slot_status(&mut *tx, slot_id, SlotStatus::Booked).await?;

        consultation_trace::record(
            &mut tx,
            apt_id,
            None,
            Event::accepted("APPOINTMENT_CREATED", Some(("APPOINTMENT", apt_id))),
        )
        .await?;
        consultation_trace::record(
            &mut tx,
            apt_id,
            None,
            Event::accepted(
                "BOOKING_CONFIRMED_NO_PAYMENT_REQUIRED",
                Some(("APPOINTMENT", apt_id)),
            ),
        )
        .await?;

        // Fired from inside this same CONFIRMED-creating transaction —
        // equally authoritative as the payment webhook's own call to this
        // function, never a speculative/optimistic client-side state, since
        // this booking never passes through PENDING_PAYMENT at all.
        crate::services::NotificationService::notify_booking_confirmed(&mut tx, apt_id).await?;

        tx.commit().await?;

        Ok(AppointmentBookingResult {
            appointment_id: apt_id,
            appointment_number: apt_num,
            status: AppointmentStatus::Confirmed,
            payment_policy: PaymentPolicy::NotRequired,
            payment_session_id: None,
            payment_redirect_url: None,
            amount: fee,
            hold_expires_at: None,
            server_time,
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
        if gateway_reference.trim().is_empty()
            || gateway_reference.chars().count() > 100
            || provider_event_id.trim().is_empty()
            || provider_event_id.chars().count() > 150
        {
            return Err(AppError::Validation(
                "Invalid payment event identity.".into(),
            ));
        }
        let ctx = AuthContext::system();

        let mut payment_tx = state.begin_authed_as(&ctx, DbRole::PaymentWorker).await?;
        let initial_txn =
            transaction_repo::find_by_payment_session(&mut *payment_tx, payment_session_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Transaction session not found.".into()))?;

        // Same period-first lock order as accounting close. Provider facts are
        // still recorded after a close, but explicitly flagged for reconciliation.
        sqlx::query("SELECT pg_advisory_xact_lock(61012,EXTRACT(YEAR FROM $1::timestamptz)::integer*100+EXTRACT(MONTH FROM $1::timestamptz)::integer)")
            .bind(initial_txn.created_at).execute(&mut *payment_tx).await?;

        // Completion/expiry take the appointment lock before changing payment
        // state. Use the same order and re-read after waiting for the lock.
        sqlx::query("SELECT id FROM appointments WHERE id=$1 FOR UPDATE")
            .bind(initial_txn.appointment_id)
            .fetch_one(&mut *payment_tx)
            .await?;
        sqlx::query("SELECT id FROM transactions WHERE id=$1 FOR UPDATE")
            .bind(initial_txn.id)
            .fetch_one(&mut *payment_tx)
            .await?;
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
        if txn
            .gateway_reference
            .as_deref()
            .is_some_and(|r| r != gateway_reference)
        {
            return Err(AppError::Conflict(
                "Payment reference does not match the recorded payment.".into(),
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

        if !is_new_event {
            let owner: Uuid = sqlx::query_scalar("SELECT transaction_id FROM payment_events WHERE gateway=$1 AND provider_event_id=$2")
                .bind(gateway).bind(provider_event_id).fetch_one(&mut *payment_tx).await?;
            if owner != txn.id {
                return Err(AppError::Conflict(
                    "Provider event belongs to another payment.".into(),
                ));
            }
        }
        let confirmed_now = if is_new_event {
            transaction_repo::mark_payment_held(&mut *payment_tx, txn.id, gateway_reference).await?
        } else {
            false
        };
        let mut newly_confirmed = false;
        let apt = if !confirmed_now {
            appointment_repo::find_by_id(&mut *payment_tx, txn.appointment_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?
        } else {
            let mut apt = appointment_repo::find_by_id(&mut *payment_tx, txn.appointment_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Appointment not found.".into()))?;

            // h23 remediation: race-safe against expire_abandoned_hold_for_slot.
            // If the reservation was already reclaimed (lost the race), do
            // NOT book the slot — it may already belong to someone else.
            let confirmed = appointment_repo::confirm_payment_if_pending(
                &mut *payment_tx,
                apt.id,
                appointment_repo::BOOKING_HOLD_SECONDS,
            )
            .await?;
            if confirmed {
                consultation_trace::record(
                    &mut payment_tx,
                    txn.appointment_id,
                    None,
                    Event::accepted("PAYMENT_CONFIRMED", Some(("TRANSACTION", txn.id))),
                )
                .await?;
                doctor_repo::set_slot_status(&mut *payment_tx, apt.slot_id, SlotStatus::Booked)
                    .await?;
                crate::services::NotificationService::notify_booking_confirmed(
                    &mut payment_tx,
                    txn.appointment_id,
                )
                .await?;
            } else {
                sqlx::query("UPDATE transactions SET reconciliation_required=true WHERE id=$1")
                    .bind(txn.id)
                    .execute(&mut *payment_tx)
                    .await?;
                crate::services::AuditService::log_in_tx(&mut *payment_tx,
                    crate::services::AuditLogEntry::new(None,ctx.role,"LATE_PAYMENT_RECONCILIATION_REQUIRED","TRANSACTION",Some(txn.id),"SUCCESS")
                        .with_after(serde_json::json!({"payment_status":"PAYMENT_HELD","reconciliation_required":true}))).await?;
                consultation_trace::record(
                    &mut payment_tx,
                    txn.appointment_id,
                    None,
                    Event::rejected(
                        "PAYMENT_CONFIRMED_AFTER_RESERVATION_EXPIRED",
                        "Payment succeeded but the booking hold had already expired and the slot was reclaimed; funds captured but not booked \u{2014} requires Finance reconciliation. No refund flow exists yet (no live payment provider is configured).",
                    ),
                )
                .await?;
            }
            if confirmed {
                apt.status = AppointmentStatus::Confirmed;
                newly_confirmed = true;
            }
            apt
        };

        // Phase 1B: atomic creation of both email intents alongside the
        // business event, on this same (payment-worker) transaction — see
        // `email_outbox_repo::enqueue_transactional`'s doc comment. Content
        // is rendered later, at dispatch time; a patient with no email on
        // file (e.g. phone/SMS-registered) simply gets no row enqueued for
        // either, same as the old inline check this replaces.
        let mut confirmation_outbox_ids: Vec<Uuid> = Vec::new();
        if newly_confirmed {
            let recipient_email: Option<String> = sqlx::query_scalar::<_, Option<String>>(
                "SELECT COALESCE(pu.email, gu.email) \
                   FROM appointments a \
                   JOIN patient_profiles p ON p.id = a.patient_id \
                   LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id \
                   LEFT JOIN users pu ON pu.id = p.user_id \
                   LEFT JOIN users gu ON gu.id = g.user_id \
                  WHERE a.id = $1",
            )
            .bind(apt.id)
            .fetch_one(&mut *payment_tx)
            .await?;
            if let Some(recipient_email) = recipient_email.filter(|e| !e.is_empty()) {
                let confirmed_id = Uuid::new_v4();
                email_outbox_repo::enqueue_transactional(
                    &mut *payment_tx,
                    confirmed_id,
                    "CONSULTATION_CONFIRMED",
                    &recipient_email,
                    &format!("consultation-confirmed:{}", apt.id),
                    "APPOINTMENT",
                    apt.id,
                    email_outbox_repo::transactional_email_expiry(),
                )
                .await?;
                confirmation_outbox_ids.push(confirmed_id);

                let payment_id = Uuid::new_v4();
                email_outbox_repo::enqueue_transactional(
                    &mut *payment_tx,
                    payment_id,
                    "PAYMENT_CONFIRMED",
                    &recipient_email,
                    &format!("payment-confirmed:{}", txn.id),
                    "TRANSACTION",
                    txn.id,
                    email_outbox_repo::transactional_email_expiry(),
                )
                .await?;
                confirmation_outbox_ids.push(payment_id);
            }
        }
        payment_tx.commit().await?;

        // Fire-and-forget, after commit (never awaited — must not delay this
        // already-committed booking/payment response). The durable fallback
        // is the periodic scheduler pass.
        if !confirmation_outbox_ids.is_empty() {
            let state = state.clone();
            tokio::spawn(async move {
                for outbox_id in confirmation_outbox_ids {
                    crate::services::email_dispatch_worker::dispatch_now(&state, outbox_id).await;
                }
            });
        }

        // Funds captured after a reservation expired require reconciliation.
        // Do not create a consultation for an expired/cancelled obligation.
        if !matches!(
            apt.status,
            AppointmentStatus::Confirmed
                | AppointmentStatus::InConsultation
                | AppointmentStatus::Completed
        ) {
            return Ok(apt);
        }

        // These existence checks must run under `begin_authed` (not the raw
        // pool) — both tables are FORCE RLS, and `ctx` here is the system
        // pseudo-identity (Admin), which every policy's admin branch
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
                    recovery_state: "NONE".into(),
                    empty_room_deadline: None,
                    patient_present: false,
                    doctor_present: false,
                    patient_presence_sequence: 0,
                    doctor_presence_sequence: 0,
                },
            )
            .await?;
            api_tx.commit().await?;
        }

        if !conversation_exists {
            let mut system_tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
                .bind(format!("activate-chat:{}", apt.id))
                .execute(&mut *system_tx)
                .await?;
            let already_active =
                chat_repo::find_conversation_by_appointment(&mut *system_tx, apt.id)
                    .await?
                    .is_some();
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
            let persisted = chat_repo::find_conversation_by_appointment(&mut *system_tx, apt.id)
                .await?
                .ok_or_else(|| AppError::internal("Conversation activation missing".into()))?;
            if !already_active {
                consultation_trace::record(
                    &mut system_tx,
                    apt.id,
                    Some(persisted.id),
                    Event::accepted("CONSULTATION_ACTIVATED", Some(("CHAT", persisted.id))),
                )
                .await?;
            }
            system_tx.commit().await?;
        }

        Ok(apt)
    }

    /// Phase 2 email support only — names/dates for a confirmed consultation,
    /// something `NotificationService`'s existing recipient-resolution SQL
    /// deliberately never carries (it only needs user ids for push routing).
    /// Reads on the system-worker pool, outside any transaction, strictly
    /// after the triggering payment transaction has already committed.
    /// Returns `None` only if the appointment/slot/profiles can't be found
    /// at all (should not happen for an appointment that was just
    /// confirmed) — never an error for "patient has no email," which is
    /// instead reflected by an empty `recipient_email` the caller checks.
    pub async fn consultation_email_details(
        state: &AppState,
        appointment_id: Uuid,
    ) -> Result<Option<ConsultationEmailDetails>, AppError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            patient_name: String,
            recipient_email: Option<String>,
            doctor_name: String,
            start_time: chrono::DateTime<Utc>,
            appointment_number: String,
        }
        let row: Option<Row> = sqlx::query_as(
            "SELECT p.full_name AS patient_name, \
                    COALESCE(pu.email, gu.email) AS recipient_email, \
                    d.full_name AS doctor_name, \
                    s.start_time AS start_time, \
                    a.appointment_number AS appointment_number \
               FROM appointments a \
               JOIN doctor_schedule_slots s ON s.id = a.slot_id \
               JOIN doctor_profiles d ON d.id = a.doctor_id \
               JOIN patient_profiles p ON p.id = a.patient_id \
               LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id \
               LEFT JOIN users pu ON pu.id = p.user_id \
               LEFT JOIN users gu ON gu.id = g.user_id \
              WHERE a.id = $1",
        )
        .bind(appointment_id)
        .fetch_optional(&state.system_db)
        .await?;
        Ok(row.map(|row| {
            let local = row
                .start_time
                .with_timezone(&crate::services::notification_service::nairobi_offset());
            ConsultationEmailDetails {
                patient_name: row.patient_name,
                recipient_email: row.recipient_email.unwrap_or_default(),
                doctor_name: row.doctor_name,
                appointment_number: row.appointment_number,
                date_label: local.format("%A, %d %B %Y").to_string(),
                time_label: local.format("%-I:%M %p").to_string(),
            }
        }))
    }
}

pub struct ConsultationEmailDetails {
    pub patient_name: String,
    pub recipient_email: String,
    pub doctor_name: String,
    pub appointment_number: String,
    pub date_label: String,
    pub time_label: String,
}
