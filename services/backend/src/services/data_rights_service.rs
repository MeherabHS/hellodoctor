//! Durable, capability-separated erasure. Completion means the login is disabled,
//! sessions revoked, profile scrubbed, device registrations disabled and personal
//! avatar deletion confirmed. Clinical/financial history is retained by its FKs.
use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId, UserId};
use crate::error::AppError;
use crate::repository::{user_repo, AppState, DbRole};
use crate::services::document_storage_service::DocumentStorageService;
use crate::services::{AuditLogEntry, AuditService};
use sqlx::{Acquire, PgConnection, Postgres, Transaction};
use uuid::Uuid;

pub struct DataRightsOutcome {
    pub outcome: &'static str,
    pub job_id: Uuid,
}
#[derive(sqlx::FromRow)]
struct Job {
    id: Uuid,
    profile_kind: String,
    profile_id: Uuid,
    user_id: Option<UserId>,
    requested_by: UserId,
    state: String,
    outcome: Option<String>,
    photo_key: Option<String>,
    photo_bucket: Option<String>,
}
fn outcome(job: Job) -> DataRightsOutcome {
    DataRightsOutcome {
        job_id: job.id,
        outcome: if job.state == "COMPLETED" {
            match job.outcome.as_deref() {
                Some("HARD_DELETED") => "HARD_DELETED",
                Some("ANONYMIZED") => "ANONYMIZED",
                _ => "PENDING",
            }
        } else {
            "PENDING"
        },
    }
}
fn table(kind: &str) -> Result<&'static str, AppError> {
    match kind {
        "PATIENT" => Ok("patient_profiles"),
        "DOCTOR" => Ok("doctor_profiles"),
        _ => Err(AppError::Conflict("Invalid erasure subject.".into())),
    }
}
pub(crate) fn short_suffix(id: Uuid) -> String {
    id.simple().to_string()[..12].to_string()
}

/// Remediation E1: re-applies the exact same login-deactivation and
/// session-revocation mutations as `deactivate_step` (reusing
/// `user_repo::anonymize_and_deactivate`, never a second hand-rolled delete
/// path), for a user whose `account_erasure_jobs` row does not exist in a
/// restored database because the restore predates the original erasure.
/// Device-token disabling is a separate call (`disable_device_tokens_for`
/// below) mirroring `disable_devices_step`'s own separate transaction — this
/// function makes no atomicity claim beyond its own single update statement.
/// Called only from `dr_reconciliation`, itself only ever invoked by the
/// operator-run `dr_reconcile` tool, never from an HTTP handler.
pub(crate) async fn reapply_erasure_deactivation_for_resurrected_account(
    state: &AppState,
    user: UserId,
) -> Result<(), AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::AuthWorker).await?;
    let phone = format!("ERASED{}", short_suffix(user.into_inner()));
    user_repo::anonymize_and_deactivate(&mut *tx, user, &phone).await?;
    sqlx::query(
        "UPDATE auth_sessions SET revoked_at=COALESCE(revoked_at,clock_timestamp()) WHERE user_id=$1",
    )
    .bind(user)
    .execute(&mut *tx)
    .await?;
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            None,
            ctx.role,
            "DR_RECONCILIATION_REAPPLIED_ERASURE",
            "USER",
            Some(user.into_inner()),
            "SUCCESS",
        )
        .with_reason(Some(
            "Disaster-recovery reconciliation found this account active in a restored \
             database despite a prior completed erasure recorded in the external journal."
                .into(),
        )),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Companion to the above — disables any still-enabled device/push tokens
/// for a resurrected account, mirroring `disable_devices_step` exactly.
pub(crate) async fn disable_device_tokens_for_resurrected_account(
    state: &AppState,
    user: UserId,
) -> Result<(), AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    sqlx::query(
        "UPDATE notification_device_tokens SET enabled=false,updated_at=clock_timestamp() WHERE user_id=$1 AND enabled",
    )
    .bind(user)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

// Registration and durable erasure intent serialize on the account identity.
// A request authenticated earlier must not re-enable devices after intent.
async fn lock_account_registration(conn: &mut PgConnection, user: UserId) -> Result<(), AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock(62031,hashtext($1::uuid::text))")
        .bind(user)
        .execute(conn)
        .await?;
    Ok(())
}
pub(crate) async fn guard_device_registration(
    conn: &mut PgConnection,
    user: UserId,
) -> Result<(), AppError> {
    lock_account_registration(conn, user).await?;
    let erasing: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_erasure_jobs WHERE user_id=$1)")
            .bind(user)
            .fetch_one(conn)
            .await?;
    if erasing {
        return Err(AppError::Forbidden(
            "Device registration is unavailable for this account.".into(),
        ));
    }
    Ok(())
}

pub async fn execute_patient_erasure(
    state: &AppState,
    ctx: &AuthContext,
    id: PatientProfileId,
    _reason: &str,
) -> Result<DataRightsOutcome, AppError> {
    request(state, ctx, id.into_inner(), "PATIENT").await
}
pub async fn execute_doctor_erasure(
    state: &AppState,
    ctx: &AuthContext,
    id: DoctorProfileId,
    _reason: &str,
) -> Result<DataRightsOutcome, AppError> {
    request(state, ctx, id.into_inner(), "DOCTOR").await
}
async fn request(
    state: &AppState,
    ctx: &AuthContext,
    profile: Uuid,
    kind: &str,
) -> Result<DataRightsOutcome, AppError> {
    abac::authorize(ctx, Action::ExecuteDataRightsAction)?;
    let mut tx = state.begin_authed(ctx).await?;
    super::profile_picture_service::lock_profile_picture(&mut *tx, profile).await?;
    let existing: Option<Job> = sqlx::query_as(
        "SELECT * FROM account_erasure_jobs WHERE profile_kind=$1 AND profile_id=$2",
    )
    .bind(kind)
    .bind(profile)
    .fetch_optional(&mut *tx)
    .await?;
    let id = if let Some(existing) = existing {
        existing.id
    } else {
        let sql = format!("SELECT user_id,profile_picture_object_key,profile_picture_bucket FROM {} WHERE id=$1 FOR UPDATE",table(kind)?);
        let row: Option<(Option<UserId>, Option<String>, Option<String>)> = sqlx::query_as(&sql)
            .bind(profile)
            .fetch_optional(&mut *tx)
            .await?;
        let (user, key, bucket) =
            row.ok_or_else(|| AppError::NotFound("Profile not found.".into()))?;
        if let Some(user) = user {
            lock_account_registration(&mut tx, user).await?;
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO account_erasure_jobs(id,profile_kind,profile_id,user_id,requested_by,photo_key,photo_bucket) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(id).bind(kind).bind(profile).bind(user).bind(ctx.user_id).bind(key).bind(bucket).execute(&mut *tx).await?;
        AuditService::log_in_tx(
            &mut *tx,
            AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                "DATA_RIGHTS_ERASURE_REQUESTED",
                "ERASURE_JOB",
                Some(id),
                "SUCCESS",
            )
            .with_reason(Some("Authorized account erasure request".into())),
        )
        .await?;
        id
    };
    tx.commit().await?;
    match claim_job(state, Some(id)).await {
        Ok(Some(claim)) => process_claim(state, id, claim).await,
        Ok(None) => {}
        Err(_) => {
            metrics::counter!("account_erasure_retry_total").increment(1);
        }
    }
    let mut tx = state.begin_authed(ctx).await?;
    let job = sqlx::query_as("SELECT * FROM account_erasure_jobs WHERE id=$1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(outcome(job))
}
async fn claim_job(state: &AppState, only: Option<Uuid>) -> Result<Option<Uuid>, AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let token = Uuid::new_v4();
    let claimed: Option<Uuid> = sqlx::query_scalar("WITH candidate AS (SELECT id FROM account_erasure_jobs WHERE state<>'COMPLETED' AND ($1::uuid IS NULL OR id=$1) AND next_attempt_at<=clock_timestamp() AND (lease_until IS NULL OR lease_until<=clock_timestamp()) ORDER BY next_attempt_at,id LIMIT 1 FOR UPDATE SKIP LOCKED) UPDATE account_erasure_jobs j SET claim_id=$2,lease_until=clock_timestamp()+interval '30 seconds',attempts=LEAST(attempts+1,1000000),updated_at=clock_timestamp() FROM candidate c WHERE j.id=c.id RETURNING j.id")
        .bind(only).bind(token).fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    // The worker also needs the selected id when not processing a specific job.
    Ok(claimed.map(|_| token))
}
async fn locked_job(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    claim: Uuid,
) -> Result<Job, AppError> {
    sqlx::query_as("SELECT * FROM account_erasure_jobs WHERE id=$1 AND claim_id=$2 FOR UPDATE")
        .bind(id)
        .bind(claim)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::Conflict("Erasure claim changed; recovery will retry.".into()))
}
async fn deactivate_step(state: &AppState, id: Uuid, claim: Uuid) -> Result<(), AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::AuthWorker).await?;
    let job = locked_job(&mut tx, id, claim).await?;
    if job.state == "PENDING_LOGIN" {
        if let Some(user) = job.user_id {
            let phone = format!("ERASED{}", short_suffix(user.into_inner()));
            user_repo::anonymize_and_deactivate(&mut *tx, user, &phone).await?;
            sqlx::query("UPDATE auth_sessions SET revoked_at=COALESCE(revoked_at,clock_timestamp()) WHERE user_id=$1")
                .bind(user).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE account_erasure_jobs SET state='PENDING_PROFILE',updated_at=clock_timestamp() WHERE id=$1")
            .bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn scrub_step(state: &AppState, id: Uuid, claim: Uuid) -> Result<(), AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed(&ctx).await?;
    let job = locked_job(&mut tx, id, claim).await?;
    if job.state == "PENDING_PROFILE" {
        super::profile_picture_service::lock_profile_picture(&mut *tx, job.profile_id).await?;
        let mut savepoint = tx.begin().await?;
        let sql = format!("DELETE FROM {} WHERE id=$1", table(&job.profile_kind)?);
        let deleted = match sqlx::query(&sql)
            .bind(job.profile_id)
            .execute(&mut *savepoint)
            .await
        {
            Ok(_) => {
                savepoint.commit().await?;
                true
            }
            Err(e) if e.as_database_error().and_then(|e| e.code()).as_deref() == Some("23503") => {
                savepoint.rollback().await?;
                false
            }
            Err(e) => return Err(e.into()),
        };
        if !deleted {
            let suffix = short_suffix(job.profile_id);
            if job.profile_kind == "PATIENT" {
                sqlx::query("UPDATE patient_profiles SET full_name=$2,date_of_birth=DATE '1900-01-01',blood_group=NULL,weight_kg=NULL,emergency_contact_phone=NULL,emergency_contact_relation=NULL,profile_picture_object_key=NULL,profile_picture_bucket=NULL,profile_picture_content_hash=NULL,profile_picture_mime_type=NULL,profile_picture_size_bytes=NULL,profile_picture_updated_at=NULL,updated_at=clock_timestamp() WHERE id=$1")
                    .bind(job.profile_id).bind(format!("Deleted Patient {suffix}")).execute(&mut *tx).await?;
            } else {
                sqlx::query("UPDATE doctor_profiles SET full_name=$2,residential_address='[ERASED]',verified_phone=$3,is_on_duty=false,profile_picture_object_key=NULL,profile_picture_bucket=NULL,profile_picture_content_hash=NULL,profile_picture_mime_type=NULL,profile_picture_size_bytes=NULL,profile_picture_updated_at=NULL,updated_at=clock_timestamp() WHERE id=$1")
                    .bind(job.profile_id).bind(format!("Deleted Doctor {suffix}")).bind(format!("ERASED{suffix}")).execute(&mut *tx).await?;
            }
        }
        sqlx::query("UPDATE account_erasure_jobs SET state='PENDING_MEDIA',outcome=$2,updated_at=clock_timestamp() WHERE id=$1")
            .bind(id).bind(if deleted {"HARD_DELETED"} else {"ANONYMIZED"}).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn disable_devices_step(state: &AppState, id: Uuid, claim: Uuid) -> Result<(), AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let job = locked_job(&mut tx, id, claim).await?;
    if let Some(user) = job.user_id {
        sqlx::query("UPDATE notification_device_tokens SET enabled=false,updated_at=clock_timestamp() WHERE user_id=$1 AND enabled")
            .bind(user).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn finish_step(state: &AppState, id: Uuid, claim: Uuid) -> Result<(), AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let job = locked_job(&mut tx, id, claim).await?;
    if job.state == "PENDING_MEDIA" {
        if let Some(key) = &job.photo_key {
            let prefix = format!(
                "avatars/{}/{}/",
                job.profile_kind.to_lowercase(),
                job.profile_id
            );
            let valid = key
                .strip_prefix(&prefix)
                .and_then(|s| s.strip_suffix(".bin"))
                .is_some_and(|s| Uuid::parse_str(s).is_ok());
            if !valid || job.photo_bucket.as_deref() != Some(state.config.storage_bucket.as_str()) {
                return Err(AppError::Conflict(
                    "Erasure media ownership requires review.".into(),
                ));
            }
            DocumentStorageService::delete_strict(state, key).await?;
        }
        sqlx::query("UPDATE account_erasure_jobs SET state='COMPLETED',claim_id=NULL,lease_until=NULL,last_error_code=NULL,updated_at=clock_timestamp() WHERE id=$1")
            .bind(id).execute(&mut *tx).await?;
        // Remediation E1: durably record erasure *intent* in the same
        // transaction as the completion itself, so a crash between this
        // commit and the external-journal sync never loses the obligation —
        // the outbox row is retried by `dr_reconciliation::sync_outbox_to_journal`
        // until the external, independently-custodied journal confirms it.
        // Only accounts with a login identity are DR-resurrection-relevant;
        // a profile-only erasure with no linked user has nothing to protect.
        if let Some(user) = job.user_id {
            let subject_ref = crate::services::dr_erasure_journal::pseudonymous_subject_ref(
                &user.into_inner().to_string(),
                "hellodoctor-user",
            );
            sqlx::query(
                "INSERT INTO durable_erasure_journal_outbox(erasure_job_id,event_id,subject_ref,state,retention_class) \
                 VALUES ($1,$2,$3,'COMPLETED','clinical-retention') ON CONFLICT (event_id) DO NOTHING",
            )
            .bind(id)
            .bind(format!("erasure:{id}"))
            .bind(subject_ref)
            .execute(&mut *tx)
            .await?;
        }
        let action = match (job.profile_kind.as_str(), job.outcome.as_deref()) {
            ("PATIENT", Some("HARD_DELETED")) => "DATA_RIGHTS_PATIENT_ERASED",
            ("PATIENT", _) => "DATA_RIGHTS_PATIENT_ANONYMIZED",
            ("DOCTOR", Some("HARD_DELETED")) => "DATA_RIGHTS_DOCTOR_ERASED",
            _ => "DATA_RIGHTS_DOCTOR_ANONYMIZED",
        };
        AuditService::log_in_tx(&mut *tx,AuditLogEntry::new(Some(job.requested_by),ctx.role,action,
            if job.profile_kind=="PATIENT" {"PATIENT_PROFILE"} else {"DOCTOR_PROFILE"},Some(job.profile_id),"SUCCESS")
            .with_after(serde_json::json!({"outcome":job.outcome,"login_deactivated":job.user_id.is_some(),"media_cleanup":"CONFIRMED"}))).await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn process_claim(state: &AppState, id: Uuid, claim: Uuid) {
    let result = async {
        deactivate_step(state, id, claim).await?;
        disable_devices_step(state, id, claim).await?;
        scrub_step(state, id, claim).await?;
        finish_step(state, id, claim).await?;
        Ok::<(), AppError>(())
    }
    .await;
    if result.is_err() {
        // Fixed diagnostic code, never raw SQL/storage errors, object keys or PII.
        metrics::counter!("account_erasure_retry_total").increment(1);
        if let Ok(mut tx) = state
            .begin_authed_as(&AuthContext::system(), DbRole::SystemWorker)
            .await
        {
            let _ = sqlx::query("UPDATE account_erasure_jobs SET claim_id=NULL,lease_until=NULL,next_attempt_at=clock_timestamp()+interval '1 minute',last_error_code='ERASURE_STEP_FAILED',updated_at=clock_timestamp() WHERE id=$1 AND claim_id=$2 AND state<>'COMPLETED'")
                .bind(id).bind(claim).execute(&mut *tx).await;
            let _ = tx.commit().await;
        }
    }
}
/// Small recovery pass called by the existing lifecycle scheduler, no new queue.
pub async fn retry_pending_erasures(state: &AppState) -> Result<u64, AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    let ids: Vec<Uuid>=sqlx::query_scalar("SELECT id FROM account_erasure_jobs WHERE state<>'COMPLETED' AND next_attempt_at<=clock_timestamp() AND (lease_until IS NULL OR lease_until<=clock_timestamp()) ORDER BY next_attempt_at,id LIMIT 10")
        .fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let mut attempted = 0;
    for id in ids {
        if let Some(claim) = claim_job(state, Some(id)).await? {
            process_claim(state, id, claim).await;
            attempted += 1;
        }
    }
    Ok(attempted)
}
