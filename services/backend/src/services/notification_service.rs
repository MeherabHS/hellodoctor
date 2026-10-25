//! Doctor Engagement extension: the one coherent notification domain.
//! Every creation path (admin verify/reject, consultation-completion earning
//! credit, the daily-summary/T-15-reminder scheduler) goes through this
//! module rather than each feature hand-rolling its own notification logic.
//!
//! Every `title`/`body` here is a short, backend-constructed template with
//! only non-clinical values interpolated (counts, amounts, currencies) —
//! never symptoms, diagnosis, prescription detail, or patient-identifying
//! clinical content. These are plain backend-generated strings, not arbitrary
//! user input, so (unlike free-text fields a person types) they don't go
//! through `sanitize::sanitize_text` — there is nothing for that to protect
//! against here, the same reasoning already applied to admin audit
//! before/after snapshots elsewhere in this codebase.

use crate::auth_context::AuthContext;
use crate::domain::ids::{PatientProfileId, UserId};
use crate::domain::models::Notification;
use crate::error::AppError;
use crate::repository::{notification_repo, AppState};
use bigdecimal::BigDecimal;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use uuid::Uuid;

/// East Africa Time is a fixed UTC+3 offset year-round — Kenya does not
/// observe daylight saving — so a `chrono_tz`/IANA-tzdata dependency isn't
/// needed just to compute "today" correctly for the daily summary; a fixed
/// offset is exact, not an approximation, for this specific deployment
/// timezone.
pub fn nairobi_offset() -> FixedOffset {
    FixedOffset::east_opt(3 * 3600).expect("UTC+3 is always a valid fixed offset")
}

pub fn nairobi_local_date(at: DateTime<Utc>) -> NaiveDate {
    at.with_timezone(&nairobi_offset()).date_naive()
}

pub struct NotificationService;

/// Own-settings view returned by `NotificationService::get_preferences` —
/// quiet-hours times as plain "HH:MM" strings (not raw `NaiveTime`) so the
/// Flutter client never has to parse a locale-dependent chrono format.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationPreferencesView {
    pub operational_enabled: bool,
    pub engagement_enabled: bool,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_start_local: String,
    pub quiet_hours_end_local: String,
    pub preferred_language: String,
    /// Milestone 4/5: good-morning/birthday/Kenyan-holiday greetings — a
    /// stricter, separate opt-in from `engagement_enabled` (role-aware
    /// default: Doctors on, Patients off — see
    /// `notification_repo::user_notification_preferences`).
    pub greeting_notifications_enabled: bool,
    /// Milestone 5: religious-origin holidays only (Good Friday, Easter
    /// Monday, Christmas, Eid al-Fitr, Eid al-Adha) — off by default for
    /// everyone, never inferred from role/name/ethnicity.
    pub religious_greetings_enabled: bool,
}

/// Input to `NotificationService::update_preferences`, bundled into one
/// struct purely to stay under clippy's argument-count lint.
#[derive(Debug, Clone)]
pub struct UpdatePreferencesInput {
    pub operational_enabled: bool,
    pub engagement_enabled: bool,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_start_local: chrono::NaiveTime,
    pub quiet_hours_end_local: chrono::NaiveTime,
    pub greeting_notifications_enabled: bool,
    pub religious_greetings_enabled: bool,
    pub language: Option<String>,
}

/// One notification recipient together with the language their content
/// should render in (`users.preferred_language` — 'en' or 'sw' in this
/// deployment). Carried as a pair rather than two separate values purely so
/// every call site resolves both at once from the same recipient-lookup
/// query, never risking a mismatched id/language pairing.
#[derive(Debug, Clone)]
struct Recipient {
    user_id: UserId,
    language: String,
}

impl NotificationService {
    /// Renders a notification's title/body from `notification_templates` in
    /// the recipient's own language, substituting only the variables the
    /// caller explicitly allowlists in `vars` — never arbitrary
    /// interpolation, and never anything free-text/user-authored (every
    /// value passed here is a backend-constructed count/amount/currency
    /// code, the same class of value this module's own top-of-file doc
    /// comment already establishes never needs `sanitize::sanitize_text`).
    /// Falls back to the caller-supplied English defaults if no enabled
    /// template row exists for this exact `(template_key, locale)` pair, so
    /// a type whose template hasn't been authored yet (or an unexpected
    /// locale value) degrades gracefully instead of failing the
    /// notification outright.
    pub(crate) async fn render<'e, E: sqlx::PgExecutor<'e>>(
        executor: E,
        template_key: &str,
        locale: &str,
        vars: &[(&str, &str)],
        fallback_title: &str,
        fallback_body: &str,
    ) -> Result<(String, String), sqlx::Error> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT title_template, body_template FROM notification_templates \
              WHERE template_key = $1 AND locale = $2 AND enabled = TRUE",
        )
        .bind(template_key)
        .bind(locale)
        .fetch_optional(executor)
        .await?;
        let Some((title_template, body_template)) = row else {
            return Ok((fallback_title.to_string(), fallback_body.to_string()));
        };
        let substitute = |template: String| {
            vars.iter().fold(template, |acc, (key, value)| {
                acc.replace(&format!("{{{key}}}"), value)
            })
        };
        Ok((substitute(title_template), substitute(body_template)))
    }

    /// Resolves a single user's own notification-content language directly
    /// (`users.preferred_language`) — used by the call sites below that
    /// address exactly one already-known user, where the richer
    /// [`Self::appointment_recipients`]/[`Self::system_appointment_recipients`]
    /// joins (which resolve *two* parties, with guardian fallback) would be
    /// more than is needed.
    pub(crate) async fn user_language<'e, E: sqlx::PgExecutor<'e>>(
        executor: E,
        user_id: UserId,
    ) -> Result<String, sqlx::Error> {
        Ok(
            sqlx::query_scalar("SELECT preferred_language FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(executor)
                .await?
                .unwrap_or_else(|| "en".to_string()),
        )
    }

    pub async fn notify_article_status(
        conn: &mut sqlx::PgConnection,
        user: UserId,
        article: Uuid,
        version: i32,
        published: bool,
    ) -> Result<(), sqlx::Error> {
        let language = Self::user_language(&mut *conn, user).await?;
        let (template_key, fallback_title, fallback_body) = if published {
            (
                "article_published",
                "Article published",
                "Your article is now available in the published feed.",
            )
        } else {
            (
                "article_archived",
                "Article archived",
                "Your article is no longer available in the published feed.",
            )
        };
        let (title, body) = Self::render(
            &mut *conn,
            template_key,
            &language,
            &[],
            fallback_title,
            fallback_body,
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            user,
            "ARTICLE_STATUS_CHANGED",
            &title,
            &body,
            Some("DOCTOR_ARTICLE"),
            Some(article),
            None,
            &format!("article:{article}:{version}"),
            None,
        )
        .await?;
        Ok(())
    }
    /// Every own-inbox operation below goes through `state.begin_authed(ctx)`
    /// (not the raw `state.db` pool) specifically so `app.current_user_id` is
    /// actually set within the transaction — the owner-scoped RLS policies
    /// (`notifications_owner_select`/`notifications_owner_mark_read`) check
    /// exactly that session variable, and it is never set outside a
    /// `begin_authed`-style transaction. A real failing test caught this the
    /// first time these functions used the bare pool directly, not assumed.
    pub async fn list_own(
        state: &AppState,
        ctx: &AuthContext,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Notification>, AppError> {
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        let mut tx = state.begin_authed(ctx).await?;
        let items = notification_repo::list_for_user(&mut *tx, ctx.user_id, limit, offset).await?;
        tx.commit().await?;
        Ok(items)
    }

    pub async fn list_cursor(
        state: &AppState,
        ctx: &AuthContext,
        limit: i64,
        time: chrono::DateTime<chrono::Utc>,
        id: Uuid,
        before: bool,
    ) -> Result<Vec<Notification>, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let items = notification_repo::list_cursor(
            &mut *tx,
            ctx.user_id,
            limit.clamp(1, 100),
            time,
            id,
            before,
        )
        .await?;
        tx.commit().await?;
        Ok(items)
    }

    pub async fn unread_count(state: &AppState, ctx: &AuthContext) -> Result<i64, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let count = notification_repo::count_unread(&mut *tx, ctx.user_id).await?;
        tx.commit().await?;
        Ok(count)
    }

    /// Returns whether a row was actually marked (vs. already read, or not
    /// owned by this user — both look identical from the caller's
    /// perspective, which is intentional: RLS/ownership never discloses
    /// "that notification exists but isn't yours" vs. "it doesn't exist").
    pub async fn mark_read(
        state: &AppState,
        ctx: &AuthContext,
        id: Uuid,
    ) -> Result<bool, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let marked = notification_repo::mark_read(&mut *tx, id, ctx.user_id).await?;
        tx.commit().await?;
        Ok(marked)
    }

    pub async fn mark_all_read(state: &AppState, ctx: &AuthContext) -> Result<u64, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let count = notification_repo::mark_all_read(&mut *tx, ctx.user_id).await?;
        tx.commit().await?;
        Ok(count)
    }

    pub async fn register_device(
        state: &AppState,
        ctx: &AuthContext,
        device_id: Uuid,
        platform: &str,
        push_token: &str,
    ) -> Result<(), AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        notification_repo::device_tokens::upsert(
            &mut tx,
            device_id,
            ctx.user_id,
            platform,
            push_token,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn revoke_device(
        state: &AppState,
        ctx: &AuthContext,
        device_id: Option<Uuid>,
        push_token: Option<&str>,
    ) -> Result<u64, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let disabled = notification_repo::device_tokens::disable_device(
            &mut *tx,
            ctx.user_id,
            device_id,
            push_token,
        )
        .await?;
        tx.commit().await?;
        Ok(disabled)
    }

    /// Own notification settings: category preferences, quiet hours, and
    /// `users.preferred_language` — surfaced together as one payload since
    /// that's how the Flutter settings screens present them (a single
    /// "Notification settings" page), even though they live in two tables.
    pub async fn get_preferences(
        state: &AppState,
        ctx: &AuthContext,
    ) -> Result<NotificationPreferencesView, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let prefs = notification_repo::user_notification_preferences(&mut *tx, ctx.user_id).await?;
        let language = Self::user_language(&mut *tx, ctx.user_id).await?;
        tx.commit().await?;
        Ok(NotificationPreferencesView {
            operational_enabled: prefs.operational_enabled,
            engagement_enabled: prefs.engagement_enabled,
            quiet_hours_enabled: prefs.quiet_hours_enabled,
            quiet_hours_start_local: prefs.quiet_hours_start_local.format("%H:%M").to_string(),
            quiet_hours_end_local: prefs.quiet_hours_end_local.format("%H:%M").to_string(),
            preferred_language: language,
            greeting_notifications_enabled: prefs.greeting_notifications_enabled,
            religious_greetings_enabled: prefs.religious_greetings_enabled,
        })
    }

    /// Updates own notification settings. `input.language`, when present, is
    /// validated against the two locales this deployment actually supports
    /// (CLAUDE.md: "Active deployment scope is Kenya only; English and
    /// Swahili") — the handler layer validates request shape, but the
    /// allowed-value set is a product rule that belongs here, not there.
    pub async fn update_preferences(
        state: &AppState,
        ctx: &AuthContext,
        input: UpdatePreferencesInput,
    ) -> Result<(), AppError> {
        if let Some(language) = input.language.as_deref() {
            if language != "en" && language != "sw" {
                return Err(AppError::Validation(
                    "language must be 'en' or 'sw'.".into(),
                ));
            }
        }
        let mut tx = state.begin_authed(ctx).await?;
        notification_repo::upsert_preferences(
            &mut *tx,
            ctx.user_id,
            input.operational_enabled,
            input.engagement_enabled,
            input.quiet_hours_enabled,
            input.quiet_hours_start_local,
            input.quiet_hours_end_local,
            input.greeting_notifications_enabled,
            input.religious_greetings_enabled,
        )
        .await?;
        if let Some(language) = input.language.as_deref() {
            crate::repository::user_repo::set_preferred_language(&mut *tx, ctx.user_id, language)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// One notification per committed PENDING -> VERIFIED internal review.
    /// The caller conditionally consumes PENDING in the same transaction;
    /// the dedup key is an independent retry safeguard.
    pub async fn notify_registration_verified(
        conn: &mut sqlx::PgConnection,
        doctor_user_id: UserId,
        doctor_profile_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let language = Self::user_language(&mut *conn, doctor_user_id).await?;
        let (title, body) = Self::render(
            &mut *conn,
            "registration_verified",
            &language,
            &[],
            "Registration verified",
            "AfyaMkononi has approved your submitted KMPDC registration information.",
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor_user_id,
            "REGISTRATION_VERIFIED",
            &title,
            &body,
            Some("DOCTOR_PROFILE"),
            Some(doctor_profile_id),
            None,
            &format!("registration-verified:{doctor_profile_id}"),
            None,
        )
        .await?;
        Ok(())
    }

    /// A Doctor's account becomes usable for consultations when the pending
    /// professional verification is approved. Keep this separate from the
    /// security `REGISTRATION_VERIFIED` record so the optional engagement
    /// preference can suppress the welcome push without suppressing the
    /// authoritative verification event.
    pub async fn notify_doctor_welcome_greeting(
        conn: &mut sqlx::PgConnection,
        doctor_user_id: UserId,
        doctor_profile_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let language = Self::user_language(&mut *conn, doctor_user_id).await?;
        let (title, body) = Self::render(
            &mut *conn,
            "doctor_welcome_greeting",
            &language,
            &[],
            "Welcome to AfyaMkononi",
            "Welcome to AfyaMkononi! Your professional account is now active.",
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &mut *conn,
            Uuid::new_v4(),
            doctor_user_id,
            "WELCOME_GREETING",
            &title,
            &body,
            Some("DOCTOR_PROFILE"),
            Some(doctor_profile_id),
            None,
            &format!("welcome-greeting:doctor:{doctor_profile_id}"),
            None,
            Some(604_800),
        )
        .await?;
        Ok(())
    }

    pub async fn notify_registration_rejected(
        conn: &mut sqlx::PgConnection,
        doctor_user_id: UserId,
        doctor_profile_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let language = Self::user_language(&mut *conn, doctor_user_id).await?;
        let (title, body) = Self::render(
            &mut *conn,
            "registration_rejected",
            &language,
            &[],
            "Registration rejected",
            "AfyaMkononi has rejected your submitted professional registration. Please review your account status.",
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor_user_id,
            "REGISTRATION_REJECTED",
            &title,
            &body,
            Some("DOCTOR_PROFILE"),
            Some(doctor_profile_id),
            None,
            &format!("registration-rejected:{doctor_profile_id}"),
            None,
        )
        .await?;
        Ok(())
    }

    /// h22 remediation: notify a Doctor whose professional verification was
    /// revoked by an Admin. Mirrors notify_registration_rejected's shape;
    /// deliberately no PHI/reason text in the push payload body (CLAUDE.md
    /// Sec.1.7) — the reason lives only in the audit trail and the admin
    /// portal, which the Doctor can view via their own account status.
    pub async fn notify_professional_verification_revoked(
        conn: &mut sqlx::PgConnection,
        doctor_user_id: UserId,
        doctor_profile_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let language = Self::user_language(&mut *conn, doctor_user_id).await?;
        let (title, body) = Self::render(
            &mut *conn,
            "professional_verification_revoked",
            &language,
            &[],
            "Professional verification revoked",
            "AfyaMkononi has revoked your professional verification. Please review your account status.",
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor_user_id,
            "PROFESSIONAL_VERIFICATION_REVOKED",
            &title,
            &body,
            Some("DOCTOR_PROFILE"),
            Some(doctor_profile_id),
            None,
            &format!("professional-verification-revoked:{doctor_profile_id}"),
            None,
        )
        .await?;
        Ok(())
    }

    /// `EARNING_CREDITED` — fired only from the same transaction that
    /// performs the authoritative, exactly-once wallet credit inside
    /// `doctor_complete_consultation` (see `consultation_service.rs`), so a
    /// rolled-back completion can never leave a phantom notification behind,
    /// and a retried/duplicate completion call (which the underlying SQL
    /// function already makes a no-op) naturally never reaches this code a
    /// second time for the same appointment.
    pub async fn notify_earning_credited(
        conn: &mut sqlx::PgConnection,
        doctor_user_id: UserId,
        appointment_id: Uuid,
        net_amount: &BigDecimal,
        currency: &str,
    ) -> Result<(), sqlx::Error> {
        let language = Self::user_language(&mut *conn, doctor_user_id).await?;
        let amount_str = net_amount.to_string();
        let fallback_body = format!("Consultation earning credited: {currency} {net_amount}.");
        let (title, body) = Self::render(
            &mut *conn,
            "earning_credited",
            &language,
            &[("currency", currency), ("amount", &amount_str)],
            "Consultation earning credited",
            &fallback_body,
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor_user_id,
            "EARNING_CREDITED",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("earning-credited:{appointment_id}"),
            None,
        )
        .await?;
        Ok(())
    }

    /// Resolves an appointment's notification recipients via the
    /// `appointment_notification_recipients` SQL function (SECURITY DEFINER,
    /// gated by the same `consultation_participant_allowed` check already
    /// used throughout `consultation_trace` — see
    /// `migrations/20261120000001_milestone1_transactional_notifications`).
    /// This lets every caller below resolve the Patient (or, for a
    /// dependent's appointment, their guardian) and Doctor recipients from
    /// *within its own already-open transaction*, regardless of which role
    /// authenticated it (ADMIN/payment-worker for the payment webhook and
    /// reschedule, DOCTOR/api for the Doctor's own prescription upload,
    /// ADMIN/finance-worker for grievance adjudication) — without that
    /// transaction otherwise being able to see an arbitrary Patient's profile
    /// row under RLS. Returns `None` only if the gate itself fails (should
    /// never happen for a caller that just performed the authorized mutation
    /// on this exact appointment); callers treat that as "skip this
    /// notification, never fail the business transaction over it."
    async fn appointment_recipients(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
    ) -> Result<Option<(Recipient, Option<Recipient>)>, sqlx::Error> {
        let row: Option<(UserId, String, Option<UserId>, Option<String>)> = sqlx::query_as(
            "SELECT doctor_user_id, doctor_language, patient_user_id, patient_language \
               FROM appointment_notification_recipients($1)",
        )
        .bind(appointment_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(
            |(doctor_user_id, doctor_language, patient_user_id, patient_language)| {
                (
                    Recipient {
                        user_id: doctor_user_id,
                        language: doctor_language,
                    },
                    patient_user_id.map(|user_id| Recipient {
                        user_id,
                        // `patient_language` is only `None` in the already-
                        // pathological case neither the patient nor their
                        // guardian resolved to a real account, which the `map`
                        // above already filters out — `unwrap_or_else` here is
                        // just a defensive fallback, never expected to trigger.
                        language: patient_language.unwrap_or_else(|| "en".into()),
                    }),
                )
            },
        ))
    }

    /// `BOOKING_CONFIRMED` — fired only from the same payment-worker
    /// transaction that just performed the authoritative `PENDING_PAYMENT`
    /// -> `CONFIRMED` transition (`appointment_service.rs::process_payment_webhook`,
    /// guarded there by `confirm_payment_if_pending` actually having
    /// transitioned the row this call) — never from a speculative/optimistic
    /// client-side booking state.
    pub async fn notify_booking_confirmed(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let Some((doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "BOOKING_CONFIRMED: appointment recipients unresolved, skipping");
            return Ok(());
        };
        let (title, body) = Self::render(
            &mut *conn,
            "booking_confirmed_doctor",
            &doctor.language,
            &[],
            "Booking confirmed",
            "A consultation booking has been confirmed.",
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor.user_id,
            "BOOKING_CONFIRMED",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("booking-confirmed:{appointment_id}:doctor"),
            None,
        )
        .await?;
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "booking_confirmed_patient",
                &patient.language,
                &[],
                "Booking confirmed",
                "Your consultation booking has been confirmed.",
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "BOOKING_CONFIRMED",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("booking-confirmed:{appointment_id}:patient"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// `BOOKING_RESCHEDULED` — fired from within the same authorized
    /// reschedule transaction (`trace_handlers.rs::reschedule`), after the
    /// `slot_id`/`status` update. Keyed by the reschedule event id (not the
    /// appointment id alone), so a consultation rescheduled more than once
    /// gets a fresh notification each time rather than being deduplicated
    /// against an earlier reschedule of the same appointment.
    pub async fn notify_booking_rescheduled(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
        reschedule_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let Some((doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "BOOKING_RESCHEDULED: appointment recipients unresolved, skipping");
            return Ok(());
        };
        let (title, body) = Self::render(
            &mut *conn,
            "booking_rescheduled_doctor",
            &doctor.language,
            &[],
            "Consultation rescheduled",
            "A consultation has been rescheduled. Check the app for the new time.",
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor.user_id,
            "BOOKING_RESCHEDULED",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("booking-rescheduled:{reschedule_id}:doctor"),
            None,
        )
        .await?;
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "booking_rescheduled_patient",
                &patient.language,
                &[],
                "Consultation rescheduled",
                "Your consultation has been rescheduled. Check the app for the new time.",
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "BOOKING_RESCHEDULED",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("booking-rescheduled:{reschedule_id}:patient"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// `PRESCRIPTION_AVAILABLE` — fired only once per appointment, the
    /// moment its first Rx page is persisted (`prescription_service.rs::
    /// upload_doctor_prescription_photo`, called only when this is the
    /// authorized assigned Doctor's own already-committed-in-this-transaction
    /// page `count == 0` before insert). Patient/guardian only — the Doctor
    /// authored the record, so notifying them of their own action is not
    /// useful. The body deliberately carries no clinical detail (no
    /// medication name, dosage, or diagnosis) — only that a record exists.
    pub async fn notify_prescription_available(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let Some((_doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "PRESCRIPTION_AVAILABLE: appointment recipients unresolved, skipping");
            return Ok(());
        };
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "prescription_available",
                &patient.language,
                &[],
                "Prescription available",
                "A new prescription is available for your consultation.",
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "PRESCRIPTION_AVAILABLE",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("prescription-available:{appointment_id}"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// `REFUND_APPROVED` — fired from the same Finance-worker transaction as
    /// `GrievanceService::adjudicate_refund`'s status transition. Explicitly
    /// worded as approved-pending-execution, never "paid" — approval alone
    /// never moves money (see that function's own doc comment: "No money has
    /// been returned"). Patient/guardian only.
    pub async fn notify_refund_approved(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
        grievance_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let Some((_doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(grievance_id = %grievance_id, "REFUND_APPROVED: appointment recipients unresolved, skipping");
            return Ok(());
        };
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "refund_approved",
                &patient.language,
                &[],
                "Refund approved",
                "Your refund request has been approved and will be processed. No funds have been returned yet.",
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "REFUND_APPROVED",
                &title,
                &body,
                Some("GRIEVANCE_REPORT"),
                Some(grievance_id),
                None,
                &format!("refund-approved:{grievance_id}"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// `REFUND_PROCESSING` / `REFUND_COMPLETED` — fired from the same
    /// Finance-worker transaction as `GrievanceService::transition_refund_request`.
    /// `REFUND_COMPLETED` is only ever reachable for `to_status == "REFUNDED"`,
    /// which that function's own validation already requires a real
    /// `provider_reference` for — this never claims completion on approval
    /// alone. `FAILED`/`RECONCILIATION_REQUIRED` are operational escape
    /// hatches, deliberately out of this milestone's bounded scope, and emit
    /// nothing. Patient/guardian only. Keyed by the refund request id, not
    /// the grievance id, so approval/processing/completion each get their
    /// own notification.
    pub async fn notify_refund_transitioned(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
        refund_request_id: Uuid,
        to_status: &str,
    ) -> Result<(), sqlx::Error> {
        let (notification_type, template_key, fallback_title, fallback_body, key_prefix) =
            match to_status {
                "PROCESSING" => (
                    "REFUND_PROCESSING",
                    "refund_processing",
                    "Refund processing",
                    "Your refund is now being processed.",
                    "refund-processing",
                ),
                "REFUNDED" => (
                    "REFUND_COMPLETED",
                    "refund_completed",
                    "Refund completed",
                    "Your refund has been completed.",
                    "refund-completed",
                ),
                _ => return Ok(()),
            };
        let Some((_doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(refund_request_id = %refund_request_id, "refund transition notification: appointment recipients unresolved, skipping");
            return Ok(());
        };
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                template_key,
                &patient.language,
                &[],
                fallback_title,
                fallback_body,
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                notification_type,
                &title,
                &body,
                Some("FINANCIAL_REFUND_REQUEST"),
                Some(refund_request_id),
                None,
                &format!("{key_prefix}:{refund_request_id}"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// System-worker-safe recipient resolution for callers that already run
    /// as the trusted system identity (the presence worker, the scheduler) —
    /// unlike [`Self::appointment_recipients`] (gated by
    /// `consultation_participant_allowed`, which only evaluates true for an
    /// end-user-authenticated ADMIN/DOCTOR/PATIENT session), these callers
    /// have no such session and instead read `doctor_profiles`/
    /// `patient_profiles` directly via their own already-granted
    /// system-worker RLS read policies (`doctor_worker_read`,
    /// `patient_profiles_system_worker_read`) — the same pattern
    /// `notification_scheduler.rs`'s T-15/daily-summary passes already use.
    async fn system_appointment_recipients(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
    ) -> Result<Option<(Recipient, Option<Recipient>)>, sqlx::Error> {
        let row: Option<(UserId, String, Option<UserId>, Option<String>)> = sqlx::query_as(
            "SELECT d.user_id, du.preferred_language, \
                    COALESCE(p.user_id, g.user_id), \
                    COALESCE(pu.preferred_language, gu.preferred_language) \
               FROM appointments a \
               JOIN doctor_profiles d ON d.id = a.doctor_id \
               JOIN users du ON du.id = d.user_id \
               JOIN patient_profiles p ON p.id = a.patient_id \
               LEFT JOIN patient_profiles g ON g.id = p.guardian_patient_id \
               LEFT JOIN users pu ON pu.id = p.user_id \
               LEFT JOIN users gu ON gu.id = g.user_id \
              WHERE a.id = $1",
        )
        .bind(appointment_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(
            |(doctor_user_id, doctor_language, patient_user_id, patient_language)| {
                (
                    Recipient {
                        user_id: doctor_user_id,
                        language: doctor_language,
                    },
                    patient_user_id.map(|user_id| Recipient {
                        user_id,
                        language: patient_language.unwrap_or_else(|| "en".into()),
                    }),
                )
            },
        ))
    }

    /// `CONSULTATION_READY` — fired from the notification scheduler tick
    /// (`notification_scheduler.rs::run_consultation_ready_pass`), derived
    /// from the SAME authorized join-window the real RTC token mint already
    /// enforces (`ConsultationService::mint_agora_token`'s `window_start`/
    /// `window_end`, `AGORA_TOKEN_WINDOW_*_MINS`) — not a new clinical state.
    /// `ttl_seconds` bounds delivery to the remaining open window, so a
    /// delayed push never invites joining a window that has already closed.
    pub async fn notify_consultation_ready(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
        ttl_seconds: i32,
    ) -> Result<(), sqlx::Error> {
        let Some((doctor, patient)) =
            Self::system_appointment_recipients(conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "CONSULTATION_READY: appointment recipients unresolved, skipping");
            return Ok(());
        };
        let (title, body) = Self::render(
            &mut *conn,
            "consultation_ready",
            &doctor.language,
            &[],
            "Consultation ready",
            "Your consultation is ready to join.",
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &mut *conn,
            Uuid::new_v4(),
            doctor.user_id,
            "CONSULTATION_READY",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("consultation-ready:{appointment_id}:doctor"),
            None,
            Some(ttl_seconds),
        )
        .await?;
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "consultation_ready",
                &patient.language,
                &[],
                "Consultation ready",
                "Your consultation is ready to join.",
            )
            .await?;
            notification_repo::insert_if_new_with_ttl(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "CONSULTATION_READY",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("consultation-ready:{appointment_id}:patient"),
                None,
                Some(ttl_seconds),
            )
            .await?;
        }
        Ok(())
    }

    /// `CONSULTATION_REJOIN` — fired from the same system-worker transaction
    /// that just recorded, from trusted Agora provider presence callbacks
    /// (never untrusted client telemetry), that the room became fully empty
    /// (`consultation_presence_service.rs::apply_provider_event`). Notifies
    /// both participants — Agora's presence model here only tracks
    /// "is the room empty," not which single side dropped, so both are
    /// equally "the disconnected participant" at this instant. Keyed by the
    /// specific grace-period deadline so a later, independent disconnect on
    /// the same session gets its own notification rather than being
    /// deduplicated against an earlier one; a short cooldown additionally
    /// suppresses a flood from rapid repeated disconnect/reconnect cycles.
    /// `ttl_seconds` bounds delivery to what remains of the 180s grace
    /// window, so a delayed push can never invite rejoining a consultation
    /// that has already been marked interrupted.
    pub async fn notify_consultation_rejoin(
        conn: &mut sqlx::PgConnection,
        session_id: Uuid,
        appointment_id: Uuid,
        deadline: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let ttl_seconds = (deadline - Utc::now()).num_seconds().clamp(1, 180) as i32;
        // `related_entity_id` on these rows is the appointment (matching
        // every other notification type's convention), not the session —
        // bind the same id actually stored below, not `session_id`.
        let (recent,): (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM notifications \
               WHERE related_entity_id = $1 AND type = 'CONSULTATION_REJOIN' \
                 AND created_at > clock_timestamp() - interval '120 seconds')",
        )
        .bind(appointment_id)
        .fetch_one(&mut *conn)
        .await?;
        if recent {
            return Ok(());
        }
        let Some((doctor, patient)) =
            Self::system_appointment_recipients(conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "CONSULTATION_REJOIN: appointment recipients unresolved, skipping");
            return Ok(());
        };
        let deadline_key = deadline.to_rfc3339();
        const FALLBACK_TITLE: &str = "Rejoin available";
        const FALLBACK_BODY: &str = "Both participants have left the consultation room. Rejoin before the grace period ends to continue, or it will be marked interrupted.";
        let (title, body) = Self::render(
            &mut *conn,
            "consultation_rejoin",
            &doctor.language,
            &[],
            FALLBACK_TITLE,
            FALLBACK_BODY,
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &mut *conn,
            Uuid::new_v4(),
            doctor.user_id,
            "CONSULTATION_REJOIN",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("consultation-rejoin:{session_id}:{deadline_key}:doctor"),
            None,
            Some(ttl_seconds),
        )
        .await?;
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "consultation_rejoin",
                &patient.language,
                &[],
                FALLBACK_TITLE,
                FALLBACK_BODY,
            )
            .await?;
            notification_repo::insert_if_new_with_ttl(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "CONSULTATION_REJOIN",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("consultation-rejoin:{session_id}:{deadline_key}:patient"),
                None,
                Some(ttl_seconds),
            )
            .await?;
        }
        Ok(())
    }

    /// `CONSULTATION_INTERRUPTED` — fired only after the authoritative 180s
    /// grace period has actually expired with the room still empty
    /// (`consultation_presence_service.rs`'s expiry worker and its
    /// late-rejoin-rejected path — never from client telemetry, never
    /// speculatively). Wording is deliberately "interrupted," never
    /// "completed" — this must not be mistaken for a medical completion.
    pub async fn notify_consultation_interrupted(
        conn: &mut sqlx::PgConnection,
        session_id: Uuid,
        appointment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let Some((doctor, patient)) =
            Self::system_appointment_recipients(conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "CONSULTATION_INTERRUPTED: appointment recipients unresolved, skipping");
            return Ok(());
        };
        const FALLBACK_TITLE: &str = "Consultation interrupted";
        const FALLBACK_BODY: &str = "This consultation was interrupted by a connection issue and could not continue. It has not been marked completed.";
        let (title, body) = Self::render(
            &mut *conn,
            "consultation_interrupted",
            &doctor.language,
            &[],
            FALLBACK_TITLE,
            FALLBACK_BODY,
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor.user_id,
            "CONSULTATION_INTERRUPTED",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("consultation-interrupted:{session_id}:doctor"),
            None,
        )
        .await?;
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "consultation_interrupted",
                &patient.language,
                &[],
                FALLBACK_TITLE,
                FALLBACK_BODY,
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "CONSULTATION_INTERRUPTED",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("consultation-interrupted:{session_id}:patient"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// `CONSULTATION_NEEDS_REVIEW` — a persistent Admin alert the moment a
    /// session enters `NEEDS_REVIEW`, reusing the existing notification/FCM
    /// infrastructure rather than a second review-queue mechanism (the
    /// review queue itself remains `consultation_sessions.recovery_state` /
    /// the existing Admin Command Center read surface; this is purely the
    /// push/inbox alert layer on top of it). Fans out to every active admin
    /// account. `role = 'ADMIN'` — confirmed directly against the schema
    /// (`migrations/20261106000002_single_admin`), not assumed: the six
    /// historical sub-roles (`PLATFORM_ADMIN`/`CLINICAL_ADMIN`/etc.) were
    /// consolidated into one canonical `ADMIN` role and a trigger
    /// (`reject_active_legacy_admin_role`) now refuses to ever assign a
    /// legacy value again, so querying for the old sub-role strings here
    /// would silently match zero accounts.
    pub async fn notify_consultation_needs_review(
        conn: &mut sqlx::PgConnection,
        session_id: Uuid,
        appointment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let admins: Vec<(UserId, String)> = sqlx::query_as(
            "SELECT id, preferred_language FROM users WHERE role = 'ADMIN' AND is_active = TRUE AND deleted_at IS NULL",
        )
        .fetch_all(&mut *conn)
        .await?;
        for (admin_user_id, language) in admins {
            let (title, body) = Self::render(
                &mut *conn,
                "consultation_needs_review",
                &language,
                &[],
                "Consultation needs review",
                "A consultation was interrupted and needs Admin review.",
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                admin_user_id,
                "CONSULTATION_NEEDS_REVIEW",
                &title,
                &body,
                Some("CONSULTATION_SESSION"),
                Some(session_id),
                None,
                &format!("consultation-needs-review:{session_id}:{admin_user_id}"),
                None,
            )
            .await?;
        }
        let _ = appointment_id;
        Ok(())
    }

    /// `CONSULTATION_RECOVERY_RESOLVED` — fired from the same Admin-authenticated
    /// transaction as `admin_resolve_consultation_recovery`'s own `RESOLVED`
    /// result (never for an `ALREADY_RESOLVED` replay). Wording distinguishes
    /// the two possible outcomes without ever promising a payment, refund or
    /// settlement — `PENDING_FINANCE_ACTION` explicitly says the financial
    /// question is still open, and `RESOLVED_NO_SETTLEMENT` explicitly says
    /// none is due; neither implies money has moved.
    pub async fn notify_consultation_recovery_resolved(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
        session_id: Uuid,
        outcome: &str,
    ) -> Result<(), sqlx::Error> {
        let (template_key, fallback_title, fallback_body) = match outcome {
            "PENDING_FINANCE_ACTION" => (
                "consultation_recovery_resolved_pending",
                "Consultation review resolved",
                "Admin review of your interrupted consultation is complete. A financial review is still pending; nothing has been refunded or paid out yet.",
            ),
            _ => (
                "consultation_recovery_resolved_no_settlement",
                "Consultation review resolved",
                "Admin review of your interrupted consultation is complete. No further financial action is required.",
            ),
        };
        let Some((doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "CONSULTATION_RECOVERY_RESOLVED: appointment recipients unresolved, skipping");
            return Ok(());
        };
        let (title, body) = Self::render(
            &mut *conn,
            template_key,
            &doctor.language,
            &[],
            fallback_title,
            fallback_body,
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor.user_id,
            "CONSULTATION_RECOVERY_RESOLVED",
            &title,
            &body,
            Some("CONSULTATION_SESSION"),
            Some(session_id),
            None,
            &format!("consultation-recovery-resolved:{session_id}:doctor"),
            None,
        )
        .await?;
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                template_key,
                &patient.language,
                &[],
                fallback_title,
                fallback_body,
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "CONSULTATION_RECOVERY_RESOLVED",
                &title,
                &body,
                Some("CONSULTATION_SESSION"),
                Some(session_id),
                None,
                &format!("consultation-recovery-resolved:{session_id}:patient"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// `CONSULTATION_COMPLETED` — fired only from the `"COMPLETED"` arm of
    /// `ConsultationService::complete_in_transaction`, i.e. only once
    /// `doctor_complete_consultation` itself (Stage B4's authoritative,
    /// PAYMENT_HELD-protected completion guard — untouched by this) actually
    /// performed the transition; the function's own `ALREADY_COMPLETED`
    /// short-circuit means a retried/duplicate completion call never reaches
    /// here a second time for the same appointment.
    pub async fn notify_consultation_completed(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let Some((doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "CONSULTATION_COMPLETED: appointment recipients unresolved, skipping");
            return Ok(());
        };
        let (title, body) = Self::render(
            &mut *conn,
            "consultation_completed",
            &doctor.language,
            &[],
            "Consultation completed",
            "Your consultation has been completed.",
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor.user_id,
            "CONSULTATION_COMPLETED",
            &title,
            &body,
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("consultation-completed:{appointment_id}:doctor"),
            None,
        )
        .await?;
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "consultation_completed",
                &patient.language,
                &[],
                "Consultation completed",
                "Your consultation has been completed.",
            )
            .await?;
            notification_repo::insert_if_new(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "CONSULTATION_COMPLETED",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("consultation-completed:{appointment_id}:patient"),
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// `PAYOUT_PAID` — must only ever be called once a disbursement item
    /// genuinely reaches a real external-provider-confirmed `PAID` state.
    /// Called from `DisbursementService::confirm_disbursement_item_paid`, in
    /// the same transaction as the real status transition, which itself is
    /// only reachable via `wallet_repo::confirm_disbursement_item_paid`'s
    /// guard (`status IN ('APPROVED','PROCESSING') AND provider_code <>
    /// 'DEV_SIMULATED'`) — so this never fires for a `DEV_SIMULATED` payout
    /// destination or for PREPARED/APPROVED/PROCESSING alone. Payout
    /// *execution* itself remains `DEFERRED_EXTERNAL_PAYMENT_INTEGRATION`
    /// (no real provider call exists anywhere yet); this notification fires
    /// only on Finance's out-of-band confirmation of a real provider
    /// statement, not on any simulated/dev path.
    pub async fn notify_payout_paid(
        conn: &mut sqlx::PgConnection,
        doctor_user_id: UserId,
        disbursement_item_id: Uuid,
        amount: &BigDecimal,
        currency: &str,
    ) -> Result<(), sqlx::Error> {
        let language = Self::user_language(&mut *conn, doctor_user_id).await?;
        let amount_str = amount.to_string();
        let fallback_body = format!("Your {currency} {amount} payout has been confirmed.");
        let (title, body) = Self::render(
            &mut *conn,
            "payout_paid",
            &language,
            &[("currency", currency), ("amount", &amount_str)],
            "Payout confirmed",
            &fallback_body,
        )
        .await?;
        notification_repo::insert_if_new(
            &mut *conn,
            Uuid::new_v4(),
            doctor_user_id,
            "PAYOUT_PAID",
            &title,
            &body,
            Some("DISBURSEMENT_ITEM"),
            Some(disbursement_item_id),
            None,
            &format!("payout-paid:{disbursement_item_id}"),
            None,
        )
        .await?;
        Ok(())
    }

    /// Milestone 4 `WELCOME_GREETING` — fired only from within
    /// `patient_handlers::update_own_profile`'s own transaction, the moment
    /// that update is confirmed to have taken this exact profile out of the
    /// OTP-login placeholder state for the first time (see that call site's
    /// own doc comment for the genuine, non-fabricated "account activation"
    /// signal this keys off). `dedup_key` has no date/count component — a
    /// profile can only leave the placeholder state once, ever.
    pub async fn notify_welcome_greeting(
        conn: &mut sqlx::PgConnection,
        patient_profile_id: PatientProfileId,
    ) -> Result<(), sqlx::Error> {
        let row: Option<(UserId, String)> = sqlx::query_as(
            "SELECT p.user_id, u.preferred_language FROM patient_profiles p \
               JOIN users u ON u.id = p.user_id \
              WHERE p.id = $1 AND p.user_id IS NOT NULL",
        )
        .bind(patient_profile_id)
        .fetch_optional(&mut *conn)
        .await?;
        let Some((user_id, language)) = row else {
            // A dependent profile (`user_id IS NULL`) was never a login
            // placeholder, so this is never expected to miss for a real
            // self-profile — defensive no-op, never a hard failure of the
            // profile-update transaction it runs inside.
            return Ok(());
        };
        let (title, body) = Self::render(
            &mut *conn,
            "welcome_greeting",
            &language,
            &[],
            "Welcome to AfyaMkononi",
            "Welcome to AfyaMkononi! Your profile is complete — you can now book a consultation whenever you need one.",
        )
        .await?;
        notification_repo::insert_if_new_with_ttl(
            &mut *conn,
            Uuid::new_v4(),
            user_id,
            "WELCOME_GREETING",
            &title,
            &body,
            Some("PATIENT_PROFILE"),
            Some(patient_profile_id.into()),
            None,
            &format!("welcome-greeting:{patient_profile_id}"),
            None,
            Some(604_800),
        )
        .await?;
        Ok(())
    }

    /// Milestone 4 `FIRST_CONSULTATION_MILESTONE` — fired only from the same
    /// "COMPLETED" arm of `ConsultationService::complete_in_transaction` as
    /// `notify_consultation_completed`, and only when this is genuinely the
    /// Patient's first-ever completed consultation (a real `COUNT(*)`
    /// against `appointments.status = 'COMPLETED'`, not inferred from the
    /// dedup key alone — the dedup key is an independent, redundant
    /// safeguard against the same race `insert_if_new`'s own doc comment
    /// describes elsewhere). The Patient/guardian and Doctor recipients are
    /// evaluated independently so each role gets its own first-ever event.
    pub async fn notify_first_consultation_milestone(
        conn: &mut sqlx::PgConnection,
        appointment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let patient_id: Option<PatientProfileId> =
            sqlx::query_scalar("SELECT patient_id FROM appointments WHERE id = $1")
                .bind(appointment_id)
                .fetch_optional(&mut *conn)
                .await?;
        let Some(patient_id) = patient_id else {
            return Ok(());
        };
        let Some((doctor, patient)) =
            Self::appointment_recipients(&mut *conn, appointment_id).await?
        else {
            tracing::warn!(appointment_id = %appointment_id, "FIRST_CONSULTATION_MILESTONE: appointment recipients unresolved, skipping");
            return Ok(());
        };
        let doctor_completed_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM appointments WHERE doctor_id = (SELECT doctor_id FROM appointments WHERE id = $1) AND status = 'COMPLETED'",
        )
        .bind(appointment_id)
        .fetch_one(&mut *conn)
        .await?;
        if doctor_completed_count == 1 {
            let (title, body) = Self::render(
                &mut *conn,
                "first_consultation_milestone",
                &doctor.language,
                &[],
                "Your first consultation is complete",
                "Congratulations on completing your first consultation with AfyaMkononi!",
            )
            .await?;
            notification_repo::insert_if_new_with_ttl(
                &mut *conn,
                Uuid::new_v4(),
                doctor.user_id,
                "FIRST_CONSULTATION_MILESTONE",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("first-consultation-milestone:doctor:{}", doctor.user_id),
                None,
                Some(604_800),
            )
            .await?;
        }
        let completed_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM appointments WHERE patient_id = $1 AND status = 'COMPLETED'",
        )
        .bind(patient_id)
        .fetch_one(&mut *conn)
        .await?;
        if completed_count != 1 {
            return Ok(());
        }
        if let Some(patient) = patient {
            let (title, body) = Self::render(
                &mut *conn,
                "first_consultation_milestone",
                &patient.language,
                &[],
                "Your first consultation is complete",
                "Congratulations on completing your first consultation with AfyaMkononi! We are glad to be part of your health journey.",
            )
            .await?;
            notification_repo::insert_if_new_with_ttl(
                &mut *conn,
                Uuid::new_v4(),
                patient.user_id,
                "FIRST_CONSULTATION_MILESTONE",
                &title,
                &body,
                Some("APPOINTMENT"),
                Some(appointment_id),
                None,
                &format!("first-consultation-milestone:{patient_id}"),
                None,
                Some(604_800),
            )
            .await?;
        }
        Ok(())
    }
}
