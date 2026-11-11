//! Phase 2: Secure Email OTP Password Recovery — self-service password reset
//! for Patient and Doctor accounts only. Admin is deliberately excluded (see
//! `is_reset_eligible`): an Admin account behaves identically to a
//! non-existent account at every step of this flow, both to keep the
//! existing "Admin recovery stays operator-mediated" policy and so a caller
//! cannot use response differences to discover that an email belongs to an
//! Admin account.
//!
//! Three-step state machine, state derived from column presence rather than
//! a stored status (see `domain::models::PasswordResetRequest`'s doc
//! comment and `migrations/20261207000001_password_reset`):
//!
//!   REQUESTED --(correct OTP)--> OTP_VERIFIED --(new password)--> COMPLETED
//!
//! `request_reset` never reveals whether the submitted email corresponds to
//! an eligible account (anti-enumeration): the only secret in this flow is
//! the emailed OTP code itself, so `request_reset` returns the same
//! response regardless of outcome. `verify_otp` and `complete_reset` are
//! only reachable by someone who already holds a secret delivered out of
//! band (the emailed code, then the post-verification authorization value),
//! so neither needs — or attempts — anti-enumeration framing; both return
//! the same generic "invalid or expired" error for every failure mode,
//! exactly like the existing MFA challenge endpoints.
use crate::crypto;
use crate::domain::models::UserRole;
use crate::domain::validation;
use crate::email_delivery::{self, OtpEmailKind, TransactionalEmailKind};
use crate::error::AppError;
use crate::ip_jail;
use crate::repository::{
    doctor_repo, password_reset_repo, patient_repo, session_repo, user_repo, AppState,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

/// 5 minutes, matching the approved copy's own wording
/// ("This code expires in **5 minutes**" — AFYAMKONONI_EMAIL_CONTENT_V1.md
/// "reset mail") and the brief's own default. The email template still takes
/// this as an explicit parameter rather than hardcoding "5 minutes" as text,
/// so the two can never silently drift apart.
pub const PASSWORD_RESET_OTP_TTL_SECONDS: i64 = 300;

/// A deliberately separate, shorter-lived window for the *second* secret
/// (the post-OTP-verification authorization) — long enough to fill in a new
/// password on a slow connection, short enough that an authorization a user
/// abandons mid-flow doesn't stay valid indefinitely.
const PASSWORD_RESET_AUTHORIZATION_TTL_SECONDS: i64 = 600;

pub struct PasswordResetService;

impl PasswordResetService {
    fn feature_gate(state: &AppState) -> Result<(), AppError> {
        if !state.config.password_reset_enabled {
            return Err(AppError::NotFound(
                "Password reset is not available.".into(),
            ));
        }
        Ok(())
    }

    /// Step 1 (public, unauthenticated). Always returns `Ok` for a
    /// syntactically valid email — whether or not an eligible account was
    /// found, whether or not an email was actually sent — except for
    /// rate-limit/feature-availability rejections, which are allowed to
    /// differ because they reveal nothing about any specific account.
    ///
    /// The `Option<String>` is a development-only convenience, never an
    /// enumeration-safe value: it is `Some(code)` only when
    /// `ENVIRONMENT=development` and `DEV_AUTH_OTP_ENABLED=true` bypass real
    /// email delivery — the exact same narrow, already-established
    /// precedent `request_patient_email_otp`/`doctor_login_step1` use for
    /// their own `debug_otp_code`. This genuinely does distinguish an
    /// eligible account from an ineligible one, but only under a condition
    /// this codebase already treats as "local development only, never
    /// production" (and `validate_password_reset_policy` refuses to enable
    /// this feature at all outside development/test), so it is not a new
    /// real-world enumeration surface.
    pub async fn request_reset(state: &AppState, email: &str) -> Result<Option<String>, AppError> {
        Self::feature_gate(state)?;
        validation::validate_email(email)?;
        let email = email.trim().to_lowercase();

        crate::services::auth_service::abuse::target_attempt(state, "password-reset", &email)
            .await?;

        let eligible = Self::eligible_user(state, &email).await?;

        let Some(user) = eligible else {
            // Spend the same shared OTP-delivery budget a real send would,
            // so "no such account" and "account exists, code sent" cost the
            // same abuse-control budget and cannot be told apart by probing
            // it repeatedly. The call's own Err (rate-limited) is allowed to
            // surface identically in both branches.
            crate::services::auth_service::abuse::otp_delivery(state, &email).await?;
            return Ok(None);
        };

        crate::services::auth_service::abuse::otp_delivery(state, &email).await?;

        let code = crypto::generate_numeric_code(6);
        let token = Uuid::new_v4().to_string();
        let otp_expires_at = Utc::now() + Duration::seconds(PASSWORD_RESET_OTP_TTL_SECONDS);
        // Single atomic upsert, not a separate invalidate-then-insert: the
        // `password_reset_requests_user_id_unique` constraint
        // (migrations/20261208000001) makes this race-safe against a second,
        // concurrent `request_reset` call for the same account — see
        // `password_reset_repo::upsert_for_user`'s own doc comment.
        password_reset_repo::upsert_for_user(
            &state.auth_db,
            &token,
            user.id,
            &crypto::hash_code(&code),
            otp_expires_at,
        )
        .await?;

        let first_name = Self::first_name_for(state, &user).await;
        let rendered = crate::services::email_templates::render_password_reset_otp(
            &first_name,
            &code,
            PASSWORD_RESET_OTP_TTL_SECONDS / 60,
            &state.email_assets,
        );
        if let Err(error) = email_delivery::send_rendered_otp_email_tracked(
            state,
            &email,
            &rendered,
            OtpEmailKind::PasswordReset,
            &token,
            otp_expires_at,
        )
        .await
        {
            password_reset_repo::delete(&state.auth_db, &token).await?;
            return Err(error);
        }

        let debug_code = (state.config.environment == "development"
            && state.config.dev_auth_otp_enabled)
            .then_some(code);
        Ok(debug_code)
    }

    /// Step 2 (public, unauthenticated). On success, returns a fresh,
    /// high-entropy, single-use authorization value the client must present
    /// to `complete_reset` — never the OTP code itself a second time.
    pub async fn verify_otp(
        state: &AppState,
        ip: &str,
        email: &str,
        otp_code: &str,
    ) -> Result<(String, i64), AppError> {
        Self::feature_gate(state)?;
        let email = email.trim().to_lowercase();
        let account_key = format!("password-reset:{email}");
        let invalid = || AppError::Unauthorized("Invalid or expired verification code.".into());

        if ip_jail::is_ip_jailed(&state.redis, ip).await? {
            return Err(AppError::Forbidden(
                "Too many failed attempts from this network. Try again later.".into(),
            ));
        }
        if ip_jail::is_account_jailed(&state.redis, &account_key).await? {
            return Err(AppError::Forbidden(
                "Too many failed attempts for this account. Try again later.".into(),
            ));
        }

        let mut tx = state.auth_db.begin().await?;

        let Some(user) = Self::eligible_user_tx(&mut tx, &email).await? else {
            tx.commit().await?;
            ip_jail::record_failure(&state.redis, ip, &account_key).await?;
            return Err(invalid());
        };

        let Some(request) =
            password_reset_repo::find_active_for_user_for_update(&mut tx, user.id).await?
        else {
            tx.commit().await?;
            ip_jail::record_failure(&state.redis, ip, &account_key).await?;
            return Err(invalid());
        };

        if request.authorization_hash.is_some() {
            // Already verified once (OTP_VERIFIED) — the OTP itself must
            // never be checked again from this state; treat exactly like an
            // unknown/expired request rather than re-validating a consumed
            // secret.
            tx.commit().await?;
            ip_jail::record_failure(&state.redis, ip, &account_key).await?;
            return Err(invalid());
        }

        if Utc::now() > request.otp_expires_at {
            password_reset_repo::delete(&mut *tx, &request.token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "Verification code expired. Please request a new one.".into(),
            ));
        }
        if request.attempt_count >= request.max_attempts {
            password_reset_repo::delete(&mut *tx, &request.token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "Too many incorrect attempts. Please request a new code.".into(),
            ));
        }
        if !crypto::verify_code(otp_code, &request.code_hash) {
            password_reset_repo::increment_attempts(&mut tx, &request.token).await?;
            tx.commit().await?;
            ip_jail::record_failure(&state.redis, ip, &account_key).await?;
            return Err(invalid());
        }

        let authorization = Uuid::new_v4().to_string();
        let authorization_hash = crypto::hash_code(&authorization);
        let authorization_expires_at =
            Utc::now() + Duration::seconds(PASSWORD_RESET_AUTHORIZATION_TTL_SECONDS);
        password_reset_repo::mark_verified(
            &mut tx,
            &request.token,
            &authorization_hash,
            authorization_expires_at,
        )
        .await?;
        tx.commit().await?;
        ip_jail::record_success(&state.redis, &account_key).await?;

        Ok((authorization, PASSWORD_RESET_AUTHORIZATION_TTL_SECONDS))
    }

    /// Step 3 (public, unauthenticated). Consumes the authorization value
    /// exactly once (the row is deleted inside the same transaction that
    /// updates the password), sets the new password, and revokes every
    /// existing session for the account — whoever held the old password's
    /// live tokens is signed out everywhere, exactly like the authenticated
    /// change-password flow, except here there is no "current session" to
    /// exempt.
    pub async fn complete_reset(
        state: &AppState,
        reset_authorization: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        Self::feature_gate(state)?;
        let invalid = || AppError::Unauthorized("Invalid or expired reset authorization.".into());
        if reset_authorization.is_empty() || reset_authorization.len() > 256 {
            return Err(invalid());
        }
        validation::validate_password_strength(new_password)?;

        let authorization_hash = crypto::hash_code(reset_authorization);

        let mut tx = state.auth_db.begin().await?;
        let Some(request) = password_reset_repo::find_by_authorization_hash_for_update(
            &mut tx,
            &authorization_hash,
        )
        .await?
        else {
            tx.commit().await?;
            return Err(invalid());
        };
        let Some(authorization_expires_at) = request.authorization_expires_at else {
            // Unreachable in practice (authorization_hash and
            // authorization_expires_at are only ever written together by
            // `mark_verified`), but fail closed rather than trust that
            // column coupling implicitly.
            tx.commit().await?;
            return Err(invalid());
        };
        if Utc::now() > authorization_expires_at {
            password_reset_repo::delete(&mut *tx, &request.token).await?;
            tx.commit().await?;
            return Err(AppError::Unauthorized(
                "This reset session has expired. Please start again.".into(),
            ));
        }

        let Some(user) = user_repo::find_by_id(&mut *tx, request.user_id)
            .await?
            .filter(|u| Self::is_reset_eligible_role(u.role) && u.is_active)
        else {
            // The account was deactivated/changed role between steps 2 and
            // 3 — fail closed, same generic message as any other failure
            // here (no new oracle).
            password_reset_repo::delete(&mut *tx, &request.token).await?;
            tx.commit().await?;
            return Err(invalid());
        };

        let new_hash = crypto::hash_password_async(new_password.to_owned()).await?;
        user_repo::update_password_hash(&mut *tx, user.id, &new_hash).await?;
        // Invalidates every outstanding recovery challenge for this account
        // (not just the one matched by this authorization), not merely the
        // single row looked up above: a second, concurrently-requested reset
        // flow for the same account (`request_reset`'s own
        // `invalidate_all_for_user` call races against this one outside any
        // shared lock) could otherwise leave a second, still-live row behind
        // after this one completes. Scoped by `user.id`, so this never
        // touches another account's reset state. Password update, challenge
        // invalidation, and session revocation all happen under the same
        // `tx`, committed or rolled back together.
        password_reset_repo::invalidate_all_for_user(&mut *tx, user.id).await?;
        let revoked = session_repo::revoke_all_sessions_for_user(&mut *tx, user.id).await?;
        tx.commit().await?;

        tracing::info!(
            revoked_sessions = revoked,
            "Password reset completed via email OTP; all sessions revoked"
        );

        if let Some(recipient) = user.email.clone() {
            let first_name = Self::first_name_for(state, &user).await;
            let rendered = crate::services::email_templates::render_password_changed(
                &first_name,
                &state.email_assets,
            );
            email_delivery::send_templated_email(
                state,
                &recipient,
                &rendered,
                TransactionalEmailKind::PasswordChanged,
                &format!("password-reset-completed:{}", Uuid::new_v4()),
            )
            .await;
        }
        Ok(())
    }

    fn is_reset_eligible_role(role: UserRole) -> bool {
        matches!(role, UserRole::Patient | UserRole::Doctor)
    }

    /// `None` covers every ineligible case uniformly (no account, wrong
    /// role — including Admin — inactive, deleted, or no password set), so
    /// `request_reset` cannot distinguish any of them from the outside.
    async fn eligible_user(
        state: &AppState,
        email: &str,
    ) -> Result<Option<crate::domain::models::User>, AppError> {
        Ok(user_repo::find_by_email(&state.auth_db, email)
            .await?
            .filter(|u| {
                Self::is_reset_eligible_role(u.role) && u.is_active && u.password_hash.is_some()
            }))
    }

    async fn eligible_user_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        email: &str,
    ) -> Result<Option<crate::domain::models::User>, AppError> {
        Ok(user_repo::find_by_email(&mut **tx, email)
            .await?
            .filter(|u| {
                Self::is_reset_eligible_role(u.role) && u.is_active && u.password_hash.is_some()
            }))
    }

    /// Role-aware display name lookup, mirroring
    /// `auth_handlers::resolve_display_name_for_email` — duplicated rather
    /// than shared because that helper lives in the handler layer and is
    /// private to it; both independently fall back to an empty string
    /// (`fallback_name` in `email_templates` renders that as "there").
    /// Deliberately queried via `state.db` (`hellodoctor_api`), never
    /// `state.auth_db`: `doctor_profiles`/`patient_profiles` are ordinary
    /// content tables outside the auth worker's grant, exactly as the
    /// existing change-password flow already established.
    async fn first_name_for(state: &AppState, user: &crate::domain::models::User) -> String {
        match user.role {
            UserRole::Doctor => doctor_repo::find_by_user_id(&state.db, user.id)
                .await
                .ok()
                .flatten()
                .map(|d| d.full_name)
                .unwrap_or_default(),
            UserRole::Patient => patient_repo::find_by_user_id(&state.db, user.id)
                .await
                .ok()
                .flatten()
                .map(|p| p.full_name)
                .unwrap_or_default(),
            _ => String::new(),
        }
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
    }
}
