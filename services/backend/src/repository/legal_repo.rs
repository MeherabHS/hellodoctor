use crate::domain::ids::UserId;
use crate::domain::models::LegalAcceptance;
use sqlx::PgExecutor;

/// All documents currently required/offered. Used by the public
/// `GET /legal/documents` endpoint and, pre-session, by
/// `auth_handlers::doctor_register`'s required-set check — both run through
/// `state.auth_db` directly, the same way that handler already looks up
/// duplicate license/phone/email before a session exists.
pub async fn list_current<'e, E: PgExecutor<'e>>(
    executor: E,
) -> Result<Vec<crate::domain::models::LegalDocument>, sqlx::Error> {
    sqlx::query_as::<_, crate::domain::models::LegalDocument>(
        "SELECT id, document_type, version, title, body, document_hash, required, is_current, created_at \
         FROM legal_documents WHERE is_current ORDER BY document_type",
    )
    .fetch_all(executor)
    .await
}

/// Appends one acceptance row. Callers insert one per submitted document
/// inside the same transaction as the rest of `doctor_register` (after the
/// `doctor_profiles`/`doctor_wallets` inserts, before commit), re-borrowing
/// the transaction each call exactly like the existing
/// `doctor_repo::insert`/`audit_repo::insert` call sites do — so the account
/// and its acknowledgements land atomically or not at all.
pub async fn insert<'e, E: PgExecutor<'e>>(
    executor: E,
    row: &LegalAcceptance,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO legal_acceptances \
         (id, user_id, document_type, document_version, document_hash, accepted_at, \
          request_id, session_id, ip_hash, app_version, platform, locale, created_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
    )
    .bind(row.id)
    .bind(row.user_id)
    .bind(row.document_type)
    .bind(&row.document_version)
    .bind(&row.document_hash)
    .bind(row.accepted_at)
    .bind(&row.request_id)
    .bind(row.session_id)
    .bind(&row.ip_hash)
    .bind(&row.app_version)
    .bind(&row.platform)
    .bind(&row.locale)
    .bind(row.created_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Backs both a Doctor's own acceptance history and the admin/compliance
/// evidence view — callers filter authorization before calling this, not
/// inside it (see `abac::Action::ViewLegalAcceptances`).
pub async fn list_for_user<'e, E: PgExecutor<'e>>(
    executor: E,
    user_id: UserId,
) -> Result<Vec<LegalAcceptance>, sqlx::Error> {
    sqlx::query_as::<_, LegalAcceptance>(
        "SELECT id, user_id, document_type, document_version, document_hash, accepted_at, \
              request_id, session_id, ip_hash, app_version, platform, locale, created_at \
         FROM legal_acceptances WHERE user_id = $1 ORDER BY accepted_at DESC",
    )
    .bind(user_id)
    .fetch_all(executor)
    .await
}
