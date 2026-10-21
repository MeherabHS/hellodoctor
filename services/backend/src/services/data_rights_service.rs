//! Admin-executed Data Protection Act 2019 (Kenya) §40 erasure requests.
//! §38 portability (export) needs no new code — `get_patient_dossier` /
//! `get_doctor_dossier` already assemble the full nested record as JSON.
//!
//! Erasure always attempts a real `DELETE` on the profile row first, inside
//! a `SAVEPOINT` (see `attempt_hard_delete`'s doc comment for why a savepoint
//! rather than a plain `DELETE` + ignored error). Postgres's own `ON DELETE
//! RESTRICT` foreign keys (appointments, grievance_reports, doctor_wallets,
//! prescriptions, etc. all reference `patient_profiles`/`doctor_profiles`
//! this way) are the single source of truth for "does this profile have
//! retained clinical, professional, or financial history" — never a
//! hand-maintained list of tables to check, which would silently go stale
//! the next time a new table adds such a reference. When the DELETE is
//! blocked, the caller rolls back just that savepoint and anonymizes the
//! profile's personally-identifying fields in place instead, leaving every
//! clinical/financial record exactly as it was, still pointing at the same
//! (now-anonymized) profile id.
//!
//! `transactions`/`disbursement_items`/etc. are never targeted by either
//! path — only the profile row itself is ever written here — so the Income
//! Tax Act/VAT Act 2013 §43 five-year financial-record retention
//! requirement is satisfied by construction, not by a runtime check that
//! could be bypassed.
//!
//! The corresponding `users` login row (if any — a guardian-managed
//! dependent profile has none, see `PatientProfile::user_id`) is anonymized
//! as a second, separate step under the `hellodoctor_auth_worker` pool:
//! `hellodoctor_api` has `UPDATE` on `patient_profiles`/`doctor_profiles`
//! but not on `users` (only `hellodoctor_auth_worker` does, migration
//! 20261003000001), so this cannot share one transaction with the profile
//! write above. If this second step fails after the first already
//! committed, a CRITICAL `system_error_logs` row is reported so an admin can
//! finish it manually — the honest outcome, not a silently inconsistent
//! partial success.

use crate::abac::{self, Action};
use crate::auth_context::AuthContext;
use crate::domain::ids::{DoctorProfileId, PatientProfileId, UserId};
use crate::error::AppError;
use crate::repository::{doctor_repo, patient_repo, user_repo, AppState, DbRole};
use crate::services::{AuditLogEntry, AuditService};
use crate::system_error::{self, ErrorEvent};
use sqlx::{Acquire, Postgres};
use uuid::Uuid;

pub struct DataRightsOutcome {
    pub outcome: &'static str,
}

/// First 12 hex characters of the id's simple (no-hyphen) form — short
/// enough to fit `VARCHAR(20)` phone-number columns alongside a prefix,
/// derived from the row's own id so it is unique without a lookup.
fn short_suffix(id: Uuid) -> String {
    id.simple().to_string()[..12].to_string()
}

/// Attempts `DELETE FROM {table} WHERE id = $1` inside a `SAVEPOINT`, so a
/// blocked delete (FK `RESTRICT` violation) rolls back only that savepoint —
/// never poisoning the caller's outer transaction, which still needs to run
/// the anonymization `UPDATE` and the audit-log `INSERT` afterward. Mirrors
/// `notification_repo::insert_if_new`'s established savepoint-via-`Acquire`
/// pattern exactly.
async fn attempt_hard_delete<'e, A>(
    executor: A,
    table: &'static str,
    id: Uuid,
) -> Result<bool, AppError>
where
    A: Acquire<'e, Database = Postgres> + Send,
{
    let mut savepoint = executor.begin().await?;
    let query = match table {
        "patient_profiles" => "DELETE FROM patient_profiles WHERE id = $1",
        "doctor_profiles" => "DELETE FROM doctor_profiles WHERE id = $1",
        _ => {
            unreachable!("attempt_hard_delete is only ever called with a literal table name above")
        }
    };
    let deleted = match sqlx::query(query).bind(id).execute(&mut *savepoint).await {
        Ok(_) => {
            savepoint.commit().await?;
            true
        }
        Err(_restrict_violation) => {
            savepoint.rollback().await?;
            false
        }
    };
    Ok(deleted)
}

/// Anonymizes the `users` login row under the one pool that can write it.
/// Reports (rather than propagates) a failure here: the profile-row
/// transaction already committed by the time this runs, so there is no
/// outer transaction left to roll back into a clean all-or-nothing state —
/// surfacing the failure as a CRITICAL system error for manual follow-up is
/// the most honest available outcome.
async fn anonymize_login(state: &AppState, ctx: &AuthContext, user_id: UserId) {
    let anon_phone = format!("ERASED{}", short_suffix(user_id.into_inner()));
    let result = async {
        let mut tx = state.begin_authed_as(ctx, DbRole::AuthWorker).await?;
        user_repo::anonymize_and_deactivate(&mut *tx, user_id, &anon_phone).await?;
        tx.commit().await?;
        Ok::<(), AppError>(())
    }
    .await;
    if let Err(error) = result {
        let event = ErrorEvent::new(
            crate::domain::models::ErrorSeverity::Critical,
            "DATA_RIGHTS",
            "data_rights_service::anonymize_login",
            "LOGIN_ANONYMIZE_FAILED",
            format!(
                "Profile erased/anonymized but the linked login row failed to anonymize: {error}. Manual follow-up required."
            ),
            crate::api::middleware::request_context::request_id(),
        )
        .with_user(user_id.into_inner());
        system_error::report(event);
    }
}

/// Entry point for `POST /api/v1/admin/patients/:id/data-rights/erase`.
pub async fn execute_patient_erasure(
    state: &AppState,
    ctx: &AuthContext,
    patient_id: PatientProfileId,
    reason: &str,
) -> Result<DataRightsOutcome, AppError> {
    abac::authorize(ctx, Action::ExecuteDataRightsAction)?;
    let mut tx = state.begin_authed(ctx).await?;

    let patient = patient_repo::find_by_id(&mut *tx, patient_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Patient profile not found.".into()))?;
    let before = serde_json::json!({
        "full_name": patient.full_name,
        "has_login": patient.user_id.is_some(),
    });

    let hard_deleted =
        attempt_hard_delete(&mut *tx, "patient_profiles", patient_id.into_inner()).await?;

    if !hard_deleted {
        let anon_name = format!("Deleted Patient {}", short_suffix(patient_id.into_inner()));
        sqlx::query(
            "UPDATE patient_profiles SET full_name=$2, date_of_birth=DATE '1900-01-01', \
             blood_group=NULL, weight_kg=NULL, emergency_contact_phone=NULL, \
             emergency_contact_relation=NULL, updated_at=NOW() WHERE id=$1",
        )
        .bind(patient_id)
        .bind(&anon_name)
        .execute(&mut *tx)
        .await?;
    }

    let action = if hard_deleted {
        "DATA_RIGHTS_PATIENT_ERASED"
    } else {
        "DATA_RIGHTS_PATIENT_ANONYMIZED"
    };
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            action,
            "PATIENT_PROFILE",
            Some(patient_id.into_inner()),
            "SUCCESS",
        )
        .with_reason(Some(reason.to_string()))
        .with_before(before),
    )
    .await?;
    tx.commit().await?;

    if let Some(user_id) = patient.user_id {
        anonymize_login(state, ctx, user_id).await;
    }

    Ok(DataRightsOutcome {
        outcome: if hard_deleted {
            "HARD_DELETED"
        } else {
            "ANONYMIZED"
        },
    })
}

/// Entry point for `POST /api/v1/admin/doctors/:id/data-rights/erase`. Only
/// personal/contact fields are scrubbed on the anonymize path — license
/// number, license authority, qualifications, specialty, and experience
/// years are KMPDC professional-regulatory record, not personal data, and
/// stay intact so past consultations' credentialing trail remains legible
/// (CLAUDE.md §2.1: physician qualification is represented exclusively by
/// verified KMPDC registration). A real hard delete is realistically almost
/// never reachable for a doctor with any consultation history, since every
/// doctor has a `doctor_wallets` row (`ON DELETE RESTRICT`) from onboarding
/// — that is expected and correct, not a bug.
pub async fn execute_doctor_erasure(
    state: &AppState,
    ctx: &AuthContext,
    doctor_id: DoctorProfileId,
    reason: &str,
) -> Result<DataRightsOutcome, AppError> {
    abac::authorize(ctx, Action::ExecuteDataRightsAction)?;
    let mut tx = state.begin_authed(ctx).await?;

    let doctor = doctor_repo::find_by_id(&mut *tx, doctor_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Doctor profile not found.".into()))?;
    let before = serde_json::json!({
        "full_name": doctor.full_name,
        "license_number": doctor.license_number,
    });

    let hard_deleted =
        attempt_hard_delete(&mut *tx, "doctor_profiles", doctor_id.into_inner()).await?;

    if !hard_deleted {
        let anon_name = format!("Deleted Doctor {}", short_suffix(doctor_id.into_inner()));
        let anon_phone = format!("ERASED{}", short_suffix(doctor_id.into_inner()));
        sqlx::query(
            // Deliberately leaves `deleted_at` untouched: every doctor_repo
            // read (including get_doctor_dossier's own lookup of this exact
            // row) filters `WHERE deleted_at IS NULL`, so setting it here
            // would make this now-anonymized-but-still-on-record profile
            // invisible to the admin dossier view that is supposed to let
            // compliance keep auditing it — confirmed by a real failing
            // test, not assumed. The linked `users` row is already
            // deactivated (anonymize_login) and `is_on_duty=FALSE` below
            // already keeps this doctor unbookable, so nothing is lost.
            "UPDATE doctor_profiles SET full_name=$2, residential_address='[ERASED]', \
             verified_phone=$3, is_on_duty=FALSE, updated_at=NOW() WHERE id=$1",
        )
        .bind(doctor_id)
        .bind(&anon_name)
        .bind(&anon_phone)
        .execute(&mut *tx)
        .await?;
    }

    let action = if hard_deleted {
        "DATA_RIGHTS_DOCTOR_ERASED"
    } else {
        "DATA_RIGHTS_DOCTOR_ANONYMIZED"
    };
    AuditService::log_in_tx(
        &mut *tx,
        AuditLogEntry::new(
            Some(ctx.user_id),
            ctx.role,
            action,
            "DOCTOR_PROFILE",
            Some(doctor_id.into_inner()),
            "SUCCESS",
        )
        .with_reason(Some(reason.to_string()))
        .with_before(before),
    )
    .await?;
    tx.commit().await?;

    anonymize_login(state, ctx, doctor.user_id).await;

    Ok(DataRightsOutcome {
        outcome: if hard_deleted {
            "HARD_DELETED"
        } else {
            "ANONYMIZED"
        },
    })
}
