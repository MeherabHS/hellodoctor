use crate::domain::models::{AuditEvent, UserRole};
use crate::error::AppError;
use crate::repository::{audit_repo, AppState};
use chrono::Utc;
use uuid::Uuid;

pub struct AuditService;

impl AuditService {
    #[allow(clippy::too_many_arguments)]
    pub async fn log(
        state: &AppState,
        actor_id: Option<Uuid>,
        actor_role: UserRole,
        action: &str,
        resource_type: &str,
        resource_id: Option<Uuid>,
        result: &str,
        reason: Option<String>,
    ) -> Result<AuditEvent, AppError> {
        let event = AuditEvent {
            id: Uuid::new_v4(),
            actor_id,
            actor_role,
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id,
            session_id: None,
            request_id: None,
            ip_hash: None,
            result: result.to_string(),
            reason,
            created_at: Utc::now(),
        };

        audit_repo::insert(&state.db, &event).await?;
        Ok(event)
    }
}
