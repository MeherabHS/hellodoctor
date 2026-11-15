//! Admin invitations (product owner, 2026-11-14; migrations/20261216000001).
//!
//! A signed-in Admin invites someone by email. The invitee opens the emailed
//! link (Admin Web `/invite?token=…`), sets their own phone number and
//! password, and becomes an Admin; afterwards they sign in like any Admin
//! (password + emailed verification code). The token is 32 random bytes, sent
//! only in the email, stored only as a SHA-256 hash, single use, valid for
//! `INVITATION_TTL_MINUTES` (5 minutes, product owner 2026-11-14), and revocable. Any Admin may invite (single-Admin
//! rule, CLAUDE.md rule 7). Inviting an email that already has an account of
//! any role is refused: promoting existing accounts is not supported.
use crate::auth_context::AuthContext;
use crate::crypto;
use crate::domain::ids::UserId;
use crate::domain::models::{User, UserRole};
use crate::domain::validation::{
    normalize_kenya_phone_number, validate_email, validate_password_strength,
};
use crate::email_delivery::{self, TransactionalEmailKind};
use crate::error::AppError;
use crate::repository::{user_repo, AppState};
use crate::services::{AuditLogEntry, AuditService};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

/// Product owner, 2026-11-14: an invitation link is valid for 5 minutes.
pub const INVITATION_TTL_MINUTES: i64 = 5;
/// Invitations one Admin may send per day (defence against a compromised session).
const INVITES_PER_ADMIN_PER_DAY: u32 = 20;
/// Public lookups/accepts per IP per hour; the token itself is unguessable.
const PUBLIC_ATTEMPTS_PER_IP_PER_HOUR: u32 = 30;
const DEV_ADMIN_WEB_URL: &str = "http://127.0.0.1:3000";
/// Production Admin Web origin (product owner, 2026-11-14). EMAIL_ADMIN_WEB_URL
/// overrides it, e.g. for staging.
pub const DEFAULT_ADMIN_WEB_URL: &str = "https://admin.afyamkononi.app";
/// ip_jail account key shared by the public invitation endpoints.
const PUBLIC_JAIL_KEY: &str = "admin-invitation";

/// Invitation emails are stricter than validate_email: a plain
/// local@domain of letters, digits and . _ % + -, so nothing that could be
/// markup (<, >, quotes) or a mail-header break (CR/LF, commas,
/// semicolons) is ever stored, rendered or used as a recipient.
pub fn validate_invite_email(email: &str) -> Result<String, AppError> {
    validate_email(email)?;
    let email = crate::domain::validation::normalize_email(email);
    let invalid = || {
        AppError::Validation(
            "Enter a plain email address (letters, digits and . _ % + - only).".into(),
        )
    };
    let (local, domain) = email.split_once('@').ok_or_else(invalid)?;
    let local_ok = !local.is_empty()
        && local.len() <= 64
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._%+-".contains(c));
    let domain_ok = domain.len() <= 253
        && domain.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if !local_ok
        || !domain_ok
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
    {
        return Err(invalid());
    }
    Ok(email)
}

/// A unique-constraint race (email or phone taken between the check and the
/// insert) is a conflict for the caller, not a server error.
fn unique_violation_as_conflict(error: sqlx::Error) -> AppError {
    match &error {
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505") => AppError::Conflict(
            "This email or phone number is already used by another account.".into(),
        ),
        _ => error.into(),
    }
}

pub struct AdminInvitationService;

/// (id, email, phone_number, is_active, created_at)
/// (id, email, phone_number, is_active, created_at, invited_by_email, invited_at)
type AdminRow = (
    Uuid,
    Option<String>,
    String,
    bool,
    DateTime<Utc>,
    Option<String>,
    Option<DateTime<Utc>>,
);
/// (id, email, invited_by, created_at, expires_at, accepted_at, revoked_at)
type InvitationRow = (
    Uuid,
    String,
    Uuid,
    DateTime<Utc>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<DateTime<Utc>>,
);

/// Result of creating an invitation. `dev_invite_url` is only ever `Some` in
/// development, where email delivery may be off (same precedent as
/// `debug_otp_code`).
pub struct CreatedInvitation {
    pub invitation: serde_json::Value,
    pub email_sent: bool,
    pub dev_invite_url: Option<String>,
}

fn new_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn invalid_link() -> AppError {
    AppError::Unauthorized(
        "This invitation link is invalid or has expired. Ask an Admin for a new one.".into(),
    )
}

fn status_of(
    accepted: Option<DateTime<Utc>>,
    revoked: Option<DateTime<Utc>>,
    expires: DateTime<Utc>,
) -> &'static str {
    if accepted.is_some() {
        "ACCEPTED"
    } else if revoked.is_some() {
        "REVOKED"
    } else if expires <= Utc::now() {
        "EXPIRED"
    } else {
        "PENDING"
    }
}

impl AdminInvitationService {
    fn is_development(state: &AppState) -> bool {
        state.config.environment == "development"
    }

    /// Outside development an invitation is only useful if it can be emailed
    /// with a working link; refuse up front instead of creating a dead invite.
    fn email_link_base(state: &AppState) -> Result<Option<String>, AppError> {
        // Development links only to an explicitly configured origin; every
        // other environment falls back to the production Admin Web.
        let configured = if Self::is_development(state) {
            state.config.email_admin_web_url.clone()
        } else {
            Some(
                state
                    .config
                    .email_admin_web_url
                    .clone()
                    .unwrap_or_else(|| DEFAULT_ADMIN_WEB_URL.to_string()),
            )
        };
        let usable = configured
            .as_deref()
            .and_then(|u| crate::services::email_templates::admin_invite_url(u, "00"))
            .is_some();
        if Self::is_development(state) {
            return Ok(configured.filter(|_| usable));
        }
        if !usable || !state.email_transport.is_configured() {
            return Err(AppError::ServiceUnavailable(
                "Admin invitations need email delivery and EMAIL_ADMIN_WEB_URL (https) to be configured.".into(),
            ));
        }
        Ok(configured)
    }

    /// Admins and invitations for the Admin Team page.
    pub async fn team(state: &AppState, ctx: &AuthContext) -> Result<serde_json::Value, AppError> {
        let admins: Vec<AdminRow> = sqlx::query_as(
            "SELECT u.id, u.email, u.phone_number, u.is_active, u.created_at, inviter.email, i.created_at              FROM users u              LEFT JOIN admin_invitations i ON i.accepted_user_id = u.id              LEFT JOIN users inviter ON inviter.id = i.invited_by              WHERE u.role='ADMIN' AND u.deleted_at IS NULL ORDER BY u.created_at",
        )
        .fetch_all(&state.auth_db)
        .await?;
        let email_of = |id: Uuid| admins.iter().find(|a| a.0 == id).and_then(|a| a.1.clone());
        let mut tx = state.begin_authed(ctx).await?;
        let rows: Vec<InvitationRow> = sqlx::query_as(
            "SELECT id, email, invited_by, created_at, expires_at, accepted_at, revoked_at FROM admin_invitations ORDER BY created_at DESC LIMIT 200",
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        let mask = |phone: &str| {
            let tail: String = phone
                .chars()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("•••• {tail}")
        };
        Ok(serde_json::json!({
            "admins": admins.iter().map(|(id, email, phone, active, created, invited_by, invited_at)| serde_json::json!({
                "id": id, "email": email, "phone": mask(phone), "is_active": active, "created_at": created,
                "is_you": *id == ctx.user_id.into_inner(),
                // None = not created through an invitation (the first Admin, set up by an operator).
                "invited_by": invited_by, "invited_at": invited_at,
            })).collect::<Vec<_>>(),
            "invitations": rows.iter().map(|(id, email, by, created, expires, accepted, revoked)| serde_json::json!({
                "id": id, "email": email, "invited_by": email_of(*by), "created_at": created, "expires_at": expires,
                "accepted_at": accepted, "revoked_at": revoked, "status": status_of(*accepted, *revoked, *expires),
            })).collect::<Vec<_>>(),
        }))
    }

    /// Creates an invitation and emails it. `replaces` revokes that open
    /// invitation in the same transaction (resend).
    pub async fn invite(
        state: &AppState,
        ctx: &AuthContext,
        email: &str,
        replaces: Option<Uuid>,
    ) -> Result<CreatedInvitation, AppError> {
        let email = validate_invite_email(email)?;
        let base = Self::email_link_base(state)?;
        crate::rate_limit::check_and_increment(
            &state.redis,
            &format!("ratelimit:admin-invite:{}", ctx.user_id),
            INVITES_PER_ADMIN_PER_DAY,
            86_400,
        )
        .await?;
        if user_repo::find_by_email(&state.auth_db, &email)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(
                "An account already uses this email. Invite a different address.".into(),
            ));
        }

        let token = new_token();
        let id = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::minutes(INVITATION_TTL_MINUTES);
        let mut tx = state.begin_authed(ctx).await?;
        if let Some(previous) = replaces {
            let revoked = sqlx::query(
                "UPDATE admin_invitations SET revoked_at=now(), revoked_by=$2 WHERE id=$1 AND email=$3 AND accepted_at IS NULL AND revoked_at IS NULL",
            )
            .bind(previous)
            .bind(ctx.user_id)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
            if revoked.rows_affected() != 1 {
                return Err(AppError::Conflict(
                    "This invitation can no longer be resent.".into(),
                ));
            }
        }
        // An expired, never-used invitation should not block a new one.
        sqlx::query(
            "UPDATE admin_invitations SET revoked_at=now(), revoked_by=$2 WHERE email=$1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at <= now()",
        )
        .bind(&email)
        .bind(ctx.user_id)
        .execute(&mut *tx)
        .await?;
        let inserted = sqlx::query(
            "INSERT INTO admin_invitations (id, email, token_hash, invited_by, expires_at) VALUES ($1,$2,$3,$4,$5) \
             ON CONFLICT (email) WHERE accepted_at IS NULL AND revoked_at IS NULL DO NOTHING",
        )
        .bind(id)
        .bind(&email)
        .bind(crypto::hash_code(&token))
        .bind(ctx.user_id)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "An invitation is already pending for this email. Resend or revoke it.".into(),
            ));
        }
        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                if replaces.is_some() {
                    "ADMIN_INVITATION_RESENT"
                } else {
                    "ADMIN_INVITATION_SENT"
                },
                "ADMIN_INVITATION",
                Some(id),
                "SUCCESS",
            )
            .with_after(serde_json::json!({ "expires_at": expires_at })),
        )
        .await?;
        tx.commit().await?;

        let mut email_sent = false;
        if let Some(url) = base
            .as_deref()
            .and_then(|b| crate::services::email_templates::admin_invite_url(b, &token))
        {
            if state.email_transport.is_configured() {
                let rendered = crate::services::email_templates::render_admin_invitation(
                    &url,
                    INVITATION_TTL_MINUTES,
                    &state.email_assets,
                );
                email_delivery::send_templated_email(
                    state,
                    &email,
                    &rendered,
                    TransactionalEmailKind::AdminInvitation,
                    &format!("admin-invitation:{id}"),
                )
                .await;
                email_sent = true;
            }
        }
        let dev_invite_url = Self::is_development(state).then(|| {
            let base = state
                .config
                .email_admin_web_url
                .as_deref()
                .unwrap_or(DEV_ADMIN_WEB_URL);
            format!("{}/invite?token={token}", base.trim_end_matches('/'))
        });
        Ok(CreatedInvitation {
            invitation: serde_json::json!({ "id": id, "email": email, "expires_at": expires_at, "status": "PENDING" }),
            email_sent,
            dev_invite_url,
        })
    }

    pub async fn resend(
        state: &AppState,
        ctx: &AuthContext,
        id: Uuid,
    ) -> Result<CreatedInvitation, AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let email: Option<String> = sqlx::query_scalar(
            "SELECT email FROM admin_invitations WHERE id=$1 AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        let email = email
            .ok_or_else(|| AppError::Conflict("This invitation can no longer be resent.".into()))?;
        Self::invite(state, ctx, &email, Some(id)).await
    }

    pub async fn revoke(state: &AppState, ctx: &AuthContext, id: Uuid) -> Result<(), AppError> {
        let mut tx = state.begin_authed(ctx).await?;
        let revoked = sqlx::query(
            "UPDATE admin_invitations SET revoked_at=now(), revoked_by=$2 WHERE id=$1 AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(id)
        .bind(ctx.user_id)
        .execute(&mut *tx)
        .await?;
        if revoked.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "This invitation is already accepted or revoked.".into(),
            ));
        }
        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                "ADMIN_INVITATION_REVOKED",
                "ADMIN_INVITATION",
                Some(id),
                "SUCCESS",
            ),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Public endpoints: IP jail (shared with sign-in) plus a per-IP budget.
    async fn public_limit(state: &AppState, ip: &str) -> Result<(), AppError> {
        if crate::ip_jail::is_ip_jailed(&state.redis, ip).await? {
            return Err(AppError::Forbidden(
                "Too many failed attempts from this network. Try again later.".into(),
            ));
        }
        crate::rate_limit::check_and_increment(
            &state.redis,
            &format!("ratelimit:admin-invite-public:{ip}"),
            PUBLIC_ATTEMPTS_PER_IP_PER_HOUR,
            3_600,
        )
        .await
    }

    /// Malformed tokens count as failed attempts too.
    async fn checked_token_hash(
        state: &AppState,
        ip: &str,
        token: &str,
    ) -> Result<String, AppError> {
        match Self::token_hash(token) {
            Ok(hash) => Ok(hash),
            Err(error) => {
                crate::ip_jail::record_failure(&state.redis, ip, PUBLIC_JAIL_KEY).await?;
                Err(error)
            }
        }
    }

    fn token_hash(token: &str) -> Result<String, AppError> {
        if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(invalid_link());
        }
        Ok(crypto::hash_code(&token.to_ascii_lowercase()))
    }

    /// Public: which email an open, unexpired invitation is for.
    pub async fn lookup(
        state: &AppState,
        ip: &str,
        token: &str,
    ) -> Result<serde_json::Value, AppError> {
        Self::public_limit(state, ip).await?;
        let hash = Self::checked_token_hash(state, ip, token).await?;
        let row: Option<(String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT email, expires_at FROM admin_invitations WHERE token_hash=$1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > now()",
        )
        .bind(hash)
        .fetch_optional(&state.auth_db)
        .await?;
        let Some((email, expires_at)) = row else {
            crate::ip_jail::record_failure(&state.redis, ip, PUBLIC_JAIL_KEY).await?;
            return Err(invalid_link());
        };
        Ok(serde_json::json!({ "email": email, "expires_at": expires_at }))
    }

    /// Public: creates the Admin account and consumes the invitation, atomically.
    pub async fn accept(
        state: &AppState,
        ip: &str,
        token: &str,
        phone_number: &str,
        password: &str,
    ) -> Result<serde_json::Value, AppError> {
        Self::public_limit(state, ip).await?;
        let hash = Self::checked_token_hash(state, ip, token).await?;
        let phone_number = normalize_kenya_phone_number(phone_number)?;
        validate_password_strength(password)?;
        // Cheap existence check before the expensive Argon2id hash, so junk
        // tokens cannot be used to make the server hash passwords.
        let live: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM admin_invitations WHERE token_hash=$1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > now())",
        )
        .bind(&hash)
        .fetch_one(&state.auth_db)
        .await?;
        if !live {
            crate::ip_jail::record_failure(&state.redis, ip, PUBLIC_JAIL_KEY).await?;
            return Err(invalid_link());
        }
        let password_hash = crypto::hash_password_async(password.to_owned()).await?;

        let mut tx = state.auth_db.begin().await?;
        let row: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, email FROM admin_invitations WHERE token_hash=$1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > now() FOR UPDATE",
        )
        .bind(hash)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((invitation_id, email)) = row else {
            tx.rollback().await?;
            crate::ip_jail::record_failure(&state.redis, ip, PUBLIC_JAIL_KEY).await?;
            return Err(invalid_link());
        };
        let taken: (bool, bool) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email=$1), EXISTS(SELECT 1 FROM users WHERE phone_number=$2)",
        )
        .bind(&email)
        .bind(&phone_number)
        .fetch_one(&mut *tx)
        .await?;
        if taken.0 {
            return Err(AppError::Conflict(
                "An account already uses this email. Ask an Admin for help.".into(),
            ));
        }
        if taken.1 {
            return Err(AppError::Conflict(
                "This phone number is already used by another account.".into(),
            ));
        }
        let user_id = UserId::new();
        let now = Utc::now();
        user_repo::insert_user(
            &mut *tx,
            &User {
                id: user_id,
                phone_number,
                email: Some(email.clone()),
                password_hash: Some(password_hash),
                role: UserRole::Admin,
                preferred_language: "en".into(),
                is_active: true,
                is_verified: true,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            },
        )
        .await
        .map_err(unique_violation_as_conflict)?;
        sqlx::query(
            "UPDATE admin_invitations SET accepted_at=now(), accepted_user_id=$2 WHERE id=$1",
        )
        .bind(invitation_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                // The new Admin accepted; the Activity Log names them by email.
                Some(user_id),
                UserRole::Admin,
                "ADMIN_INVITATION_ACCEPTED",
                "USER",
                Some(user_id.into_inner()),
                "SUCCESS",
            )
            .with_reason(Some(format!(
                "Admin account created by accepting invitation {invitation_id}."
            ))),
        )
        .await?;
        tx.commit().await?;
        Ok(serde_json::json!({ "email": email }))
    }
}
