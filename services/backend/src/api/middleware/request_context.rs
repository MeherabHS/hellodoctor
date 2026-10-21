use crate::auth_context::AuthContext;
use axum::{extract::Request, middleware::Next, response::Response};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct RequestContext {
    pub id: String,
    pub client_type: Option<String>,
    pub client_build: Option<String>,
    pub evidence: Arc<Mutex<RequestEvidence>>,
}
#[derive(Clone, Debug, Default)]
pub struct RequestEvidence {
    pub actor: Option<AuthContext>,
    pub appointment: Option<Uuid>,
    pub conversation: Option<Uuid>,
    pub retention_days: u32,
}
pub fn bind_actor(actor: &AuthContext, retention_days: u32) {
    if let Some(context) = current() {
        if let Ok(mut evidence) = context.evidence.lock() {
            evidence.actor = Some(actor.clone());
            evidence.retention_days = retention_days;
        }
    }
}
pub fn bind_consultation(appointment: Uuid, conversation: Option<Uuid>) {
    if let Some(context) = current() {
        if let Ok(mut evidence) = context.evidence.lock() {
            evidence.appointment = Some(appointment);
            if conversation.is_some() {
                evidence.conversation = conversation;
            }
        }
    }
}
pub fn evidence() -> Option<RequestEvidence> {
    let context = current()?;
    let scope = context.evidence.lock().ok()?.clone();
    if scope.actor.is_some() && scope.appointment.is_some() {
        Some(scope)
    } else {
        None
    }
}
tokio::task_local! { pub static REQUEST_CONTEXT: RequestContext; }
pub fn current() -> Option<RequestContext> {
    REQUEST_CONTEXT.try_with(Clone::clone).ok()
}
pub fn request_id() -> String {
    current().map_or_else(|| Uuid::new_v4().to_string(), |c| c.id)
}
pub async fn middleware(request: Request, next: Next) -> Response {
    let (id, client_type, client_build) = {
        let header = |name: &str| {
            request
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let id = header("x-request-id")
            .and_then(|s| Uuid::parse_str(&s).ok())
            .unwrap_or_else(Uuid::new_v4)
            .to_string();
        let client_type = header("x-client-type").filter(|s| {
            [
                "PATIENT_FLUTTER",
                "DOCTOR_FLUTTER",
                "DOCTOR_WEB",
                "ADMIN_WEB",
            ]
            .contains(&s.as_str())
        });
        let client_build = header("x-client-build").filter(|s| {
            s.len() <= 40
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        });
        (id, client_type, client_build)
    };
    REQUEST_CONTEXT
        .scope(
            RequestContext {
                id: id.clone(),
                client_type,
                client_build,
                evidence: Arc::new(Mutex::new(RequestEvidence::default())),
            },
            async move {
                let mut response = next.run(request).await;
                if let Ok(value) = id.parse() {
                    response.headers_mut().insert("x-request-id", value);
                }
                response
            },
        )
        .await
}
