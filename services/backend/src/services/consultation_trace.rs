use crate::{
    auth_context::AuthContext,
    domain::models::UserRole,
    error::AppError,
    repository::{AppState, DbRole},
    services::audit_service::{AuditLogEntry, AuditService},
};
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

pub struct Event<'a> {
    pub event: &'a str,
    pub resource: Option<(&'a str, Uuid)>,
    pub result: &'a str,
    pub reason: Option<&'a str>,
    pub metadata: Value,
}
impl<'a> Event<'a> {
    pub fn accepted(event: &'a str, resource: Option<(&'a str, Uuid)>) -> Self {
        Self {
            event,
            resource,
            result: "ACCEPTED",
            reason: None,
            metadata: json!({}),
        }
    }
    pub fn rejected(event: &'a str, reason: &'a str) -> Self {
        Self {
            event,
            resource: None,
            result: "REJECTED",
            reason: Some(reason),
            metadata: json!({}),
        }
    }
}
pub async fn record(
    tx: &mut PgConnection,
    appointment: Uuid,
    conversation: Option<Uuid>,
    event: Event<'_>,
) -> Result<(), AppError> {
    crate::api::middleware::request_context::bind_consultation(appointment, conversation);
    let context = crate::api::middleware::request_context::current();
    sqlx::query("INSERT INTO consultation_trace_events(appointment_id,conversation_id,actor_role,event_type,resource_type,resource_id,result,reason_code,safe_metadata,client_type,client_build) VALUES($1,$2,'SYSTEM',$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(appointment).bind(conversation).bind(event.event).bind(event.resource.map(|r|r.0)).bind(event.resource.map(|r|r.1))
        .bind(event.result).bind(event.reason).bind(event.metadata).bind(context.as_ref().and_then(|c|c.client_type.as_deref())).bind(context.as_ref().and_then(|c|c.client_build.as_deref()))
        .execute(tx).await?;
    Ok(())
}
pub async fn rejected(
    state: &AppState,
    ctx: &AuthContext,
    appointment: Uuid,
    conversation: Option<Uuid>,
    event: &str,
    reason: &str,
) -> Result<(), AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    record(
        &mut tx,
        appointment,
        conversation,
        Event::rejected(event, reason),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn admin_read(
    tx: &mut PgConnection,
    ctx: &AuthContext,
    action: &str,
    resource: &str,
    id: Uuid,
) -> Result<(), AppError> {
    if ctx.role == UserRole::Admin {
        AuditService::log_in_tx(
            tx,
            AuditLogEntry::new(
                Some(ctx.user_id),
                ctx.role,
                action,
                resource,
                Some(id),
                "SUCCESS",
            ),
        )
        .await?;
    }
    Ok(())
}
pub async fn maintenance(state: &AppState) -> Result<u64, AppError> {
    let ctx = AuthContext::system();
    let mut tx = state.begin_authed_as(&ctx, DbRole::SystemWorker).await?;
    // Milestone 8.0 critical-query audit: `now()` (transaction start time,
    // STABLE) instead of `clock_timestamp()` (VOLATILE) here lets the
    // planner fold the comparison to a constant and use real statistics
    // against the new idx_chat_conversations_expiry_unlocked partial index,
    // rather than falling back to a generic selectivity guess that forced a
    // full scan regardless of the index (verified via EXPLAIN: 387 buffers
    // -> 2 buffers). A conversation that becomes eligible mid-scan is simply
    // picked up on this job's next 60-second tick; the per-row lock UPDATE
    // below still uses genuinely-current `clock_timestamp()`.
    let expired:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT id,appointment_id FROM chat_conversations WHERE NOT is_locked AND expires_at<=now() ORDER BY expires_at LIMIT 100 FOR UPDATE SKIP LOCKED")
        .fetch_all(&mut *tx).await?;
    for (conversation, appointment) in expired {
        sqlx::query("UPDATE chat_conversations SET is_locked=TRUE,locked_at=clock_timestamp(),lock_reason='WINDOW_EXPIRED' WHERE id=$1 AND NOT is_locked").bind(conversation).execute(&mut *tx).await?;
        record(
            &mut tx,
            appointment,
            Some(conversation),
            Event::accepted("CHAT_LOCKED", Some(("CHAT", conversation))),
        )
        .await?;
    }
    let deleted: i64 = sqlx::query_scalar("SELECT cleanup_consultation_trace(500)")
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(deleted as u64)
}

pub async fn reject_chat_if_visible(
    state: &AppState,
    ctx: &AuthContext,
    conversation: Uuid,
    reason: &str,
) -> Result<(), AppError> {
    let mut tx = state.begin_authed(ctx).await?;
    if let Some(chat) =
        crate::repository::chat_repo::find_conversation(&mut *tx, conversation).await?
    {
        record(
            &mut tx,
            chat.appointment_id,
            Some(conversation),
            Event::rejected("CHAT_MESSAGE_REJECTED", reason),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
