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
use crate::domain::ids::UserId;
use crate::domain::models::Notification;
use crate::error::AppError;
use crate::repository::{notification_repo, AppState};
use bigdecimal::BigDecimal;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use sqlx::{Acquire, Postgres};
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

impl NotificationService {
    pub async fn notify_article_status<'e, A: Acquire<'e, Database = Postgres> + Send>(
        executor: A,
        user: UserId,
        article: Uuid,
        version: i32,
        published: bool,
    ) -> Result<(), sqlx::Error> {
        notification_repo::insert_if_new(
            executor,
            Uuid::new_v4(),
            user,
            "ARTICLE_STATUS_CHANGED",
            if published {
                "Article published"
            } else {
                "Article archived"
            },
            if published {
                "Your article is now available in the published feed."
            } else {
                "Your article is no longer available in the published feed."
            },
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

    /// One notification per committed PENDING -> VERIFIED internal review.
    /// The caller conditionally consumes PENDING in the same transaction;
    /// the dedup key is an independent retry safeguard.
    pub async fn notify_registration_verified<'e, A: Acquire<'e, Database = Postgres> + Send>(
        executor: A,
        doctor_user_id: UserId,
        doctor_profile_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        notification_repo::insert_if_new(
            executor,
            Uuid::new_v4(),
            doctor_user_id,
            "REGISTRATION_VERIFIED",
            "Registration verified",
            "HelloDoctor has approved your submitted KMPDC registration information.",
            Some("DOCTOR_PROFILE"),
            Some(doctor_profile_id),
            None,
            &format!("registration-verified:{doctor_profile_id}"),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn notify_registration_rejected<'e, A: Acquire<'e, Database = Postgres> + Send>(
        executor: A,
        doctor_user_id: UserId,
        doctor_profile_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        notification_repo::insert_if_new(
            executor,
            Uuid::new_v4(),
            doctor_user_id,
            "REGISTRATION_REJECTED",
            "Registration rejected",
            "HelloDoctor has rejected your submitted professional registration. Please review your account status.",
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
    pub async fn notify_professional_verification_revoked<
        'e,
        A: Acquire<'e, Database = Postgres> + Send,
    >(
        executor: A,
        doctor_user_id: UserId,
        doctor_profile_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        notification_repo::insert_if_new(
            executor,
            Uuid::new_v4(),
            doctor_user_id,
            "PROFESSIONAL_VERIFICATION_REVOKED",
            "Professional verification revoked",
            "HelloDoctor has revoked your professional verification. Please review your account status.",
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
    pub async fn notify_earning_credited<'e, A: Acquire<'e, Database = Postgres> + Send>(
        executor: A,
        doctor_user_id: UserId,
        appointment_id: Uuid,
        net_amount: &BigDecimal,
        currency: &str,
    ) -> Result<(), sqlx::Error> {
        notification_repo::insert_if_new(
            executor,
            Uuid::new_v4(),
            doctor_user_id,
            "EARNING_CREDITED",
            "Consultation earning credited",
            &format!("Consultation earning credited: {currency} {net_amount}."),
            Some("APPOINTMENT"),
            Some(appointment_id),
            None,
            &format!("earning-credited:{appointment_id}"),
            None,
        )
        .await?;
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
    pub async fn notify_payout_paid<'e, A: Acquire<'e, Database = Postgres> + Send>(
        executor: A,
        doctor_user_id: UserId,
        disbursement_item_id: Uuid,
        amount: &BigDecimal,
        currency: &str,
    ) -> Result<(), sqlx::Error> {
        notification_repo::insert_if_new(
            executor,
            Uuid::new_v4(),
            doctor_user_id,
            "PAYOUT_PAID",
            "Payout confirmed",
            &format!("Your {currency} {amount} payout has been confirmed."),
            Some("DISBURSEMENT_ITEM"),
            Some(disbursement_item_id),
            None,
            &format!("payout-paid:{disbursement_item_id}"),
            None,
        )
        .await?;
        Ok(())
    }
}
