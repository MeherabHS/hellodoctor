use crate::domain::models::Gateway;
use sqlx::PgExecutor;
use uuid::Uuid;

/// Attempts to record a provider webhook event, relying on the pre-existing
/// unique `(gateway, provider_event_id)` index for deduplication. Returns
/// `true` if this is a genuinely new event (safe to process), `false` if this
/// exact `provider_event_id` was already recorded for this gateway — a
/// replay (whether a network-level retry redelivering the identical
/// callback, or a provider resending under the same id), which the caller
/// must not repeat any financial effect for.
pub async fn try_record_event<'e, E: PgExecutor<'e>>(
    executor: E,
    transaction_id: Uuid,
    gateway: Gateway,
    provider_event_id: &str,
    event_type: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO payment_events (transaction_id, gateway, provider_event_id, provider_event_type, event_type) \
         VALUES ($1, $2, $3, $4, $4) \
         ON CONFLICT (gateway, provider_event_id) DO NOTHING",
    )
    .bind(transaction_id)
    .bind(gateway)
    .bind(provider_event_id)
    .bind(event_type)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() == 1)
}
